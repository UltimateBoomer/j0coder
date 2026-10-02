//! Host-only control plane. No HTTP account/role/policy mutation endpoints.
use crate::{accounts, api, security};
use anyhow::{Context, Result, ensure};
use sqlx::{PgPool, Row};
use std::io::{self, BufRead};
use uuid::Uuid;
fn password() -> Result<String> {
    let mut p = String::new();
    io::stdin().lock().read_line(&mut p)?;
    while p.ends_with(['\n', '\r']) {
        p.pop();
    }
    Ok(p)
}
async fn id(db: &PgPool, name: &str) -> Result<Uuid> {
    sqlx::query_scalar("SELECT id FROM users WHERE username=$1")
        .bind(name)
        .fetch_optional(db)
        .await?
        .context("user not found")
}
fn arg(args: &[String], n: usize) -> Result<&str> {
    args.get(n)
        .map(String::as_str)
        .context("missing argument; run api manage help")
}
async fn audit(db: &PgPool, action: &str, user: Option<Uuid>) -> Result<()> {
    sqlx::query("INSERT INTO security_audit(action,user_id) VALUES($1,$2)")
        .bind(action)
        .bind(user)
        .execute(db)
        .await?;
    Ok(())
}
pub async fn run(db: &PgPool, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str).unwrap_or("help") {
        "help" => println!(
            "api manage users\napi manage create-user USERNAME [admin] < password\napi manage invite\napi manage reset USERNAME\napi manage suspend USERNAME | unsuspend USERNAME\napi manage role USERNAME admin|learner\napi manage revoke USERNAME\napi manage policy USERNAME FIELD VALUE|null\napi manage policy-show USERNAME\napi manage validate FILE | import FILE | save PROBLEM_ID FILE | publish PROBLEM_ID\napi manage catalog FILE\napi manage audit\nPasswords must be supplied through stdin; links printed by invite/reset are secrets."
        ),
        "users" => {
            let rows =
                sqlx::query("SELECT id,username,admin,suspended FROM users ORDER BY username")
                    .fetch_all(db)
                    .await?;
            for r in rows {
                println!(
                    "{}",
                    serde_json::json!({"id":r.get::<Uuid,_>("id"),"username":r.get::<String,_>("username"),"admin":r.get::<bool,_>("admin"),"suspended":r.get::<bool,_>("suspended")})
                );
            }
        }
        "create-user" => {
            let name = arg(args, 1)?;
            let admin = match args.get(2).map(String::as_str) {
                None => false,
                Some("admin") => true,
                _ => anyhow::bail!("expected optional admin"),
            };
            api::create_user(db, name, &password()?, admin).await?;
            audit(db, "cli_create_user", Some(id(db, name).await?)).await?;
        }
        "invite" => {
            let token = accounts::issue_token(db, "invite", None).await?;
            audit(db, "cli_invite", None).await?;
            println!(
                "{}/register#{}",
                crate::env("PUBLIC_ORIGIN", "http://localhost:8080"),
                token
            );
        }
        "reset" => {
            let user = id(db, arg(args, 1)?).await?;
            let token = accounts::issue_token(db, "reset", Some(user)).await?;
            audit(db, "cli_reset_link", Some(user)).await?;
            println!(
                "{}/reset-password#{}",
                crate::env("PUBLIC_ORIGIN", "http://localhost:8080"),
                token
            );
        }
        "suspend" | "unsuspend" | "role" => {
            let command = arg(args, 0)?;
            let user = id(db, arg(args, 1)?).await?;
            let mut tx = db.begin().await?;
            if command == "role" {
                let admin = match arg(args, 2)? {
                    "admin" => true,
                    "learner" => false,
                    _ => anyhow::bail!("expected admin|learner"),
                };
                sqlx::query(
                    "UPDATE users SET admin=$2,auth_generation=auth_generation+1 WHERE id=$1",
                )
                .bind(user)
                .bind(admin)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "UPDATE users SET suspended=$2,auth_generation=auth_generation+1 WHERE id=$1",
                )
                .bind(user)
                .bind(command == "suspend")
                .execute(&mut *tx)
                .await?;
            }
            if command == "suspend" {
                sqlx::query("UPDATE submissions SET status='completed',result=$2,updated_at=now() WHERE user_id=$1 AND status='queued'")
                    .bind(user).bind(serde_json::json!({"elapsed_ms":0,"verdict":"cancelled","passed":0,"total":0,"cases":[],"diagnostic":null})).execute(&mut *tx).await?;
            }
            sqlx::query("DELETE FROM sessions WHERE user_id=$1")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO security_audit(action,user_id) VALUES($1,$2)")
                .bind(format!("cli_{command}"))
                .bind(user)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        "revoke" => {
            accounts::revoke(db, id(db, arg(args, 1)?).await?, "cli_revoke").await?;
        }
        "policy-show" => println!(
            "{}",
            serde_json::to_string_pretty(
                &security::Policy::load(db, id(db, arg(args, 1)?).await?).await?
            )?
        ),
        "policy" => {
            let user = id(db, arg(args, 1)?).await?;
            let field = arg(args, 2)?;
            ensure!(
                [
                    "api_rate",
                    "save_rate",
                    "submissions_minute",
                    "submissions_day",
                    "pending",
                    "ticket_rate",
                    "editor_sessions",
                    "editor_messages",
                    "editor_bytes"
                ]
                .contains(&field),
                "unknown policy field"
            );
            let value = match arg(args, 3)? {
                "null" => None,
                v => Some(v.parse::<i32>()?),
            };
            let mut tx = db.begin().await?;
            sqlx::query("INSERT INTO user_policies(user_id) VALUES($1) ON CONFLICT DO NOTHING")
                .bind(user)
                .execute(&mut *tx)
                .await?;
            sqlx::query(&format!(
                "UPDATE user_policies SET {field}=$2 WHERE user_id=$1"
            ))
            .bind(user)
            .bind(value)
            .execute(&mut *tx)
            .await?;
            sqlx::query("INSERT INTO security_audit(action,user_id) VALUES($1,$2)")
                .bind(format!("cli_policy_{field}"))
                .bind(user)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        "validate" | "import" | "save" => {
            let command = arg(args, 0)?;
            let file = arg(args, if command == "save" { 2 } else { 1 })?;
            let bytes = std::fs::read(file)?;
            let (p, _) = crate::catalog::validate_problem(&bytes)?;
            if command == "validate" {
                println!("valid");
            } else if command == "save" {
                api::save_draft_service(db, arg(args, 1)?.parse()?, p)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.1))?;
                audit(db, "cli_save_problem", None).await?;
            } else {
                let draft = api::create_draft_service(db, p)
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e.1))?;
                println!("{}", serde_json::to_value(draft.0)?);
                audit(db, "cli_import_problem", None).await?;
            }
        }
        "publish" => {
            let result = api::publish_service(db, arg(args, 1)?.parse()?)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e.1))?;
            println!("{}", serde_json::to_value(result.0)?);
            audit(db, "cli_publish", None).await?;
        }
        "catalog" => {
            let r: api::CatalogSettingsUpdate =
                serde_json::from_slice(&std::fs::read(arg(args, 1)?)?)?;
            let value = api::catalog_update_service(db, r)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e.1))?;
            println!("{}", value.0);
            audit(db, "cli_catalog", None).await?;
        }
        "audit" => {
            let rows = sqlx::query(
                "SELECT occurred_at,action,user_id FROM security_audit ORDER BY id DESC LIMIT 100",
            )
            .fetch_all(db)
            .await?;
            for r in rows {
                println!(
                    "{}",
                    serde_json::json!({"at":r.get::<chrono::DateTime<chrono::Utc>,_>("occurred_at"),"action":r.get::<String,_>("action"),"user":r.get::<Option<Uuid>,_>("user_id")})
                );
            }
        }
        _ => anyhow::bail!("unknown command; run api manage help"),
    }
    Ok(())
}
