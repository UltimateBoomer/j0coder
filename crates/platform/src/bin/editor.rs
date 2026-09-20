#[tokio::main]
async fn main() -> anyhow::Result<()> {
    practice::logging();
    practice::sandbox::Podman::new().preflight().await?;
    let shutdown = practice::shutdown::signal();
    let router = axum::Router::new()
        .route("/editor/ws", axum::routing::get(practice::editor::upgrade))
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .with_state(practice::editor::EditorState::new(shutdown.clone()));
    let server_shutdown = shutdown.clone();
    axum::serve(
        tokio::net::TcpListener::bind(practice::env("EDITOR_BIND", "0.0.0.0:8081")).await?,
        router,
    )
    .with_graceful_shutdown(async move { server_shutdown.cancelled().await })
    .await?;
    Ok(())
}
