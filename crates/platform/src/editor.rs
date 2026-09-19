use crate::{
    api::{App, Error},
    contract::Language,
    sandbox::Podman,
};
use axum::{
    Json,
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
#[derive(Serialize, Deserialize)]
struct Ticket {
    user: uuid::Uuid,
    language: Language,
}
#[derive(Deserialize)]
pub struct Request {
    language: Language,
}
pub async fn ticket(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<Request>,
) -> Result<Json<Value>, Error> {
    let u = crate::api::user(&a, &h, true).await?;
    let mut c = crate::queue::connection().await?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let _: () = c
        .set_ex(
            format!("editor:ticket:{token}"),
            serde_json::to_string(&Ticket {
                user: u.id,
                language: r.language,
            })
            .unwrap(),
            30,
        )
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(Json(json!({"ticket":token,"path":"/editor/ws"})))
}
#[derive(Deserialize)]
pub struct Connect {
    ticket: String,
}
pub async fn upgrade(
    Query(q): Query<Connect>,
    h: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, Error> {
    if h.get("origin").and_then(|v| v.to_str().ok())
        != Some(crate::env("PUBLIC_ORIGIN", "http://localhost:8080").as_str())
    {
        return Err(Error(StatusCode::FORBIDDEN, "origin rejected".into()));
    }
    let mut c = crate::queue::connection().await?;
    let raw: Option<String> = redis::cmd("GETDEL")
        .arg(format!("editor:ticket:{}", q.ticket))
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let t: Ticket = serde_json::from_str(
        &raw.ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "expired ticket".into()))?,
    )
    .map_err(|e| anyhow::anyhow!(e))?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();
    let script = r#"redis.call('ZREMRANGEBYSCORE',KEYS[1],'-inf',ARGV[1]);redis.call('ZREMRANGEBYSCORE',KEYS[2],'-inf',ARGV[1]);if redis.call('ZCARD',KEYS[1])>=tonumber(ARGV[4]) or redis.call('ZCARD',KEYS[2])>=2 then return 0 end;redis.call('ZADD',KEYS[1],ARGV[2],ARGV[3]);redis.call('ZADD',KEYS[2],ARGV[2],ARGV[3]);return 1"#;
    let capacity: i32 = redis::Script::new(script)
        .key("editor:active")
        .key(format!("editor:user:{}", t.user))
        .arg(now)
        .arg(now + 360)
        .arg(&id)
        .arg(crate::env("EDITOR_CAPACITY", "8"))
        .invoke_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    if capacity == 0 {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "semantic completion at capacity".into(),
        ));
    }
    Ok(ws
        .max_message_size(1024 * 1024)
        .on_upgrade(move |socket| async move {
            let user = t.user;
            let result = bridge(socket, t, &id).await;
            if let Err(e) = result {
                tracing::warn!(error=%e,"editor session closed")
            }
            let _: redis::RedisResult<usize> = c.zrem("editor:active", &id).await;
            let _: redis::RedisResult<usize> = c.zrem(format!("editor:user:{user}"), &id).await;
        }))
}
// Only text-document operations are admitted. Workspace commands, configuration changes,
// arbitrary URIs, file operations, and client-selected executable paths are never forwarded.
fn safe_message(v: &Value, language: Language) -> bool {
    let uri = match language {
        Language::Cpp => "file:///workspace/solution.cpp",
        Language::Python => "file:///workspace/solution.py",
    };
    if let Some(method) = v.get("method").and_then(Value::as_str)
        && ![
            "initialize",
            "initialized",
            "shutdown",
            "exit",
            "$/cancelRequest",
            "textDocument/didOpen",
            "textDocument/didChange",
            "textDocument/didClose",
            "textDocument/completion",
            "completionItem/resolve",
            "textDocument/hover",
            "textDocument/signatureHelp",
            "textDocument/definition",
            "textDocument/documentSymbol",
        ]
        .contains(&method)
    {
        return false;
    }
    fn paths(v: &Value, uri: &str) -> bool {
        match v {
            Value::Object(m) => m.iter().all(|(k, v)| {
                if k == "uri" || k == "targetUri" || k == "rootUri" {
                    v.as_str()
                        .is_some_and(|s| s == uri || s == "file:///workspace")
                } else if k == "command" || k == "initializationOptions" || k == "rootPath" {
                    false
                } else {
                    paths(v, uri)
                }
            }),
            Value::Array(a) => a.iter().all(|v| paths(v, uri)),
            _ => true,
        }
    }
    paths(v, uri)
}
async fn bridge(mut socket: WebSocket, t: Ticket, session: &str) -> anyhow::Result<()> {
    let b = Podman::new();
    let name = format!("practice-editor-{session}");
    let cmd = match t.language {
        Language::Cpp => vec![
            "clangd",
            "--background-index=false",
            "--clang-tidy=false",
            "--log=error",
            "--compile-commands-dir=/workspace",
        ],
        Language::Python => vec!["pyright-langserver", "--stdio"],
    };
    let mut args = vec![
        "create",
        "--timeout=3600",
        "--name",
        &name,
        "--label=practice.editor=true",
        "--runtime",
        &b.runtime,
        "--network=none",
        "--http-proxy=false",
        "--read-only",
        "--read-only-tmpfs=false",
        "--user=65534:65534",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges",
        "--security-opt=label=disable",
        "--memory=512m",
        "--memory-swap=512m",
        "--cpus=1",
        "--pids-limit=64",
        "--tmpfs",
        "/tmp:rw,noexec,nosuid,nodev,size=32m,mode=1777",
        "--tmpfs",
        "/workspace/.cache:rw,noexec,nosuid,nodev,size=32m,mode=1777",
        "--log-driver=none",
        "--workdir=/workspace",
        "-i",
        &b.image,
    ];
    args.extend(cmd);
    let id = String::from_utf8(b.checked(&args).await?)?
        .trim()
        .to_string();
    let result = async {
        let mut process = b.command().args(["start", "-a", "-i", &id])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let mut stdin = process.stdin.take().unwrap();
        let mut stdout = BufReader::new(process.stdout.take().unwrap());
        // Frame reads must not be cancelled midway when a client message arrives.
        let (frames_tx, mut frames_rx) = tokio::sync::mpsc::channel(8);
        let reader_task = tokio::spawn(async move {
            loop {
                let frame = read_frame(&mut stdout).await;
                let failed = frame.is_err();
                if frames_tx.send(frame).await.is_err() || failed { break; }
            }
        });
        let mut activity = tokio::time::Instant::now();
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        let mut c = crate::queue::connection().await?;
        let session_result = async {
            loop {
                tokio::select! {
                    message = socket.recv() => {
                        let Some(Ok(Message::Text(text))) = message else { break; };
                        let mut value: Value = serde_json::from_str(&text)?;
                        if !safe_message(&value, t.language) {
                            if let Some(id) = value.get("id") {
                                socket.send(Message::Text(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"operation restricted"}}).to_string().into())).await?;
                            }
                            continue;
                        }
                        if value["method"] == "initialize" {
                            value["params"] = json!({"rootUri":"file:///workspace","processId":null,"capabilities":value["params"]["capabilities"]});
                        }
                        activity = tokio::time::Instant::now();
                        send_frame(&mut stdin, &value).await?;
                    }
                    frame = frames_rx.recv() => {
                        let frame = frame.ok_or_else(||anyhow::anyhow!("LSP closed"))??;
                        let value: Value = serde_json::from_slice(&frame)?;
                        if value.get("method").is_some() && value.get("id").is_some() {
                            let response=json!({"jsonrpc":"2.0","id":value["id"],"result":if value["method"]=="workspace/configuration" {json!([{}])} else {Value::Null}});
                            send_frame(&mut stdin, &response).await?;
                        } else {
                            socket.send(Message::Text(String::from_utf8(frame)?.into())).await?;
                        }
                    }
                    _ = tick.tick() => {
                        if activity.elapsed()>Duration::from_secs(300) { break; }
                        let expiry=chrono::Utc::now().timestamp()+360;
                        let renewed:i32=redis::Script::new("if not redis.call('ZSCORE',KEYS[1],ARGV[1]) or not redis.call('ZSCORE',KEYS[2],ARGV[1]) then return 0 end;redis.call('ZADD',KEYS[1],ARGV[2],ARGV[1]);redis.call('ZADD',KEYS[2],ARGV[2],ARGV[1]);return 1")
                            .key("editor:active").key(format!("editor:user:{}",t.user)).arg(session).arg(expiry).invoke_async(&mut c).await?;
                        if renewed==0 { break; }
                    }
                }
            }
            Ok::<_,anyhow::Error>(())
        }.await;
        reader_task.abort();
        let _ = process.kill().await;
        session_result
    }.await;
    let _ = b.checked(&["rm", "-f", &id]).await;
    result
}
async fn send_frame<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Value,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", bytes.len()).as_bytes())
        .await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
async fn read_frame<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
    let mut length = None;
    for _ in 0..16 {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        anyhow::ensure!(n > 0 && n < 4096, "invalid LSP header");
        if line == "\r\n" {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            length = Some(v.trim().parse::<usize>()?);
        }
    }
    let n = length.ok_or_else(|| anyhow::anyhow!("missing LSP length"))?;
    anyhow::ensure!(n <= 1024 * 1024, "LSP frame too large");
    let mut data = vec![0; n];
    reader.read_exact(&mut data).await?;
    Ok(data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_commands_and_paths() {
        assert!(!safe_message(
            &json!({"method":"workspace/executeCommand"}),
            Language::Cpp
        ));
        assert!(!safe_message(
            &json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///etc/passwd"}}}),
            Language::Cpp
        ));
        assert!(safe_message(
            &json!({"method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///workspace/solution.cpp"}}}),
            Language::Cpp
        ));
    }
}

#[cfg(test)]
mod framing_tests {
    use super::*;
    #[tokio::test]
    async fn reads_fragmented_unicode_frame() {
        let (mut tx, mut rx) = tokio::io::duplex(1024);
        let bytes = br#"{"jsonrpc":"2.0","method":"initialized"}"#;
        let n = bytes.len();
        tokio::spawn(async move {
            tx.write_all(format!("Content-Length: {n}\r\n").as_bytes())
                .await
                .unwrap();
            tokio::task::yield_now().await;
            tx.write_all(b"\r\n").await.unwrap();
            tx.write_all(bytes).await.unwrap();
        });
        assert_eq!(
            read_frame(&mut BufReader::new(&mut rx)).await.unwrap(),
            bytes
        );
    }
    #[tokio::test]
    async fn rejects_oversized_frame() {
        let input = b"Content-Length: 1048577\r\n\r\n";
        assert!(read_frame(&mut &input[..]).await.is_err());
    }
}
