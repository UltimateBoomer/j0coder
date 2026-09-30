use crate::contract::*;
use anyhow::{Result, ensure};
use k8s_openapi::{
    api::core::v1::{
        Container, EmptyDirVolumeSource, LocalObjectReference, Pod, PodSpec, ResourceRequirements,
        SeccompProfile, SecurityContext, Toleration, Volume, VolumeMount,
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::ObjectMeta},
};
use kube::{
    Api, Client,
    api::{AttachParams, DeleteParams, ListParams, LogParams, PostParams},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    process::Command,
};
#[derive(Clone)]
pub struct Podman {
    pub image: String,
    pub runtime: String,
}

#[derive(Clone)]
pub struct Kubernetes {
    pods: Api<Pod>,
    image: String,
    runtime_class: String,
    start_timeout: Duration,
    node_selector: BTreeMap<String, String>,
    tolerations: Vec<Toleration>,
    image_pull_secrets: Vec<LocalObjectReference>,
}

#[derive(Clone)]
pub enum Backend {
    Podman(Podman),
    Kubernetes(Box<Kubernetes>),
}

fn backend_kind(value: &str) -> Result<&str> {
    ensure!(
        matches!(value, "podman" | "kubernetes"),
        "unsupported SANDBOX_BACKEND {value:?}; expected podman or kubernetes"
    );
    Ok(value)
}

fn sandbox_memory(name: &str, compile: bool, limits: &Limits) -> u64 {
    if compile {
        if name == "solution.kt" { 2048 } else { 1024 }
    } else if name == "program.jar.b64" {
        limits.memory_mib + 256
    } else {
        limits.memory_mib
    }
}
fn sandbox_wall(name: &str, compile: bool, limits: &Limits) -> u64 {
    if compile {
        if name == "solution.kt" {
            120_000
        } else if name == "solution.java" {
            45_000
        } else {
            30_000
        }
    } else if name == "program.jar.b64" {
        limits.time_ms + 1500
    } else {
        limits.time_ms
    }
}

pub struct LspSession {
    pub stdin: Option<Box<dyn AsyncWrite + Unpin + Send>>,
    pub stdout: Option<Box<dyn AsyncRead + Unpin + Send>>,
    cleanup: LspCleanup,
}
enum LspCleanup {
    Podman(Podman, String, tokio::process::Child),
    Kubernetes(Api<Pod>, String, kube::api::AttachedProcess),
}
impl LspSession {
    pub async fn cleanup(mut self) {
        match &mut self.cleanup {
            LspCleanup::Podman(backend, id, child) => {
                let _ = child.kill().await;
                backend.cleanup(id).await;
            }
            LspCleanup::Kubernetes(pods, id, process) => {
                process.abort();
                let _ = tokio::time::timeout(
                    Duration::from_secs(10),
                    pods.delete(
                        id,
                        &DeleteParams {
                            grace_period_seconds: Some(0),
                            ..Default::default()
                        },
                    ),
                )
                .await;
            }
        }
    }
}

impl Backend {
    pub async fn from_env() -> Result<Self> {
        match backend_kind(&crate::env("SANDBOX_BACKEND", "podman"))? {
            "podman" => Ok(Self::Podman(Podman::new())),
            "kubernetes" => Ok(Self::Kubernetes(Box::new(Kubernetes::new().await?))),
            _ => unreachable!(),
        }
    }
    pub async fn preflight(&self) -> Result<()> {
        match self {
            Self::Podman(b) => b.preflight().await,
            Self::Kubernetes(b) => b.preflight().await,
        }
    }
    pub async fn execute(
        &self,
        source: &[u8],
        name: &str,
        args: &str,
        limits: &Limits,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<std::result::Result<Collected, Verdict>> {
        match self {
            Self::Podman(b) => {
                b.execute(source, name, args, limits, compile, shutdown)
                    .await
            }
            Self::Kubernetes(b) => {
                b.execute(source, name, args, limits, compile, shutdown)
                    .await
            }
        }
    }
    pub async fn start_lsp(
        &self,
        command: &[&str],
        language: Language,
        session: &str,
    ) -> Result<LspSession> {
        match self {
            Self::Podman(b) => b.start_lsp(command, language, session).await,
            Self::Kubernetes(b) => b.start_lsp(command, language, session).await,
        }
    }
    pub async fn cleanup_orphaned_editors(
        &self,
        active: &HashSet<String>,
        previously_orphaned: &HashSet<String>,
    ) -> Result<HashSet<String>> {
        let mut orphaned = HashSet::new();
        match self {
            Self::Podman(b) => {
                let output = b
                    .checked(&[
                        "ps",
                        "-a",
                        "--filter=label=locoder.editor=true",
                        "--format={{.ID}} {{.Names}}",
                    ])
                    .await?;
                for line in String::from_utf8(output)?.lines() {
                    if let Some((id, name)) = line.split_once(' ')
                        && let Some(session) = name.strip_prefix("locoder-editor-")
                        && !active.contains(session)
                    {
                        orphaned.insert(session.to_owned());
                        if previously_orphaned.contains(session) {
                            b.cleanup(id).await;
                        }
                    }
                }
            }
            Self::Kubernetes(b) => {
                for pod in
                    b.pods
                        .list(&ListParams::default().labels(
                            "app.kubernetes.io/managed-by=locoder,locoder.io/purpose=editor",
                        ))
                        .await?
                {
                    if let Some(name) = pod.metadata.name
                        && let Some(session) = name.strip_prefix("locoder-editor-")
                        && !active.contains(session)
                    {
                        orphaned.insert(session.to_owned());
                        if previously_orphaned.contains(session) {
                            b.delete_pod(&name).await;
                        }
                    }
                }
            }
        }
        Ok(orphaned)
    }
}

impl Kubernetes {
    pub async fn new() -> Result<Self> {
        let client = Client::try_default().await?;
        let namespace = crate::env("SANDBOX_NAMESPACE", "locoder-sandbox");
        Ok(Self {
            pods: Api::namespaced(client, &namespace),
            image: crate::env("TOOLCHAIN_IMAGE", "localhost/locoder-toolchain:1"),
            runtime_class: crate::env("SANDBOX_RUNTIME_CLASS", ""),
            start_timeout: Duration::from_secs(
                crate::env("KUBERNETES_POD_START_TIMEOUT", "30").parse()?,
            ),
            node_selector: serde_json::from_str(&crate::env("SANDBOX_NODE_SELECTOR", "{}"))?,
            tolerations: serde_json::from_str(&crate::env("SANDBOX_TOLERATIONS", "[]"))?,
            image_pull_secrets: serde_json::from_str(&crate::env(
                "SANDBOX_IMAGE_PULL_SECRETS",
                "[]",
            ))?,
        })
    }
    fn pod(
        &self,
        name: &str,
        purpose: &str,
        command: Vec<String>,
        memory_mib: u64,
        ttl: u64,
    ) -> Pod {
        let mut labels = BTreeMap::new();
        labels.insert("app.kubernetes.io/managed-by".into(), "locoder".into());
        labels.insert("locoder.io/owner".into(), "locoder".into());
        labels.insert("locoder.io/purpose".into(), purpose.into());
        labels.insert(
            "locoder.io/id".into(),
            name.trim_start_matches("locoder-").into(),
        );
        labels.insert(
            "locoder.io/expiry".into(),
            (chrono::Utc::now().timestamp() + ttl as i64).to_string(),
        );
        let security = SecurityContext {
            allow_privilege_escalation: Some(false),
            capabilities: Some(k8s_openapi::api::core::v1::Capabilities {
                add: None,
                drop: Some(vec!["ALL".into()]),
            }),
            read_only_root_filesystem: Some(true),
            run_as_group: Some(65534),
            run_as_non_root: Some(true),
            run_as_user: Some(65534),
            seccomp_profile: Some(SeccompProfile {
                type_: "RuntimeDefault".into(),
                localhost_profile: None,
            }),
            ..Default::default()
        };
        let mut requests = BTreeMap::new();
        requests.insert("cpu".into(), Quantity("100m".into()));
        requests.insert(
            "memory".into(),
            Quantity(if purpose == "editor" && memory_mib >= 2048 {
                format!("{memory_mib}Mi")
            } else {
                "128Mi".into()
            }),
        );
        requests.insert("ephemeral-storage".into(), Quantity("64Mi".into()));
        let mut limits = BTreeMap::new();
        limits.insert("cpu".into(), Quantity("1".into()));
        limits.insert("memory".into(), Quantity(format!("{memory_mib}Mi")));
        limits.insert(
            "ephemeral-storage".into(),
            Quantity(if purpose == "editor" { "2Gi" } else { "256Mi" }.into()),
        );
        let volumes = [
            ("tmp", "32Mi"),
            ("input", "48Mi"),
            ("work", "128Mi"),
            (
                "editor-cache",
                if purpose == "editor" { "1Gi" } else { "32Mi" },
            ),
        ]
        .into_iter()
        .map(|(n, s)| Volume {
            name: n.into(),
            empty_dir: Some(EmptyDirVolumeSource {
                medium: None,
                size_limit: Some(Quantity(s.into())),
            }),
            ..Default::default()
        })
        .collect();
        let mounts = [
            ("tmp", "/tmp"),
            ("input", "/input"),
            ("work", "/work"),
            ("editor-cache", "/workspace/.cache"),
        ]
        .into_iter()
        .map(|(n, p)| VolumeMount {
            name: n.into(),
            mount_path: p.into(),
            ..Default::default()
        })
        .collect();
        Pod {
            metadata: ObjectMeta {
                name: Some(name.into()),
                labels: Some(labels),
                ..Default::default()
            },
            spec: Some(PodSpec {
                automount_service_account_token: Some(false),
                containers: vec![Container {
                    name: "sandbox".into(),
                    image: Some(self.image.clone()),
                    image_pull_policy: Some("IfNotPresent".into()),
                    command: Some(command),
                    working_dir: (purpose == "editor").then(|| "/workspace".into()),
                    env: Some(
                        [
                            ("LOCODER_STREAM_PROTOCOL", "1"),
                            ("HOME", "/workspace/.cache"),
                            ("XDG_CACHE_HOME", "/workspace/.cache"),
                            (
                                "IJ_JAVA_OPTIONS",
                                if purpose == "editor" && memory_mib == 4096 {
                                    "-Xmx3g -Duser.home=/workspace/.cache"
                                } else {
                                    ""
                                },
                            ),
                        ]
                        .into_iter()
                        .map(|(name, value)| k8s_openapi::api::core::v1::EnvVar {
                            name: name.into(),
                            value: Some(value.into()),
                            ..Default::default()
                        })
                        .collect(),
                    ),
                    stdin: Some(true),
                    stdin_once: Some(true),
                    tty: Some(false),
                    security_context: Some(security),
                    resources: Some(ResourceRequirements {
                        limits: Some(limits),
                        requests: Some(requests),
                        ..Default::default()
                    }),
                    volume_mounts: Some(mounts),
                    ..Default::default()
                }],
                enable_service_links: Some(false),
                restart_policy: Some("Never".into()),
                runtime_class_name: Some(self.runtime_class.clone()),
                image_pull_secrets: (!self.image_pull_secrets.is_empty())
                    .then(|| self.image_pull_secrets.clone()),
                node_selector: (!self.node_selector.is_empty()).then(|| self.node_selector.clone()),
                tolerations: (!self.tolerations.is_empty()).then(|| self.tolerations.clone()),
                security_context: Some(k8s_openapi::api::core::v1::PodSecurityContext {
                    fs_group: Some(65534),
                    run_as_group: Some(65534),
                    run_as_non_root: Some(true),
                    run_as_user: Some(65534),
                    seccomp_profile: Some(SeccompProfile {
                        type_: "RuntimeDefault".into(),
                        localhost_profile: None,
                    }),
                    ..Default::default()
                }),
                termination_grace_period_seconds: Some(1),
                volumes: Some(volumes),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
    async fn create_pod(&self, pod: &Pod) -> Result<()> {
        tokio::time::timeout(
            self.start_timeout,
            self.pods.create(&PostParams::default(), pod),
        )
        .await
        .map_err(|_| anyhow::anyhow!("timed out creating sandbox Pod"))??;
        Ok(())
    }
    async fn delete_pod(&self, name: &str) {
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            self.pods.delete(
                name,
                &DeleteParams {
                    grace_period_seconds: Some(0),
                    ..Default::default()
                },
            ),
        )
        .await;
    }
    async fn wait_running(&self, name: &str) -> Result<()> {
        tokio::time::timeout(self.start_timeout, async {
            loop {
                let p = self.pods.get(name).await?;
                if p.status.as_ref().and_then(|s| s.phase.as_deref()) == Some("Running") {
                    return Ok::<_, anyhow::Error>(());
                }
                if matches!(
                    p.status.as_ref().and_then(|s| s.phase.as_deref()),
                    Some("Failed" | "Succeeded")
                ) {
                    anyhow::bail!("sandbox Pod terminated before attach")
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for sandbox Pod"))??;
        Ok(())
    }
    async fn attached(
        &self,
        name: &str,
        input: &[u8],
        wall: Duration,
        cap: usize,
    ) -> Result<(Vec<u8>, bool)> {
        self.wait_running(name).await?;
        let mut process = tokio::time::timeout(
            self.start_timeout,
            self.pods.attach(
                name,
                &AttachParams::default()
                    .stdin(true)
                    .stdout(true)
                    .stderr(false),
            ),
        )
        .await
        .map_err(|_| anyhow::anyhow!("timed out attaching to sandbox Pod"))??;
        let mut stdin = process
            .stdin()
            .ok_or_else(|| anyhow::anyhow!("attach stdin unavailable"))?;
        stdin.write_all(input).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let mut stdout = process
            .stdout()
            .ok_or_else(|| anyhow::anyhow!("attach stdout unavailable"))?;
        let status = process
            .take_status()
            .ok_or_else(|| anyhow::anyhow!("attach status unavailable"))?;
        tokio::pin!(status);
        let deadline = tokio::time::sleep(wall);
        tokio::pin!(deadline);
        let mut completed = false;
        let mut out = Vec::new();
        let mut buf = [0; 8192];
        loop {
            tokio::select! {
                n = stdout.read(&mut buf) => {
                    let n = n?;
                    if n == 0 {
                        return Ok((out, false));
                    }
                    ensure!(out.len() + n <= cap, "sandbox response exceeded bound");
                    out.extend_from_slice(&buf[..n]);
                }
                _ = &mut status, if !completed => {
                    // The exit status can arrive before the final stdout frames.
                    // Aborting here discards a valid harness response.
                    completed = true;
                }
                _ = &mut deadline => {
                    process.abort();
                    return Ok((out, true));
                }
            }
        }
    }
    pub async fn execute(
        &self,
        source: &[u8],
        name: &str,
        input: &str,
        limits: &Limits,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<std::result::Result<Collected, Verdict>> {
        ensure!(
            !self.runtime_class.is_empty(),
            "SANDBOX_RUNTIME_CLASS is required"
        );
        let id = format!("locoder-{}", uuid::Uuid::new_v4().simple());
        let cap = if compile {
            64 * 1024 * 1024
        } else {
            limits.output_bytes as usize * 6 + 65536
        };
        let output = limits.output_bytes.to_string();
        let pod = self.pod(
            &id,
            if compile { "compile" } else { "execute" },
            vec![
                "python3".into(),
                "/opt/harness.py".into(),
                if compile {
                    "compile".into()
                } else {
                    "run".into()
                },
                output,
                limits.memory_mib.to_string(),
            ],
            sandbox_memory(name, compile, limits),
            crate::env("KUBERNETES_SANDBOX_TTL", "300").parse()?,
        );
        self.create_pod(&pod).await?;
        let request = serde_json::to_vec(
            &serde_json::json!({"name":name,"data":base64_encode(source),"input":input}),
        )?;
        let wall = Duration::from_millis(sandbox_wall(name, compile, limits));
        let run = tokio::select! { r=self.attached(&id,&request,wall,cap) => r, _=shutdown.cancelled()=>Err(anyhow::anyhow!("execution cancelled during shutdown")) };
        let should_wait_for_status = matches!(&run, Ok((_, false)));
        let status = if should_wait_for_status {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    let pod = self.pods.get(&id).await?;
                    if matches!(
                        pod.status
                            .as_ref()
                            .and_then(|status| status.phase.as_deref()),
                        Some("Failed" | "Succeeded")
                    ) {
                        return Ok::<_, kube::Error>(pod);
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .ok()
            .and_then(Result::ok)
        } else {
            None
        };
        let oom = status
            .as_ref()
            .and_then(|p| p.status.as_ref())
            .and_then(|s| s.container_statuses.as_ref())
            .and_then(|s| s.first())
            .and_then(|s| s.state.as_ref())
            .and_then(|s| s.terminated.as_ref())
            .and_then(|t| t.reason.as_deref())
            == Some("OOMKilled");
        // Some runtimes close an attach stream without delivering its final
        // stdout frames. A completed Pod's log is the same bounded harness
        // response and remains available until the Pod is deleted.
        let recovered = if status.is_some()
            && matches!(&run, Ok((bytes, false)) if serde_json::from_slice::<Collected>(bytes).is_err())
        {
            tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(log) = self
                        .pods
                        .logs(
                            &id,
                            &LogParams {
                                limit_bytes: Some((cap + 1) as i64),
                                ..Default::default()
                            },
                        )
                        .await
                        && !log.is_empty()
                    {
                        break log;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            })
            .await
            .ok()
        } else {
            None
        };
        self.delete_pod(&id).await;
        let (mut bytes, timeout) = match run {
            Ok(value) => value,
            Err(error) if error.to_string().contains("response exceeded bound") => {
                return Ok(Err(Verdict::OutputLimit));
            }
            Err(error) => return Err(error),
        };
        if !timeout
            && serde_json::from_slice::<Collected>(&bytes).is_err()
            && let Some(log) = recovered
        {
            ensure!(log.len() <= cap, "sandbox response exceeded bound");
            bytes = log.into_bytes();
        }
        Ok(if oom {
            Err(Verdict::MemoryLimit)
        } else if timeout {
            Err(Verdict::TimeLimit)
        } else {
            match serde_json::from_slice::<Collected>(&bytes) {
                Ok(c) if c.overflow => Err(Verdict::OutputLimit),
                Ok(c) => Ok(c),
                Err(error) => {
                    tracing::warn!(response_bytes = bytes.len(), error = %error, "sandbox returned an invalid response");
                    anyhow::bail!("sandbox returned an invalid response")
                }
            }
        })
    }
    pub async fn preflight(&self) -> Result<()> {
        ensure!(
            !self.runtime_class.is_empty(),
            "SANDBOX_RUNTIME_CLASS is required"
        );
        let name = format!("locoder-preflight-{}", uuid::Uuid::new_v4().simple());
        let pod = self.pod(&name, "preflight", vec!["dmesg".into()], 256, 60);
        self.create_pod(&pod).await?;
        let result = tokio::time::timeout(self.start_timeout, async {
            loop {
                let pod = self.pods.get(&name).await?;
                match pod
                    .status
                    .as_ref()
                    .and_then(|status| status.phase.as_deref())
                {
                    Some("Succeeded") => break,
                    Some("Failed") => anyhow::bail!("gVisor preflight Pod failed"),
                    _ => tokio::time::sleep(Duration::from_millis(200)).await,
                }
            }
            self.pods
                .logs(&name, &LogParams::default())
                .await
                .map_err(anyhow::Error::from)
        })
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for gVisor preflight Pod"))?;
        self.delete_pod(&name).await;
        let out = result?;
        ensure!(
            out.contains("gVisor"),
            "RuntimeClass did not identify as gVisor"
        );
        Ok(())
    }
    async fn start_lsp(
        &self,
        command: &[&str],
        language: Language,
        session: &str,
    ) -> Result<LspSession> {
        ensure!(
            !self.runtime_class.is_empty(),
            "SANDBOX_RUNTIME_CLASS is required"
        );
        let name = format!(
            "locoder-editor-{}",
            session.to_ascii_lowercase().replace('_', "-")
        );
        let pod = self.pod(
            &name,
            "editor",
            command.iter().map(|s| s.to_string()).collect(),
            match language {
                Language::Java => 2048,
                Language::Kotlin => 4096,
                _ => 512,
            },
            3600,
        );
        self.create_pod(&pod).await?;
        if let Err(e) = self.wait_running(&name).await {
            self.delete_pod(&name).await;
            return Err(e);
        }
        let attach = tokio::time::timeout(
            self.start_timeout,
            self.pods.attach(
                &name,
                &AttachParams::default()
                    .stdin(true)
                    .stdout(true)
                    .stderr(false),
            ),
        )
        .await;
        let mut process = match attach {
            Ok(Ok(process)) => process,
            Ok(Err(error)) => {
                self.delete_pod(&name).await;
                return Err(error.into());
            }
            Err(_) => {
                self.delete_pod(&name).await;
                anyhow::bail!("timed out attaching to editor Pod");
            }
        };
        let Some(stdin) = process.stdin() else {
            process.abort();
            self.delete_pod(&name).await;
            anyhow::bail!("LSP attach stdin unavailable");
        };
        let Some(stdout) = process.stdout() else {
            process.abort();
            self.delete_pod(&name).await;
            anyhow::bail!("LSP attach stdout unavailable");
        };
        let stdin = Box::new(stdin);
        let stdout = Box::new(stdout);
        Ok(LspSession {
            stdin: Some(stdin),
            stdout: Some(stdout),
            cleanup: LspCleanup::Kubernetes(self.pods.clone(), name, process),
        })
    }
    pub async fn cleanup_expired(&self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        for p in self
            .pods
            .list(&ListParams::default().labels("app.kubernetes.io/managed-by=locoder"))
            .await?
        {
            let expired = p
                .metadata
                .labels
                .as_ref()
                .and_then(|l| l.get("locoder.io/expiry"))
                .and_then(|s| s.parse::<i64>().ok())
                .is_some_and(|t| t <= now);
            let terminal = matches!(
                p.status.as_ref().and_then(|s| s.phase.as_deref()),
                Some("Failed" | "Succeeded")
            );
            if (expired || terminal)
                && let Some(n) = p.metadata.name
            {
                let _ = self.pods.delete(&n, &DeleteParams::default()).await;
            }
        }
        Ok(())
    }
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if c.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Collected {
    pub exit: i32,
    #[serde(default, deserialize_with = "deserialize_present_output")]
    pub output: Option<serde_json::Value>,
    pub log: String,
    pub overflow: bool,
    #[serde(default)]
    pub artifact: Option<String>,
}

fn deserialize_present_output<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    // A present JSON null is a valid result for nullable and empty codec values.
    Value::deserialize(deserializer).map(Some)
}

#[allow(async_fn_in_trait)]
pub trait SandboxBackend {
    async fn create(
        &self,
        limits: &Limits,
        name: &str,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<String>;
    async fn start(
        &self,
        id: &str,
        input: &str,
        wall: Duration,
        cap: usize,
    ) -> Result<(Vec<u8>, bool)>;
    async fn collect(&self, id: &str) -> Result<bool>;
    async fn kill(&self, id: &str);
    async fn cleanup(&self, id: &str);
}
impl Default for Podman {
    fn default() -> Self {
        Self::new()
    }
}
impl Podman {
    pub fn new() -> Self {
        Self {
            image: crate::env("TOOLCHAIN_IMAGE", "localhost/locoder-toolchain:1"),
            runtime: crate::env("SANDBOX_RUNTIME", "runsc"),
        }
    }
    pub fn command(&self) -> Command {
        let mut c = Command::new("podman");
        if let Ok(url) = std::env::var("CONTAINER_HOST") {
            c.args(["--remote", "--url", &url]);
        }
        c.kill_on_drop(true);
        c
    }
    pub async fn checked(&self, args: &[&str]) -> Result<Vec<u8>> {
        // Podman's remote CLI cannot select an OCI runtime. The dedicated server
        // selects runsc; inspect every created container before any start/copy.
        let mut filtered = Vec::new();
        let mut skip = false;
        for arg in args {
            if skip {
                skip = false;
                continue;
            }
            if *arg == "--runtime" && std::env::var("CONTAINER_HOST").is_ok() {
                skip = true;
                continue;
            }
            filtered.push(*arg);
        }
        let out = tokio::time::timeout(
            Duration::from_secs(30),
            self.command().args(&filtered).output(),
        )
        .await??;
        ensure!(
            out.status.success(),
            "podman operation failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if args.first() == Some(&"create") {
            let id = String::from_utf8(out.stdout.clone())?;
            let id = id.trim();
            let inspect = self
                .command()
                .args(["inspect", "--format", "{{.OCIRuntime}}", id])
                .output()
                .await;
            let inspect = match inspect {
                Ok(inspect) => inspect,
                Err(error) => {
                    let _ = self.command().args(["rm", "-f", id]).output().await;
                    return Err(error.into());
                }
            };
            let runtime = String::from_utf8_lossy(&inspect.stdout);
            if !inspect.status.success() || runtime.trim().rsplit('/').next() != Some("runsc") {
                let _ = self.command().args(["rm", "-f", id]).output().await;
                anyhow::bail!("refusing container: Podman server did not select gVisor runsc");
            }
        }
        Ok(out.stdout)
    }
    pub async fn preflight(&self) -> Result<()> {
        ensure!(
            self.runtime.rsplit('/').next() == Some("runsc"),
            "gVisor runsc is mandatory"
        );
        let id = self
            .checked(&[
                "create",
                "--timeout=10",
                "--runtime",
                &self.runtime,
                "--network=none",
                "--http-proxy=false",
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--security-opt=label=disable",
                "--user=65534:65534",
                "--memory=256m",
                "--memory-swap=256m",
                "--cpus=1",
                "--pids-limit=64",
                &self.image,
                "dmesg",
            ])
            .await?;
        let id = String::from_utf8(id)?.trim().to_string();
        let result = self.checked(&["start", "-a", &id]).await;
        self.cleanup(&id).await;
        let out = result?;
        ensure!(
            String::from_utf8_lossy(&out).contains("gVisor"),
            "sandbox did not identify as gVisor"
        );
        Ok(())
    }
    pub async fn copy_bytes(&self, id: &str, name: &str, data: &[u8]) -> Result<()> {
        let path = std::env::temp_dir().join(format!("locoder-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, data).await?;
        let r = self
            .checked(&["cp", path.to_str().unwrap(), &format!("{id}:/input/{name}")])
            .await;
        let _ = tokio::fs::remove_file(path).await;
        r?;
        Ok(())
    }
    async fn start_lsp(
        &self,
        command: &[&str],
        language: Language,
        session: &str,
    ) -> Result<LspSession> {
        let name = format!("locoder-editor-{session}");
        let memory = match language {
            Language::Java => "2048m",
            Language::Kotlin => "4096m",
            _ => "512m",
        };
        let mut args = vec![
            "create",
            "--timeout=3600",
            "--name",
            &name,
            "--label=locoder.editor=true",
            "--runtime",
            &self.runtime,
            "--network=none",
            "--http-proxy=false",
            "--read-only",
            "--read-only-tmpfs=false",
            "--user=65534:65534",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--security-opt=label=disable",
            "--memory",
            memory,
            "--memory-swap",
            memory,
            "--cpus=1",
            "--pids-limit=64",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,nodev,size=32m,mode=1777",
            "--tmpfs",
            "/workspace/.cache:rw,noexec,nosuid,nodev,size=1024m,mode=1777",
            "--log-driver=none",
            "--workdir=/workspace",
            "--env=HOME=/workspace/.cache",
            "--env=XDG_CACHE_HOME=/workspace/.cache",
            "-i",
            &self.image,
        ];
        if language == Language::Kotlin {
            args.splice(
                args.len() - 2..args.len() - 2,
                ["--env=IJ_JAVA_OPTIONS=-Xmx3g -Duser.home=/workspace/.cache"],
            );
        }
        args.extend(command.iter().copied());
        let id = String::from_utf8(self.checked(&args).await?)?
            .trim()
            .to_string();
        let child = self
            .command()
            .args(["start", "-a", "-i", &id])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                self.cleanup(&id).await;
                return Err(error.into());
            }
        };
        let Some(stdin) = child.stdin.take() else {
            let _ = child.kill().await;
            self.cleanup(&id).await;
            anyhow::bail!("LSP stdin unavailable");
        };
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill().await;
            self.cleanup(&id).await;
            anyhow::bail!("LSP stdout unavailable");
        };
        let stdin = Box::new(stdin);
        let stdout = Box::new(stdout);
        Ok(LspSession {
            stdin: Some(stdin),
            stdout: Some(stdout),
            cleanup: LspCleanup::Podman(self.clone(), id, child),
        })
    }
    pub async fn execute(
        &self,
        source: &[u8],
        name: &str,
        args: &str,
        limits: &Limits,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<std::result::Result<Collected, Verdict>> {
        let id = self.create(limits, name, compile, shutdown).await?;
        let guard = Cleanup(self.clone(), id.clone());
        let operation = async {
            self.copy_bytes(&id, name, source).await?;
            let result = self
                .start(
                    &id,
                    args,
                    Duration::from_millis(sandbox_wall(name, compile, limits)),
                    if compile {
                        64 * 1024 * 1024
                    } else {
                        limits.output_bytes as usize * 6 + 65536
                    },
                )
                .await;
            let oom = self.collect(&id).await.unwrap_or(false);
            let (bytes, timeout) = result?;
            Ok::<_, anyhow::Error>(if oom {
                Err(Verdict::MemoryLimit)
            } else if timeout {
                Err(Verdict::TimeLimit)
            } else {
                match serde_json::from_slice::<Collected>(&bytes) {
                    Ok(c) if c.overflow => Err(Verdict::OutputLimit),
                    Ok(c) => Ok(c),
                    Err(_) => Err(if compile {
                        Verdict::CompilationError
                    } else {
                        Verdict::RuntimeError
                    }),
                }
            })
        };
        let outcome = tokio::select! {
            result = operation => result?,
            _ = shutdown.cancelled() => {
                self.cleanup(&id).await;
                std::mem::forget(guard);
                anyhow::bail!("execution cancelled during shutdown")
            }
        };
        self.cleanup(&id).await;
        std::mem::forget(guard);
        Ok(outcome)
    }
}
struct Cleanup(Podman, String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let backend = self.0.clone();
        let id = self.1.clone();
        tokio::spawn(async move {
            backend.cleanup(&id).await;
        });
    }
}
impl SandboxBackend for Podman {
    async fn create(
        &self,
        l: &Limits,
        source_name: &str,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<String> {
        let name = format!("locoder-{}", uuid::Uuid::new_v4());
        let memory = sandbox_memory(source_name, compile, l);
        let timeout = (sandbox_wall(source_name, compile, l).div_ceil(1000) + 5).to_string();
        let memory = format!("{memory}m");
        let output_bytes = l.output_bytes.to_string();
        let heap_mib = l.memory_mib.to_string();
        let args = [
            "create",
            "--timeout",
            &timeout,
            "--name",
            &name,
            "--label=locoder.sandbox=true",
            "--runtime",
            &self.runtime,
            "--network=none",
            "--http-proxy=false",
            "--user=65534:65534",
            "--read-only",
            "--read-only-tmpfs=false",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--security-opt=label=disable",
            "--pids-limit=64",
            "--cpus=1",
            "--memory",
            &memory,
            "--memory-swap",
            &memory,
            "--tmpfs",
            "/work:rw,exec,nosuid,nodev,size=128m,mode=1777",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,nodev,size=32m,mode=1777",
            "--shm-size=8m",
            "--ulimit",
            "nofile=64:64",
            "--ulimit",
            "fsize=67108864:67108864",
            "--log-driver=none",
            "-i",
            &self.image,
            "python3",
            "/opt/harness.py",
            if compile { "compile" } else { "run" },
            &output_bytes,
            &heap_mib,
        ];
        let create = self.checked(&args);
        let out = tokio::select! {
            result = create => result?,
            _ = shutdown.cancelled() => {
                // Removing by the unique requested name also covers a create that
                // reached the Podman service just before its CLI was cancelled.
                self.cleanup(&name).await;
                anyhow::bail!("sandbox creation cancelled during shutdown")
            }
        };
        Ok(String::from_utf8(out)?.trim().into())
    }
    async fn start(
        &self,
        id: &str,
        input: &str,
        wall: Duration,
        cap: usize,
    ) -> Result<(Vec<u8>, bool)> {
        let mut child = self
            .command()
            .args(["start", "-a", "-i", id])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input.as_bytes()).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let mut stdout = child.stdout.take().unwrap();
        let mut bytes = Vec::new();
        let r = tokio::time::timeout(wall, async {
            let mut b = [0u8; 8192];
            loop {
                let n = stdout.read(&mut b).await?;
                if n == 0 {
                    break;
                }
                if bytes.len() + n > cap {
                    return Ok::<_, std::io::Error>(false);
                }
                bytes.extend_from_slice(&b[..n]);
            }
            let status = child.wait().await?;
            if status.code() == Some(125)
                || status.code() == Some(126)
                || status.code() == Some(127)
            {
                return Err(std::io::Error::other("Podman failed to start sandbox"));
            }
            Ok(true)
        })
        .await;
        match r {
            Ok(Ok(true)) => Ok((bytes, false)),
            Ok(Ok(false)) => {
                self.kill(id).await;
                Ok((
                    serde_json::to_vec(&Collected {
                        exit: 1,
                        output: None,
                        log: String::new(),
                        overflow: true,
                        artifact: None,
                    })?,
                    false,
                ))
            }
            Ok(Err(e)) => Err(e.into()),
            Err(_) => {
                self.kill(id).await;
                Ok((vec![], true))
            }
        }
    }
    async fn collect(&self, id: &str) -> Result<bool> {
        let out = self
            .checked(&["inspect", "--format", "{{.State.OOMKilled}}", id])
            .await?;
        Ok(String::from_utf8_lossy(&out).trim() == "true")
    }
    async fn kill(&self, id: &str) {
        let _ = self.checked(&["kill", id]).await;
    }
    async fn cleanup(&self, id: &str) {
        let _ = self.checked(&["rm", "-f", id]).await;
    }
}
pub async fn judge(job: &Job, source: &str, tests: &[Case]) -> Result<Outcome> {
    let (_trigger, shutdown) = crate::shutdown::channel();
    judge_with_shutdown(job, source, tests, &shutdown).await
}

pub async fn judge_with_shutdown(
    job: &Job,
    source: &str,
    tests: &[Case],
    shutdown: &crate::shutdown::Shutdown,
) -> Result<Outcome> {
    run_cases(job, source, tests, shutdown, true).await
}

/// Execute a private reference with the same wrapper and limits as a submission.
/// Returned values are typed and ordered by case; callers decide whether to expose them.
pub async fn reference_outputs(job: &Job, source: &str, tests: &[Case]) -> Result<Vec<Value>> {
    reference_values(job, source, tests, true).await
}

/// Produce typed expected values for input-only cases. Stateful callers may use
/// placeholder operation expectations; they are not compared in this mode.
pub async fn reference_expected_outputs(
    job: &Job,
    source: &str,
    inputs: &[Case],
) -> Result<Vec<Value>> {
    reference_values(job, source, inputs, false).await
}

async fn reference_values(
    job: &Job,
    source: &str,
    tests: &[Case],
    compare_expected: bool,
) -> Result<Vec<Value>> {
    let (_trigger, shutdown) = crate::shutdown::channel();
    let visible = tests
        .iter()
        .cloned()
        .map(|mut case| {
            case.hidden = false;
            case
        })
        .collect::<Vec<_>>();
    let outcome = run_cases(job, source, &visible, &shutdown, compare_expected).await?;
    if outcome.verdict != Verdict::Accepted {
        let detail = outcome
            .cases
            .iter()
            .enumerate()
            .find(|(_, c)| c.verdict.as_ref().is_some_and(|v| *v != Verdict::Accepted))
            .map(|(i, c)| format!("case {}: {}", i + 1, c.log))
            .unwrap_or_default();
        anyhow::bail!(
            "{:?}: {} {}",
            outcome.verdict,
            outcome.diagnostic.unwrap_or_default(),
            detail
        );
    }
    outcome
        .cases
        .into_iter()
        .enumerate()
        .map(|(index, case)| {
            case.output.ok_or_else(|| {
                anyhow::anyhow!("case {}: {:?}: {}", index + 1, case.verdict, case.log)
            })
        })
        .collect()
}

async fn run_cases(
    job: &Job,
    source: &str,
    tests: &[Case],
    shutdown: &crate::shutdown::Shutdown,
    compare_expected: bool,
) -> Result<Outcome> {
    let b = Backend::from_env().await?;
    let wrapped =
        job.language
            .wrapper_with_definitions(&job.interface, &job.type_definitions, source)?;
    let execution = job.language.execution();
    let mut artifact = None;
    let mut result = Outcome {
        elapsed_ms: 0,
        verdict: Verdict::Accepted,
        passed: 0,
        total: tests.len(),
        cases: vec![],
        diagnostic: None,
    };
    if execution.compilation_required {
        match b
            .execute(
                wrapped.as_bytes(),
                execution.source_filename,
                "",
                &job.limits,
                true,
                shutdown,
            )
            .await?
        {
            Ok(c) if c.exit == 0 => artifact = c.artifact,
            Ok(c) => {
                result.verdict = Verdict::CompilationError;
                result.diagnostic = Some(c.log);
                result.cases = not_run(tests);
                return Ok(result);
            }
            Err(v) => {
                result.verdict = if v == Verdict::TimeLimit
                    || v == Verdict::MemoryLimit
                    || v == Verdict::OutputLimit
                {
                    v
                } else {
                    Verdict::CompilationError
                };
                result.cases = not_run(tests);
                return Ok(result);
            }
        }
        if artifact.is_none() {
            anyhow::bail!("compiler returned no artifact")
        }
    }
    for case in tests {
        let (name, code) = match &artifact {
            Some(a) => (execution.compiled_artifact_filename, a.as_bytes()),
            None => (execution.source_filename, wrapped.as_bytes()),
        };
        let r = b
            .execute(
                code,
                name,
                &serde_json::to_string(&case_input(case))?,
                &job.limits,
                false,
                shutdown,
            )
            .await?;
        let (verdict, output, log) = match r {
            Err(v) => (Some(v), None, String::new()),
            Ok(c) if c.exit != 0 => (Some(Verdict::RuntimeError), None, c.log),
            Ok(c) => {
                let (verdict, detail) =
                    compare_case(job, case, c.output.as_ref(), compare_expected);
                let log = match detail {
                    Some(d) if c.log.is_empty() => d,
                    Some(d) => format!("{}\n{}", c.log, d),
                    None => c.log,
                };
                (verdict, c.output, log)
            }
        };
        if verdict.as_ref().is_none_or(|v| *v == Verdict::Accepted) {
            result.passed += 1
        } else if result.verdict == Verdict::Accepted {
            result.verdict = verdict.clone().unwrap()
        }
        result.cases.push(CaseResult {
            hidden: case.hidden,
            verdict,
            output: if case.hidden { None } else { output },
            log: if case.hidden { String::new() } else { log },
        })
    }
    Ok(result)
}

fn case_input(case: &Case) -> Value {
    if let Some(args) = &case.args {
        return args.clone();
    }
    serde_json::json!({
        "constructor_args": case.constructor_args.as_ref().unwrap(),
        "operations": case.operations.as_ref().unwrap().iter().map(|o| serde_json::json!({"method":o.method,"args":o.args})).collect::<Vec<_>>()
    })
}

fn compare_case(
    job: &Job,
    case: &Case,
    output: Option<&Value>,
    compare_expected: bool,
) -> (Option<Verdict>, Option<String>) {
    match &job.interface {
        crate::contract::Interface::Function { returns, .. } => {
            let Some(actual) = output else {
                return (Some(Verdict::RuntimeError), None);
            };
            if !returns.valid_with(actual, &job.type_definitions) {
                (Some(Verdict::RuntimeError), None)
            } else {
                (
                    case.expected
                        .as_ref()
                        .filter(|_| compare_expected)
                        .map(|e| {
                            if job.comparison.matches(returns, e, actual) {
                                Verdict::Accepted
                            } else {
                                Verdict::WrongAnswer
                            }
                        }),
                    None,
                )
            }
        }
        interface @ crate::contract::Interface::DataStructure { .. } => {
            let actual = match output.and_then(Value::as_array) {
                Some(v) => v,
                None => return (Some(Verdict::RuntimeError), None),
            };
            let Some(ops) = case.operations.as_ref() else {
                return (Some(Verdict::RuntimeError), None);
            };
            if actual.len() != ops.len() {
                return (
                    Some(Verdict::RuntimeError),
                    Some(format!(
                        "trace returned {} results for {} operations",
                        actual.len(),
                        ops.len()
                    )),
                );
            }
            for (index, (op, value)) in ops.iter().zip(actual).enumerate() {
                let Some(method) = interface.method(&op.method) else {
                    return (Some(Verdict::RuntimeError), None);
                };
                if !method.returns.valid_with(value, &job.type_definitions) {
                    return (
                        Some(Verdict::RuntimeError),
                        Some(format!(
                            "operation {index} ({}) returned an invalid value",
                            op.method
                        )),
                    );
                }
                if compare_expected && !job.comparison.matches(&method.returns, &op.expected, value)
                {
                    return (
                        Some(Verdict::WrongAnswer),
                        Some(format!(
                            "operation {index} ({}) produced the wrong result",
                            op.method
                        )),
                    );
                }
            }
            (Some(Verdict::Accepted), None)
        }
    }
}

pub fn not_run(tests: &[Case]) -> Vec<CaseResult> {
    tests
        .iter()
        .map(|case| CaseResult {
            hidden: case.hidden,
            verdict: None,
            output: None,
            log: String::new(),
        })
        .collect()
}

#[cfg(test)]
mod kubernetes_tests {
    use super::*;

    #[test]
    fn collected_output_distinguishes_json_null_from_missing_output() {
        let mut response = serde_json::json!({"exit":0,"output":null,"log":"","overflow":false});
        let collected: Collected = serde_json::from_value(response.clone()).unwrap();
        assert_eq!(collected.output, Some(Value::Null));
        response["output"] = serde_json::json!(42);
        let collected: Collected = serde_json::from_value(response.clone()).unwrap();
        assert_eq!(collected.output, Some(serde_json::json!(42)));
        response.as_object_mut().unwrap().remove("output");
        let collected: Collected = serde_json::from_value(response).unwrap();
        assert_eq!(collected.output, None);
    }

    #[test]
    fn jvm_limits_add_only_execution_headroom() {
        let limits = Limits {
            time_ms: 2000,
            memory_mib: 256,
            output_bytes: 4096,
        };
        assert_eq!(sandbox_wall("solution.java", true, &limits), 45_000);
        assert_eq!(sandbox_memory("solution.java", true, &limits), 1024);
        assert_eq!(sandbox_wall("solution.kt", true, &limits), 120_000);
        assert_eq!(sandbox_memory("solution.kt", true, &limits), 2048);
        assert_eq!(sandbox_wall("program.jar.b64", false, &limits), 3500);
        assert_eq!(sandbox_memory("program.jar.b64", false, &limits), 512);
        assert_eq!(sandbox_wall("program.b64", false, &limits), 2000);
        assert_eq!(sandbox_memory("program.b64", false, &limits), 256);
    }

    #[test]
    fn reference_comparison_can_check_or_ignore_recorded_expectations() {
        let mut job = Job {
            attempt_base: 0,
            generation: uuid::Uuid::new_v4(),
            schema: 3,
            id: uuid::Uuid::new_v4(),
            language: Language::Python,
            version: uuid::Uuid::new_v4(),
            interface: Interface::Function {
                name: "solve".into(),
                params: vec![],
                returns: Type::Int,
            },
            limits: Limits::default(),
            mode: "run".into(),
            type_definitions: vec![],
            comparison: Comparison::default(),
        };
        let case = Case {
            args: Some(serde_json::json!([])),
            expected: Some(serde_json::json!(2)),
            hidden: false,
            constructor_args: None,
            operations: None,
        };
        assert_eq!(
            compare_case(&job, &case, Some(&serde_json::json!(3)), true).0,
            Some(Verdict::WrongAnswer)
        );
        assert_eq!(
            compare_case(&job, &case, Some(&serde_json::json!(3)), false).0,
            None
        );
        assert_eq!(
            compare_case(&job, &case, Some(&serde_json::json!("invalid")), false).0,
            Some(Verdict::RuntimeError)
        );
        assert_eq!(
            compare_case(&job, &case, Some(&Value::Null), false).0,
            Some(Verdict::RuntimeError)
        );
        job.interface = Interface::Function {
            name: "solve".into(),
            params: vec![],
            returns: Type::Nullable(Box::new(Type::Int)),
        };
        assert_eq!(compare_case(&job, &case, Some(&Value::Null), false).0, None);
        assert_eq!(
            compare_case(&job, &case, None, false).0,
            Some(Verdict::RuntimeError)
        );
    }

    #[tokio::test]
    #[ignore = "requires a configured gVisor sandbox"]
    async fn reference_reports_success_and_wrong_answers() -> Result<()> {
        let job = Job {
            attempt_base: 0,
            generation: uuid::Uuid::new_v4(),
            schema: 3,
            id: uuid::Uuid::new_v4(),
            language: Language::Python,
            version: uuid::Uuid::new_v4(),
            interface: Interface::Function {
                name: "solve".into(),
                params: vec![],
                returns: Type::Int,
            },
            limits: Limits::default(),
            mode: "run".into(),
            type_definitions: vec![],
            comparison: Comparison::default(),
        };
        let case = Case {
            args: Some(serde_json::json!([])),
            expected: Some(serde_json::json!(2)),
            hidden: true,
            constructor_args: None,
            operations: None,
        };
        assert_eq!(
            reference_outputs(&job, "def solve(): return 2", std::slice::from_ref(&case)).await?,
            vec![serde_json::json!(2)]
        );
        anyhow::ensure!(
            reference_outputs(&job, "def solve(): return 3", &[case])
                .await
                .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires a configured Kubernetes sandbox cluster"]
    async fn compiles_a_minimal_cpp_program() -> Result<()> {
        let backend = Backend::from_env().await?;
        let (_trigger, shutdown) = crate::shutdown::channel();
        let result = backend
            .execute(
                b"int main() { return 0; }",
                "solution.cpp",
                "",
                &Limits::default(),
                true,
                &shutdown,
            )
            .await?;
        let collected = result.map_err(|verdict| anyhow::anyhow!("{verdict:?}"))?;
        anyhow::ensure!(collected.exit == 0, "compiler failed: {}", collected.log);
        anyhow::ensure!(
            collected.artifact.is_some(),
            "compiler returned no artifact"
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires a configured Kubernetes sandbox cluster"]
    async fn runs_a_minimal_python_program() -> Result<()> {
        let backend = Backend::from_env().await?;
        let (_trigger, shutdown) = crate::shutdown::channel();
        let result = backend
            .execute(
                b"open('/work/result', 'w').write('1')",
                "solution.py",
                "",
                &Limits::default(),
                false,
                &shutdown,
            )
            .await?;
        let collected = result.map_err(|verdict| anyhow::anyhow!("{verdict:?}"))?;
        anyhow::ensure!(collected.exit == 0, "runner failed: {}", collected.log);
        anyhow::ensure!(
            collected.output == Some(serde_json::json!(1)),
            "unexpected output"
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires a configured Kubernetes sandbox cluster"]
    async fn judges_a_cpp_submission() -> Result<()> {
        let job = Job {
            attempt_base: 0,
            generation: uuid::Uuid::new_v4(),
            schema: 3,
            id: uuid::Uuid::new_v4(),
            language: Language::Cpp,
            version: uuid::Uuid::new_v4(),
            interface: Interface::Function {
                name: "solve".into(),
                params: vec![Parameter {
                    name: "x".into(),
                    ty: Type::Int,
                    constraints: None,
                }],
                returns: Type::Int,
            },
            limits: Limits::default(),
            mode: "submit".into(),
            type_definitions: vec![],
            comparison: Comparison::default(),
        };
        let cases = [Case {
            args: Some(serde_json::json!([1])),
            expected: Some(serde_json::json!(2)),
            hidden: false,
            constructor_args: None,
            operations: None,
        }];
        let outcome = judge(&job, "int solve(int x) { return x + 1; }", &cases).await?;
        anyhow::ensure!(
            outcome.verdict == Verdict::Accepted,
            "judge returned {outcome:?}"
        );
        Ok(())
    }

    #[test]
    fn validates_backend_selection() {
        assert_eq!(backend_kind("podman").unwrap(), "podman");
        assert_eq!(backend_kind("kubernetes").unwrap(), "kubernetes");
        assert!(backend_kind("docker").is_err());
    }

    #[tokio::test]
    async fn generated_pod_is_restricted_and_bounded() {
        let client =
            Client::try_from(kube::Config::new("https://127.0.0.1".parse().unwrap())).unwrap();
        let backend = Kubernetes {
            pods: Api::namespaced(client, "sandbox"),
            image: "registry/toolchain@sha256:deadbeef".into(),
            runtime_class: "runsc".into(),
            start_timeout: Duration::from_secs(1),
            node_selector: BTreeMap::new(),
            tolerations: vec![],
            image_pull_secrets: vec![LocalObjectReference {
                name: "ghcr-pull".into(),
            }],
        };
        let pod = backend.pod("locoder-test", "execute", vec!["true".into()], 256, 60);
        let spec = pod.spec.unwrap();
        assert_eq!(spec.runtime_class_name.as_deref(), Some("runsc"));
        assert_eq!(spec.automount_service_account_token, Some(false));
        assert_eq!(spec.image_pull_secrets.unwrap()[0].name, "ghcr-pull");
        assert_eq!(spec.host_network, None);
        assert_eq!(spec.host_pid, None);
        assert_eq!(spec.host_ipc, None);
        assert!(
            spec.volumes
                .unwrap()
                .iter()
                .all(|v| v.empty_dir.is_some() && v.host_path.is_none())
        );
        let container = &spec.containers[0];
        let security = container.security_context.as_ref().unwrap();
        assert_eq!(security.run_as_non_root, Some(true));
        assert_eq!(security.run_as_user, Some(65534));
        assert_eq!(security.allow_privilege_escalation, Some(false));
        assert_eq!(security.read_only_root_filesystem, Some(true));
        assert_eq!(security.privileged, None);
        assert_eq!(
            security.capabilities.as_ref().unwrap().drop.as_deref(),
            Some(&["ALL".into()][..])
        );
        let resources = container.resources.as_ref().unwrap();
        assert!(
            resources
                .limits
                .as_ref()
                .unwrap()
                .contains_key("ephemeral-storage")
        );
        assert!(resources.requests.as_ref().unwrap().contains_key("memory"));
        for memory in [2048, 4096] {
            let editor = backend.pod(
                "locoder-editor-test",
                "editor",
                vec!["true".into()],
                memory,
                60,
            );
            let spec = editor.spec.unwrap();
            let resources = spec.containers[0].resources.as_ref().unwrap();
            assert_eq!(
                resources.requests.as_ref().unwrap()["memory"].0,
                format!("{memory}Mi")
            );
            assert_eq!(
                resources.limits.as_ref().unwrap()["memory"].0,
                format!("{memory}Mi")
            );
            assert_eq!(
                spec.volumes
                    .as_ref()
                    .unwrap()
                    .iter()
                    .find(|v| v.name == "editor-cache")
                    .unwrap()
                    .empty_dir
                    .as_ref()
                    .unwrap()
                    .size_limit
                    .as_ref()
                    .unwrap()
                    .0,
                "1Gi"
            );
        }
    }
}
