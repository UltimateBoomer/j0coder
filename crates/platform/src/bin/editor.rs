#[tokio::main]
async fn main() -> anyhow::Result<()> {
    locoder::logging();
    let backend = locoder::sandbox::Backend::from_env().await?;
    backend.preflight().await?;
    let shutdown = locoder::shutdown::signal();
    let sweep_shutdown = shutdown.clone();
    tokio::spawn(async move {
        let mut previously_orphaned = std::collections::HashSet::new();
        while !sweep_shutdown.is_cancelled() {
            match locoder::editor::cleanup_orphans(&backend, &previously_orphaned).await {
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
