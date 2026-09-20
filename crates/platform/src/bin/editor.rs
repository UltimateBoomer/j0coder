#[tokio::main]
async fn main() -> anyhow::Result<()> {
    locoder::logging();
    locoder::sandbox::Podman::new().preflight().await?;
    let shutdown = locoder::shutdown::signal();
    let router = axum::Router::new()
        .route("/editor/ws", axum::routing::get(locoder::editor::upgrade))
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .with_state(locoder::editor::EditorState::new(shutdown.clone()));
    let server_shutdown = shutdown.clone();
    axum::serve(
        tokio::net::TcpListener::bind(locoder::env("EDITOR_BIND", "0.0.0.0:8081")).await?,
        router,
    )
    .with_graceful_shutdown(async move { server_shutdown.cancelled().await })
    .await?;
    Ok(())
}
