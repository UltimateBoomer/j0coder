use j0coder::api;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    j0coder::logging();
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "migrate") {
        sqlx::migrate!("../../migrations").run(&db).await?;
        return Ok(());
    }
    if j0coder::env("MIGRATE_ON_START", "true") == "true" {
        sqlx::migrate!("../../migrations").run(&db).await?;
    }
    if args.get(1).is_some_and(|a| a == "bootstrap-status") {
        println!("{}", api::bootstrap_status(&db).await?);
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "bootstrap-admin") {
        let username = args.get(2).ok_or_else(|| {
            anyhow::anyhow!("usage: api bootstrap-admin USERNAME; password read from stdin")
        })?;
        let mut password = String::new();
        std::io::stdin().read_line(&mut password)?;
        let password = password.strip_suffix('\n').unwrap_or(&password);
        if args.get(3).is_some_and(|a| a == "--if-empty") {
            println!(
                "{}",
                api::bootstrap_if_empty(&db, username, password).await?
            );
        } else {
            api::create_user(&db, username, password, true).await?;
        }
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "manage") {
        return j0coder::manage::run(&db, &args[2..]).await;
    }
    anyhow::ensure!(
        ["invite", "closed"].contains(&j0coder::env("REGISTRATION_MODE", "invite").as_str()),
        "invalid REGISTRATION_MODE"
    );
    j0coder::security::Policy::deployment_defaults()?;
    let app = api::App {
        db: db.clone(),
        origin: j0coder::env("PUBLIC_ORIGIN", "http://localhost:8080"),
        secure: j0coder::env("COOKIE_SECURE", "true") == "true",
        private: false,
    };
    let shutdown = j0coder::shutdown::signal();
    let maintenance_db = db.clone();
    let maintenance_shutdown = shutdown.clone();
    let maintenance = tokio::spawn(async move {
        loop {
            if maintenance_shutdown.is_cancelled() {
                break;
            }
            if let Err(error) = j0coder::security::refresh_editors(&maintenance_db).await {
                tracing::warn!(%error,"authorization refresh unavailable")
            }
            tokio::select! {_=maintenance_shutdown.cancelled()=>break,_=tokio::time::sleep(std::time::Duration::from_secs(10))=>{}}
        }
    });
    let admin_task = if j0coder::env("WEB_ADMIN_ENABLED", "false") == "true" {
        let origin = j0coder::env("ADMIN_ORIGIN", "http://localhost:8082");
        let private = api::App {
            db: db.clone(),
            secure: origin.starts_with("https://"),
            origin,
            private: true,
        };
        let listener =
            tokio::net::TcpListener::bind(j0coder::env("ADMIN_BIND", "127.0.0.1:8082")).await?;
        let signal = shutdown.clone();
        Some(tokio::spawn(async move {
            axum::serve(
                listener,
                api::router(private).into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async move { signal.cancelled().await })
            .await
        }))
    } else {
        None
    };
    let dispatcher = tokio::spawn(j0coder::queue::dispatch_forever(
        db.clone(),
        shutdown.clone(),
    ));
    let events = tokio::spawn(j0coder::queue::events_forever(db, shutdown.clone()));
    let listener = tokio::net::TcpListener::bind(j0coder::env("API_BIND", "0.0.0.0:8080")).await?;
    let server_shutdown = shutdown.clone();
    axum::serve(
        listener,
        api::router(app).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move { server_shutdown.cancelled().await })
    .await?;
    dispatcher.await?;
    events.await?;
    maintenance.await?;
    if let Some(task) = admin_task {
        task.await??;
    }
    Ok(())
}
