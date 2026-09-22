//! Same-origin WebSocket bridge for deployments accessed through the API service.
//! The editor service still validates the origin and consumes the one-use ticket.
use crate::api::Error;
use axum::{
    extract::{
        Query, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::time::Duration;
use tokio_tungstenite::tungstenite::{Message as UpstreamMessage, client::IntoClientRequest};

#[derive(Deserialize)]
pub struct Connect {
    ticket: String,
}

pub async fn upgrade(
    Query(query): Query<Connect>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, Error> {
    let origin = crate::env("PUBLIC_ORIGIN", "http://localhost:8080");
    if headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        != Some(&origin)
    {
        return Err(Error(StatusCode::FORBIDDEN, "origin rejected".into()));
    }
    if query.ticket.len() != 64 || !query.ticket.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error(
            StatusCode::BAD_REQUEST,
            "invalid editor ticket".into(),
        ));
    }
    let upstream = crate::env("EDITOR_UPSTREAM_WS", "ws://localhost:8081/editor/ws");
    let mut request = format!("{upstream}?ticket={}", query.ticket)
        .into_client_request()
        .map_err(|_| Error(StatusCode::SERVICE_UNAVAILABLE, "editor unavailable".into()))?;
    request.headers_mut().insert(
        header::ORIGIN,
        origin.parse().map_err(|_| {
            Error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid public origin".into(),
            )
        })?,
    );
    let (editor, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(request),
    )
    .await
    .map_err(|_| {
        Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "editor connection timed out".into(),
        )
    })?
    .map_err(|error| {
        tracing::warn!(%error, "editor upstream connection failed");
        Error(StatusCode::SERVICE_UNAVAILABLE, "editor unavailable".into())
    })?;
    Ok(ws
        .max_message_size(1024 * 1024)
        .on_upgrade(move |browser| bridge(browser, editor)))
}

async fn bridge(
    browser: WebSocket,
    editor: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) {
    let (mut browser_tx, mut browser_rx) = browser.split();
    let (mut editor_tx, mut editor_rx) = editor.split();
    loop {
        tokio::select! {
            incoming = browser_rx.next() => {
                let message = match incoming {
                    Some(Ok(Message::Text(text))) => UpstreamMessage::Text(text.to_string().into()),
                    Some(Ok(Message::Binary(bytes))) => UpstreamMessage::Binary(bytes),
                    Some(Ok(Message::Ping(bytes))) => UpstreamMessage::Ping(bytes),
                    Some(Ok(Message::Pong(bytes))) => UpstreamMessage::Pong(bytes),
                    _ => break,
                };
                if editor_tx.send(message).await.is_err() { break; }
            }
            incoming = editor_rx.next() => {
                let message = match incoming {
                    Some(Ok(UpstreamMessage::Text(text))) => Message::Text(text.to_string().into()),
                    Some(Ok(UpstreamMessage::Binary(bytes))) => Message::Binary(bytes),
                    Some(Ok(UpstreamMessage::Ping(bytes))) => Message::Ping(bytes),
                    Some(Ok(UpstreamMessage::Pong(bytes))) => Message::Pong(bytes),
                    _ => break,
                };
                if browser_tx.send(message).await.is_err() { break; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a live editor ticket and API port-forward"]
    async fn initializes_lsp_through_api() -> anyhow::Result<()> {
        let ticket = std::env::var("EDITOR_TEST_TICKET")?;
        let origin = "http://localhost:8080";
        let mut request =
            format!("ws://localhost:8080/editor/ws?ticket={ticket}").into_client_request()?;
        request
            .headers_mut()
            .insert(header::ORIGIN, origin.parse()?);
        let (mut socket, _) = tokio_tungstenite::connect_async(request).await?;
        socket
            .send(UpstreamMessage::Text(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": {"capabilities": {}, "rootUri": "file:///workspace"}
                })
                .to_string()
                .into(),
            ))
            .await?;
        let response = tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let message = socket
                    .next()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("WebSocket closed"))??;
                if let UpstreamMessage::Text(text) = message {
                    let value: serde_json::Value = serde_json::from_str(&text)?;
                    if value["id"] == 1 {
                        break Ok::<_, anyhow::Error>(value);
                    }
                }
            }
        })
        .await??;
        anyhow::ensure!(
            response.get("result").is_some(),
            "LSP initialization failed: {response}"
        );
        socket.close(None).await?;
        Ok(())
    }
}
