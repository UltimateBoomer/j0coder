#[tokio::main]
async fn main() -> anyhow::Result<()> {
    j0coder::logging();
    let backend = j0coder::sandbox::Backend::from_env().await?;
    backend.preflight().await?;
    let shutdown = j0coder::shutdown::signal();
    let sweep_shutdown = shutdown.clone();
    tokio::spawn(async move {
        let mut previously_orphaned = std::collections::HashSet::new();
        while !sweep_shutdown.is_cancelled() {
            match j0coder::editor::cleanup_orphans(&backend, &previously_orphaned).await {
                Ok(orphaned) => previously_orphaned = orphaned,
                Err(error) => tracing::warn!(%error, "editor orphan cleanup failed"),
            }
            tokio::select! {
                _ = sweep_shutdown.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {},
            }
        }
    });
    let router = axum::Router::new()
        .route("/editor/ws", axum::routing::get(j0coder::editor::upgrade))
        .route("/healthz", axum::routing::get(|| async { "ok" }))
        .with_state(j0coder::editor::EditorState::new(shutdown.clone()));
    let server_shutdown = shutdown.clone();
    axum::serve(
        tokio::net::TcpListener::bind(j0coder::env("EDITOR_BIND", "0.0.0.0:8081")).await?,
        router,
    )
    .with_graceful_shutdown(async move { server_shutdown.cancelled().await })
    .await?;
    Ok(())
}
