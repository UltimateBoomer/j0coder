use crate::contract::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
#[derive(Clone)]
pub struct Podman {
    pub image: String,
    pub runtime: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Collected {
    pub exit: i32,
    pub output: Option<serde_json::Value>,
    pub log: String,
    pub overflow: bool,
    #[serde(default)]
    pub artifact: Option<String>,
}
#[allow(async_fn_in_trait)]
pub trait SandboxBackend {
    async fn create(
        &self,
        limits: &Limits,
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
                .await?;
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
    pub async fn execute(
        &self,
        source: &[u8],
        name: &str,
        args: &str,
        limits: &Limits,
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<std::result::Result<Collected, Verdict>> {
        let id = self.create(limits, compile, shutdown).await?;
        let guard = Cleanup(self.clone(), id.clone());
        let operation = async {
            self.copy_bytes(&id, name, source).await?;
            let result = self
                .start(
                    &id,
                    args,
                    Duration::from_millis(if compile { 30000 } else { limits.time_ms }),
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
        compile: bool,
        shutdown: &crate::shutdown::Shutdown,
    ) -> Result<String> {
        let name = format!("locoder-{}", uuid::Uuid::new_v4());
        let memory = if compile { 1024 } else { l.memory_mib };
        let timeout = if compile {
            35
        } else {
            l.time_ms.div_ceil(1000) + 3
        }
        .to_string();
        let memory = format!("{memory}m");
        let output_bytes = l.output_bytes.to_string();
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
    let b = Podman::new();
    let wrapped = match (&job.signature, &job.interface) {
        (Some(signature), _) => job.language.wrapper(signature, source)?,
        (_, Some(interface)) => job.language.wrapper_interface(interface, source)?,
        _ => anyhow::bail!("job has no interface"),
    };
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
                let (verdict, detail) = compare_case(job, case, c.output.as_ref());
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
) -> (Option<Verdict>, Option<String>) {
    if let Some(sig) = &job.signature {
        let Some(actual) = output else {
            return (Some(Verdict::RuntimeError), None);
        };
        if !sig.returns.valid_with(actual, &job.type_definitions) {
            return (Some(Verdict::RuntimeError), None);
        }
        return (
            case.expected.as_ref().map(|e| {
                if job.comparison.matches(&sig.returns, e, actual) {
                    Verdict::Accepted
                } else {
                    Verdict::WrongAnswer
                }
            }),
            None,
        );
    }
    match job.interface.as_ref() {
        None => (Some(Verdict::RuntimeError), None),
        Some(crate::contract::Interface::Function { returns, .. }) => {
            let Some(actual) = output else {
                return (Some(Verdict::RuntimeError), None);
            };
            if !returns.valid_with(actual, &job.type_definitions) {
                (Some(Verdict::RuntimeError), None)
            } else {
                (
                    case.expected.as_ref().map(|e| {
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
        Some(interface @ crate::contract::Interface::DataStructure { .. }) => {
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
                if !job.comparison.matches(&method.returns, &op.expected, value) {
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
