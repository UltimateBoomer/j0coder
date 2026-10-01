use j0coder::api;

/// Run against a disposable PostgreSQL server with BOOTSTRAP_TEST_DATABASE_URL.
#[tokio::test]
#[ignore = "requires disposable PostgreSQL; creates a private schema"]
async fn serialized_bootstrap_and_restored_account_states() -> anyhow::Result<()> {
    let url = std::env::var("BOOTSTRAP_TEST_DATABASE_URL")?;
    let schema = format!("bootstrap_{}", uuid::Uuid::new_v4().simple());
    let admin_db = sqlx::PgPool::connect(&url).await?;
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin_db)
        .await?;
    let search_path = schema.clone();
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let command = format!("SET search_path TO {search_path}");
            Box::pin(async move {
                sqlx::query(&command).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await?;
    sqlx::query("CREATE TABLE users (id uuid PRIMARY KEY, username text UNIQUE NOT NULL, password text NOT NULL, admin boolean NOT NULL)").execute(&db).await?;
    assert_eq!(api::bootstrap_status(&db).await?, "empty");
    let (first, second) = tokio::join!(
        api::bootstrap_if_empty(&db, "first", "test password 123"),
        api::bootstrap_if_empty(&db, "second", "test password 456")
    );
    let outcomes = [first?, second?];
    assert!(outcomes.contains(&"created"));
    assert!(outcomes.contains(&"administrator-present"));
    assert_eq!(api::bootstrap_status(&db).await?, "administrator-present");
    assert_eq!(
        api::bootstrap_if_empty(&db, "ignored", "invalid").await?,
        "administrator-present"
    );
    sqlx::query("UPDATE users SET admin=false")
        .execute(&db)
        .await?;
    assert_eq!(api::bootstrap_status(&db).await?, "recovery-required");
    assert!(
        api::bootstrap_if_empty(&db, "new", "test password 123")
            .await
            .is_err()
    );
    db.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_db)
        .await?;
    Ok(())
}
