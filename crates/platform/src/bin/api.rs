use locoder::api;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    locoder::logging();
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "migrate") {
        sqlx::migrate!("../../migrations").run(&db).await?;
        return Ok(());
    }
    if locoder::env("MIGRATE_ON_START", "true") == "true" {
        sqlx::migrate!("../../migrations").run(&db).await?;
    }
    if args.get(1).is_some_and(|a| a == "bootstrap-admin") {
        let username = args.get(2).ok_or_else(|| {
            anyhow::anyhow!("usage: api bootstrap-admin USERNAME; password read from stdin")
        })?;
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        api::create_user(&db, username, password.trim_end(), true).await?;
        return Ok(());
    }
    let app = api::App {
        db: db.clone(),
        origin: locoder::env("PUBLIC_ORIGIN", "http://localhost:8080"),
        secure: locoder::env("COOKIE_SECURE", "true") == "true",
    };
    let shutdown = locoder::shutdown::signal();
    let dispatcher = tokio::spawn(locoder::queue::dispatch_forever(
        db.clone(),
        shutdown.clone(),
    ));
    let events = tokio::spawn(locoder::queue::events_forever(db, shutdown.clone()));
    let listener = tokio::net::TcpListener::bind(locoder::env("API_BIND", "0.0.0.0:8080")).await?;
    let server_shutdown = shutdown.clone();
    axum::serve(listener, api::router(app))
        .with_graceful_shutdown(async move { server_shutdown.cancelled().await })
        .await?;
    dispatcher.await?;
    events.await?;
    Ok(())
}
