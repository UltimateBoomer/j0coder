#[tokio::main]
async fn main() -> anyhow::Result<()> {
    practice::logging();
    practice::sandbox::Podman::new().preflight().await?;
    let router = axum::Router::new()
        .route("/editor/ws", axum::routing::get(practice::editor::upgrade))
        .route("/healthz", axum::routing::get(|| async { "ok" }));
    axum::serve(
        tokio::net::TcpListener::bind(practice::env("EDITOR_BIND", "0.0.0.0:8081")).await?,
        router,
    )
    .await?;
    Ok(())
}
