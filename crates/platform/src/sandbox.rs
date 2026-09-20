use crate::contract::*;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
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
    async fn create(&self, limits: &Limits, compile: bool) -> Result<String>;
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
            image: crate::env("TOOLCHAIN_IMAGE", "localhost/practice-toolchain:1"),
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
        let path = std::env::temp_dir().join(format!("practice-{}", uuid::Uuid::new_v4()));
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
    ) -> Result<std::result::Result<Collected, Verdict>> {
        let id = self.create(limits, compile).await?;
        let guard = Cleanup(self.clone(), id.clone());
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
        let outcome = if oom {
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
    async fn create(&self, l: &Limits, compile: bool) -> Result<String> {
        let name = format!("practice-{}", uuid::Uuid::new_v4());
        let memory = if compile { 1024 } else { l.memory_mib };
        let out = self
            .checked(&[
                "create",
                "--timeout",
                &if compile {
                    35
                } else {
                    l.time_ms.div_ceil(1000) + 3
                }
                .to_string(),
                "--name",
                &name,
                "--label=practice.sandbox=true",
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
                &format!("{memory}m"),
                "--memory-swap",
                &format!("{memory}m"),
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
                &l.output_bytes.to_string(),
            ])
            .await?;
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
    let b = Podman::new();
    let wrapped = job.language.wrapper(&job.signature, source)?;
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
            )
            .await?
        {
            Ok(c) if c.exit == 0 => artifact = c.artifact,
            Ok(c) => {
                result.verdict = Verdict::CompilationError;
                result.diagnostic = Some(c.log);
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
                &serde_json::to_string(&case.args)?,
                &job.limits,
                false,
            )
            .await?;
        let (verdict, output, log) = match r {
            Err(v) => (Some(v), None, String::new()),
            Ok(c) if c.exit != 0 => (Some(Verdict::RuntimeError), None, c.log),
            Ok(c) => {
                let valid = c
                    .output
                    .as_ref()
                    .is_some_and(|v| job.signature.returns.valid(v));
                let verdict = if !valid {
                    Some(Verdict::RuntimeError)
                } else {
                    case.expected.as_ref().map(|expected| {
                        if c.output.as_ref() == Some(expected) {
                            Verdict::Accepted
                        } else {
                            Verdict::WrongAnswer
                        }
                    })
                };
                (verdict, c.output, c.log)
            }
        };
        if verdict.as_ref().is_none_or(|v| *v == Verdict::Accepted) {
            result.passed += 1
        } else if result.verdict == Verdict::Accepted {
            result.verdict = verdict.clone().unwrap()
        }
        if !case.hidden {
            result.cases.push(CaseResult {
                verdict,
                output,
                log,
            })
        }
    }
    Ok(result)
}
