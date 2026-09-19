use crate::contract::*;
use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;
#[derive(Clone)]
pub struct App {
    pub db: PgPool,
    pub origin: String,
    pub secure: bool,
}
#[derive(Debug)]
pub struct Error(pub StatusCode, pub String);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error=%e,"request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "service temporarily unavailable".into(),
        )
    }
}
impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        anyhow::Error::from(e).into()
    }
}
type Result<T> = std::result::Result<T, Error>;
fn bad(s: &str) -> Error {
    Error(StatusCode::BAD_REQUEST, s.into())
}
fn deny() -> Error {
    Error(StatusCode::UNAUTHORIZED, "authentication required".into())
}
#[derive(Serialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub admin: bool,
    pub csrf: String,
}
pub async fn user(a: &App, h: &HeaderMap, write: bool) -> Result<User> {
    let sid = h
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|s| s.trim().strip_prefix("practice_session="))
        })
        .ok_or_else(deny)?;
    let row=sqlx::query("SELECT u.id,u.username,u.admin,s.csrf FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.id=$1 AND s.expires>now()").bind(sid).fetch_optional(&a.db).await?.ok_or_else(deny)?;
    let u = User {
        id: row.get("id"),
        username: row.get("username"),
        admin: row.get("admin"),
        csrf: row.get("csrf"),
    };
    if write {
        origin(a, h)?;
        if h.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(&u.csrf) {
            return Err(Error(StatusCode::FORBIDDEN, "invalid CSRF token".into()));
        }
    }
    Ok(u)
}
fn origin(a: &App, h: &HeaderMap) -> Result<()> {
    if h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(&a.origin) {
        return Err(Error(StatusCode::FORBIDDEN, "origin rejected".into()));
    }
    Ok(())
}
async fn admin(a: &App, h: &HeaderMap, write: bool) -> Result<User> {
    let u = user(a, h, write).await?;
    if !u.admin {
        return Err(Error(
            StatusCode::FORBIDDEN,
            "administrator required".into(),
        ));
    }
    Ok(u)
}
pub fn hash_password(p: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        (12..=256).contains(&p.len()),
        "password must be 12–256 bytes"
    );
    Ok(Argon2::default()
        .hash_password(p.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}
pub async fn create_user(
    db: &PgPool,
    name: &str,
    password: &str,
    is_admin: bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
        "invalid username"
    );
    let hash = hash_password(password)?;
    sqlx::query("INSERT INTO users(id,username,password,admin) VALUES($1,$2,$3,$4)")
        .bind(Uuid::new_v4())
        .bind(name)
        .bind(hash)
        .bind(is_admin)
        .execute(db)
        .await?;
    Ok(())
}
#[derive(Deserialize)]
struct Login {
    username: String,
    password: String,
}
async fn login(State(a): State<App>, h: HeaderMap, Json(r): Json<Login>) -> Result<Response> {
    origin(&a, &h)?;
    if r.password.len() > 256 || r.username.len() > 64 {
        return Err(deny());
    }
    let row = sqlx::query("SELECT id,password FROM users WHERE username=$1")
        .bind(&r.username)
        .fetch_optional(&a.db)
        .await?;
    let fallback = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$DQ8ZWIVg7t/OTDtfO+VEcsRUlAYgUbqXB6Dl7AeQWUo";
    let hash = row
        .as_ref()
        .map(|r| r.get::<String, _>("password"))
        .unwrap_or(fallback.into());
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(r.password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))?;
    if !valid || row.is_none() {
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        return Err(deny());
    }
    let sid = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let csrf = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO sessions(id,user_id,csrf) VALUES($1,$2,$3)")
        .bind(&sid)
        .bind(row.unwrap().get::<Uuid, _>("id"))
        .bind(csrf)
        .execute(&a.db)
        .await?;
    let cookie = format!(
        "practice_session={sid}; Path=/; HttpOnly; SameSite=Strict; Max-Age=604800{}",
        if a.secure { "; Secure" } else { "" }
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({"ok":true}))).into_response())
}
async fn me(State(a): State<App>, h: HeaderMap) -> Result<Json<User>> {
    Ok(Json(user(&a, &h, false).await?))
}
async fn logout(State(a): State<App>, h: HeaderMap) -> Result<Response> {
    let u = user(&a, &h, true).await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND csrf=$2")
        .bind(u.id)
        .bind(u.csrf)
        .execute(&a.db)
        .await?;
    Ok((
        [(
            header::SET_COOKIE,
            "practice_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    )
        .into_response())
}
async fn add_user(State(a): State<App>, h: HeaderMap, Json(r): Json<Login>) -> Result<StatusCode> {
    admin(&a, &h, true).await?;
    create_user(&a.db, &r.username, &r.password, false)
        .await
        .map_err(|e| bad(&e.to_string()))?;
    Ok(StatusCode::CREATED)
}
#[derive(Deserialize)]
struct Filters {
    q: Option<String>,
    difficulty: Option<String>,
    tag: Option<String>,
}
async fn problems(
    State(a): State<App>,
    h: HeaderMap,
    Query(f): Query<Filters>,
) -> Result<Json<Value>> {
    user(&a, &h, false).await?;
    let rows=sqlx::query("SELECT p.id,v.id AS version,v.public FROM problems p JOIN versions v ON v.id=p.current_version WHERE ($1='' OR v.public->>'title' ILIKE '%'||$1||'%') AND ($2='' OR v.public->>'difficulty'=$2) AND ($3='' OR v.public->'tags' ? $3) ORDER BY v.public->>'title' LIMIT 500").bind(f.q.unwrap_or_default()).bind(f.difficulty.unwrap_or_default()).bind(f.tag.unwrap_or_default()).fetch_all(&a.db).await?;
    Ok(Json(Value::Array(rows.iter().map(|r|{let p:Value=r.get("public");json!({"id":r.get::<Uuid,_>("id"),"version":r.get::<Uuid,_>("version"),"title":p["title"],"difficulty":p["difficulty"],"tags":p["tags"]})}).collect())))
}
async fn problem(State(a): State<App>, h: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    user(&a, &h, false).await?;
    let r=sqlx::query("SELECT v.id,v.public FROM problems p JOIN versions v ON v.id=p.current_version WHERE p.id=$1").bind(id).fetch_optional(&a.db).await?.ok_or_else(||Error(StatusCode::NOT_FOUND,"problem not found".into()))?;
    let p: Value = r.get("public");
    let sig: Signature =
        serde_json::from_value(p["signature"].clone()).map_err(|e| anyhow::anyhow!(e))?;
    Ok(Json(
        json!({"id":id,"version":r.get::<Uuid,_>("id"),"problem":p,"starters":{"cpp":sig.starter(Language::Cpp),"python":sig.starter(Language::Python)}}),
    ))
}
async fn drafts(State(a): State<App>, h: HeaderMap) -> Result<Json<Value>> {
    admin(&a, &h, false).await?;
    let rows =
        sqlx::query("SELECT id,draft,current_version FROM problems ORDER BY draft->>'title'")
            .fetch_all(&a.db)
            .await?;
    Ok(Json(json!(rows.iter().map(|r|json!({"id":r.get::<Uuid,_>("id"),"draft":r.get::<Value,_>("draft"),"version":r.get::<Option<Uuid>,_>("current_version")})).collect::<Vec<_>>())))
}
async fn new_draft(
    State(a): State<App>,
    h: HeaderMap,
    Json(p): Json<Problem>,
) -> Result<Json<Value>> {
    admin(&a, &h, true).await?;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO problems(id,draft) VALUES($1,$2)")
        .bind(id)
        .bind(json!(p))
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"id":id})))
}
async fn save_draft(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
    Json(p): Json<Problem>,
) -> Result<StatusCode> {
    admin(&a, &h, true).await?;
    let r = sqlx::query("UPDATE problems SET draft=$2 WHERE id=$1")
        .bind(id)
        .bind(json!(p))
        .execute(&a.db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(Error(StatusCode::NOT_FOUND, "draft not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn publish(State(a): State<App>, h: HeaderMap, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    admin(&a, &h, true).await?;
    let mut tx = a.db.begin().await?;
    let p: Value = sqlx::query_scalar("SELECT draft FROM problems WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| bad("unknown draft"))?;
    let mut p: Problem = serde_json::from_value(p).map_err(|e| bad(&e.to_string()))?;
    p.validate().map_err(|e| bad(&e.to_string()))?;
    let version = Uuid::new_v4();
    let tests = json!(p.tests);
    p.tests.retain(|t| !t.hidden);
    sqlx::query("INSERT INTO versions(id,problem_id,public,tests) VALUES($1,$2,$3,$4)")
        .bind(version)
        .bind(id)
        .bind(json!(p))
        .bind(tests)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE problems SET current_version=$2 WHERE id=$1")
        .bind(id)
        .bind(version)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"version":version})))
}
#[derive(Deserialize, Serialize)]
struct Submit {
    version: Uuid,
    language: Language,
    source: String,
    mode: String,
    #[serde(default)]
    cases: Option<Vec<Case>>,
}
async fn submit(State(a): State<App>, h: HeaderMap, Json(r): Json<Submit>) -> Result<Response> {
    let u = user(&a, &h, true).await?;
    if r.source.len() > 100000 || !["run", "submit"].contains(&r.mode.as_str()) {
        return Err(bad("invalid source or mode"));
    }
    let key = h
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| bad("Idempotency-Key required"))?;
    let hash = hex::encode(Sha256::digest(serde_json::to_vec(&r).unwrap()));
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("submit:{}", u.id))
        .execute(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT id,request_hash FROM submissions WHERE user_id=$1 AND idempotency_key=$2",
    )
    .bind(u.id)
    .bind(key)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.get::<String, _>("request_hash") != hash {
            return Err(Error(
                StatusCode::CONFLICT,
                "idempotency key reused with different request".into(),
            ));
        }
        return Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":row.get::<Uuid,_>("id")})),
        )
            .into_response());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM submissions WHERE user_id=$1 AND status!='completed'",
    )
    .bind(u.id)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 10 {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "too many pending submissions".into(),
        ));
    }
    let row = sqlx::query("SELECT public FROM versions WHERE id=$1")
        .bind(r.version)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| bad("unknown version"))?;
    let p: Problem = serde_json::from_value(row.get("public")).map_err(|e| anyhow::anyhow!(e))?;
    let id = Uuid::new_v4();
    let custom_cases = if r.mode == "run" {
        let mut cases = r.cases.clone().unwrap_or(p.tests);
        if cases.is_empty() || cases.len() > 20 {
            return Err(bad("run requires 1–20 cases"));
        }
        for c in &mut cases {
            c.hidden = false;
            if !p.signature.args_valid(&c.args)
                || c.expected
                    .as_ref()
                    .is_some_and(|v| !p.signature.returns.valid(v))
            {
                return Err(bad("invalid typed arguments or expected value"));
            }
        }
        Some(json!(cases))
    } else {
        if r.cases.is_some() {
            return Err(bad("custom cases only allowed for run"));
        }
        None
    };
    let job = Job {
        attempt_base: 0,
        generation: Uuid::new_v4(),
        schema: 1,
        id,
        language: r.language,
        version: r.version,
        signature: p.signature,
        limits: p.limits,
        mode: r.mode,
    };
    sqlx::query("INSERT INTO submissions(id,user_id,version_id,idempotency_key,request_hash,source,custom_cases,job) VALUES($1,$2,$3,$4,$5,$6,$7,$8)").bind(id).bind(u.id).bind(r.version).bind(key).bind(hash).bind(r.source).bind(custom_cases).bind(json!(job)).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO outbox(submission_id) VALUES($1)")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(json!({"id":id}))).into_response())
}
async fn submission(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let u = user(&a, &h, false).await?;
    let r = sqlx::query(
        "SELECT id,version_id,status,result,created_at FROM submissions WHERE id=$1 AND user_id=$2",
    )
    .bind(id)
    .bind(u.id)
    .fetch_optional(&a.db)
    .await?
    .ok_or_else(|| Error(StatusCode::NOT_FOUND, "submission not found".into()))?;
    Ok(Json(
        json!({"id":r.get::<Uuid,_>("id"),"version":r.get::<Uuid,_>("version_id"),"status":r.get::<String,_>("status"),"result":r.get::<Option<Value>,_>("result")}),
    ))
}
#[derive(Deserialize)]
struct History {
    version: Uuid,
}
async fn history(
    State(a): State<App>,
    h: HeaderMap,
    Query(q): Query<History>,
) -> Result<Json<Value>> {
    let u = user(&a, &h, false).await?;
    let rows=sqlx::query("SELECT id,status,result,created_at,job->>'language' AS language,job->>'mode' AS mode FROM submissions WHERE user_id=$1 AND version_id=$2 ORDER BY created_at DESC LIMIT 50").bind(u.id).bind(q.version).fetch_all(&a.db).await?;
    Ok(Json(json!(rows.iter().map(|r|json!({"id":r.get::<Uuid,_>("id"),"status":r.get::<String,_>("status"),"result":r.get::<Option<Value>,_>("result"),"language":r.get::<String,_>("language"),"mode":r.get::<String,_>("mode"),"created_at":r.get::<chrono::DateTime<chrono::Utc>,_>("created_at")})).collect::<Vec<_>>())))
}
async fn ready(State(a): State<App>) -> Result<Json<Value>> {
    sqlx::query("SELECT 1").execute(&a.db).await?;
    let mut c = crate::queue::connection().await?;
    let _: String = redis::cmd("PING")
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(Json(json!({"ready":true})))
}
async fn metrics(State(a): State<App>, h: HeaderMap) -> Result<String> {
    admin(&a, &h, false).await?;
    let rows = sqlx::query("SELECT status,count(*) AS n FROM submissions GROUP BY status")
        .fetch_all(&a.db)
        .await?;
    let mut output = String::new();
    for r in rows {
        output.push_str(&format!(
            "practice_submissions{{status=\"{}\"}} {}\n",
            r.get::<String, _>("status"),
            r.get::<i64, _>("n")
        ));
    }
    let rows=sqlx::query("SELECT result->>'verdict' AS verdict,count(*) AS n,coalesce(sum((result->>'elapsed_ms')::bigint),0)::bigint AS elapsed FROM submissions WHERE status='completed' GROUP BY result->>'verdict'").fetch_all(&a.db).await?;
    for r in rows {
        let verdict: String = r.get("verdict");
        output.push_str(&format!("practice_executions_total{{verdict=\"{verdict}\"}} {}\npractice_execution_seconds_total{{verdict=\"{verdict}\"}} {}\n",r.get::<i64,_>("n"),r.get::<i64,_>("elapsed") as f64/1000.0));
    }
    let mut c = crate::queue::connection().await?;
    let _: usize = redis::cmd("ZREMRANGEBYSCORE")
        .arg("editor:active")
        .arg("-inf")
        .arg(chrono::Utc::now().timestamp())
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let active: usize = redis::cmd("ZCARD")
        .arg("editor:active")
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    output.push_str(&format!("practice_editor_sessions {active}\n"));
    Ok(output)
}
pub fn router(a: App) -> Router {
    Router::new().route("/healthz",get(||async{"ok"})).route("/readyz",get(ready)).route("/api/v1/openapi.json",get(||async{([(header::CONTENT_TYPE,"application/json")],include_str!("../../../openapi.json"))})).route("/metrics",get(metrics)).route("/api/v1/session",get(me).post(login).delete(logout)).route("/api/v1/problems",get(problems)).route("/api/v1/problems/{id}",get(problem)).route("/api/v1/admin/users",post(add_user)).route("/api/v1/admin/problems",get(drafts).post(new_draft)).route("/api/v1/admin/problems/{id}",put(save_draft)).route("/api/v1/admin/problems/{id}/publish",post(publish)).route("/api/v1/submissions",get(history).post(submit)).route("/api/v1/submissions/{id}",get(submission)).route("/api/v1/editor-ticket",post(crate::editor::ticket)).fallback_service(tower_http::services::ServeDir::new(crate::env("WEB_DIR","web/dist")).not_found_service(tower_http::services::ServeFile::new(format!("{}/index.html",crate::env("WEB_DIR","web/dist"))))).layer(DefaultBodyLimit::max(2*1024*1024)).layer(axum::middleware::from_fn(|req:axum::extract::Request,next:axum::middleware::Next|async move {let mut response=next.run(req).await;let h=response.headers_mut();h.insert("x-content-type-options","nosniff".parse().unwrap());h.insert("referrer-policy","same-origin".parse().unwrap());h.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; worker-src 'self' blob:; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'".parse().unwrap());response})).with_state(a)
}
