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
use strum::IntoEnumIterator;
use uuid::Uuid;
#[derive(Clone)]
pub struct App {
    pub db: PgPool,
    pub origin: String,
    pub secure: bool,
    pub private: bool,
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
    #[serde(skip)]
    pub session: String,
    #[serde(skip)]
    pub generation: i64,
}
pub async fn user(a: &App, h: &HeaderMap, write: bool) -> Result<User> {
    let sid = h
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';').find_map(|s| {
                s.trim().strip_prefix(if a.private {
                    "practice_admin="
                } else {
                    "practice_session="
                })
            })
        })
        .ok_or_else(deny)?;
    if !crate::security::valid_token(sid) {
        return Err(deny());
    }
    let hashed = crate::security::digest(sid);
    let row=sqlx::query("UPDATE sessions s SET last_seen=now() FROM users u WHERE u.id=s.user_id AND s.id=$1 AND s.audience=$2 AND s.generation=u.auth_generation AND NOT u.suspended AND s.expires>now() AND s.last_seen>now()-CASE WHEN s.audience='admin' THEN interval '15 minutes' ELSE interval '24 hours' END RETURNING u.id,u.username,u.admin,s.csrf,s.generation,s.authenticated_at").bind(&hashed).bind(if a.private {"admin"} else {"public"}).fetch_optional(&a.db).await?.ok_or_else(deny)?;
    let u = User {
        id: row.get("id"),
        username: row.get("username"),
        admin: a.private && row.get::<bool, _>("admin"),
        csrf: row.get("csrf"),
        session: hashed,
        generation: row.get("generation"),
    };
    if write {
        origin(a, h)?;
        if h.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(&u.csrf) {
            return Err(Error(StatusCode::FORBIDDEN, "invalid CSRF token".into()));
        }
    }
    Ok(u)
}
pub(crate) fn origin(a: &App, h: &HeaderMap) -> Result<()> {
    if h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(&a.origin) {
        return Err(Error(StatusCode::FORBIDDEN, "origin rejected".into()));
    }
    Ok(())
}
pub(crate) async fn admin(a: &App, h: &HeaderMap, write: bool) -> Result<User> {
    if !a.private {
        return Err(Error(StatusCode::NOT_FOUND, "not found".into()));
    }
    let u = user(a, h, write).await?;
    if write {
        let recent: bool = sqlx::query_scalar(
            "SELECT authenticated_at>now()-interval '5 minutes' FROM sessions WHERE id=$1",
        )
        .bind(&u.session)
        .fetch_one(&a.db)
        .await?;
        if !recent {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "reauthentication required".into(),
            ));
        }
    }
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
        p.chars().count() >= 15 && p.len() <= 256,
        "password must contain at least 15 characters and at most 256 bytes"
    );
    Ok(Argon2::default()
        .hash_password(p.as_bytes(), &SaltString::generate(&mut OsRng))
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}
pub fn validate_username(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
        "invalid username"
    );
    Ok(())
}
pub async fn password_hash_async(password: String) -> Result<String> {
    let _slot = crate::security::password_slot().await?;
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| crate::security::unavailable())?
        .map_err(|_| bad("password must contain at least 15 characters and at most 256 bytes"))
}
pub async fn create_user(
    db: &PgPool,
    name: &str,
    password: &str,
    is_admin: bool,
) -> anyhow::Result<()> {
    validate_username(name)?;
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
/// Bootstrap state is derived from the database, including restored installations.
pub async fn bootstrap_status(db: &PgPool) -> anyhow::Result<&'static str> {
    let (users, admins): (i64, i64) =
        sqlx::query_as("SELECT count(*), count(*) FILTER (WHERE admin) FROM users")
            .fetch_one(db)
            .await?;
    Ok(if admins > 0 {
        "administrator-present"
    } else if users > 0 {
        "recovery-required"
    } else {
        "empty"
    })
}

pub async fn bootstrap_if_empty(
    db: &PgPool,
    name: &str,
    password: &str,
) -> anyhow::Result<&'static str> {
    let mut tx = db.begin().await?;
    // Excludes all concurrent user inserts, including ordinary account creation.
    sqlx::query("LOCK TABLE users IN EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let (users, admins): (i64, i64) =
        sqlx::query_as("SELECT count(*), count(*) FILTER (WHERE admin) FROM users")
            .fetch_one(&mut *tx)
            .await?;
    if admins > 0 {
        return Ok("administrator-present");
    }
    anyhow::ensure!(
        users == 0,
        "recovery required: existing users have no administrator"
    );
    anyhow::ensure!(
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
        "invalid username"
    );
    let hash = hash_password(password)?;
    sqlx::query("INSERT INTO users(id,username,password,admin) VALUES($1,$2,$3,true)")
        .bind(Uuid::new_v4())
        .bind(name)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok("created")
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub username: String,
    pub password: String,
}
async fn login(State(a): State<App>, h: HeaderMap, Json(r): Json<Login>) -> Result<Response> {
    origin(&a, &h)?;
    if r.password.len() > 256 || r.username.len() > 64 {
        return Err(deny());
    }
    let row = sqlx::query(
        "SELECT id,password,auth_generation,suspended,admin FROM users WHERE username=$1",
    )
    .bind(&r.username)
    .fetch_optional(&a.db)
    .await?;
    let fallback = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$DQ8ZWIVg7t/OTDtfO+VEcsRUlAYgUbqXB6Dl7AeQWUo";
    let hash = row
        .as_ref()
        .map(|r| r.get::<String, _>("password"))
        .unwrap_or(fallback.into());
    let username_key = crate::security::digest(&r.username);
    let mut login_connection = crate::queue::connection().await?;
    let failures: Option<u32> = valkey::cmd("GET")
        .arg(format!("login:failed:{username_key}"))
        .query_async(&mut login_connection)
        .await
        .map_err(|_| crate::security::unavailable())?;
    if failures.unwrap_or(0) >= 10 {
        return Ok(crate::security::limited("login_failures", 900));
    }
    let _slot = crate::security::password_slot().await?;
    let valid = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(r.password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(|e| anyhow::anyhow!(e))?;
    if !valid
        || row.as_ref().is_none_or(|r| {
            r.get::<bool, _>("suspended") || (a.private && !r.get::<bool, _>("admin"))
        })
    {
        let _:i64=valkey::Script::new("local n=redis.call('INCR',KEYS[1]);if n==1 then redis.call('EXPIRE',KEYS[1],900) end;return n").key(format!("login:failed:{username_key}")).invoke_async(&mut login_connection).await.map_err(|_|crate::security::unavailable())?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        return Err(deny());
    }
    let sid = crate::security::token();
    let csrf = crate::security::token();
    let row = row.unwrap();
    let id: Uuid = row.get("id");
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let inserted=sqlx::query("INSERT INTO sessions(id,user_id,csrf,audience,generation,created_at,last_seen,authenticated_at,expires) SELECT $1,id,$2,$3,auth_generation,clock_timestamp(),clock_timestamp(),clock_timestamp(),clock_timestamp()+CASE WHEN $3='admin' THEN interval '1 hour' ELSE interval '7 days' END FROM users WHERE id=$4 AND NOT suspended AND auth_generation=$5")
      .bind(crate::security::digest(&sid)).bind(&csrf).bind(if a.private {"admin"} else {"public"}).bind(id).bind(row.get::<i64,_>("auth_generation")).execute(&mut *tx).await?;
    if inserted.rows_affected() == 0 {
        return Err(deny());
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1 AND id NOT IN (SELECT id FROM sessions WHERE user_id=$1 ORDER BY created_at DESC,id DESC LIMIT 5)").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    let cookie = format!(
        "{}={sid}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        if a.private {
            "practice_admin"
        } else {
            "practice_session"
        },
        if a.private { 3600 } else { 604800 },
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
            if a.private {
                "practice_admin=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
            } else {
                "practice_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
            },
        )],
        Json(json!({"ok":true})),
    )
        .into_response())
}
async fn browse(a: &App, h: &HeaderMap) -> Result<()> {
    if crate::env("GUEST_BROWSING_ENABLED", "false") == "true" && !a.private {
        return Ok(());
    }
    user(a, h, false).await?;
    Ok(())
}
#[derive(Deserialize)]
struct Filters {
    q: Option<String>,
    difficulty: Option<String>,
    tag: Option<String>,
    min_score: Option<i16>,
    max_score: Option<i16>,
    cursor: Option<String>,
    limit: Option<i64>,
}
async fn problems(
    State(a): State<App>,
    h: HeaderMap,
    Query(f): Query<Filters>,
) -> Result<Json<Vec<crate::contract::ProblemSummary>>> {
    browse(&a, &h).await?;
    let limit = f.limit.unwrap_or(100).clamp(1, 100);
    let rows=sqlx::query("SELECT p.id,v.id AS version,v.public FROM problems p JOIN versions v ON v.id=p.current_version WHERE ($1='' OR concat_ws(' ',v.public->>'title',v.public->>'summary',v.public->>'tags') ILIKE '%'||$1||'%') AND ($2='' OR v.public->>'difficulty'=$2) AND ($3='' OR v.public->'tags' ? $3) AND coalesce((v.public->>'difficulty_score')::smallint,CASE v.public->>'difficulty' WHEN 'easy' THEN 2 WHEN 'medium' THEN 3 ELSE 4 END) BETWEEN $4 AND $5 AND ($6='' OR (v.public->>'title',p.id::text)>($6,$7)) ORDER BY v.public->>'title',p.id LIMIT $8")
      .bind(f.q.unwrap_or_default()).bind(f.difficulty.unwrap_or_default()).bind(f.tag.unwrap_or_default()).bind(f.min_score.unwrap_or(1)).bind(f.max_score.unwrap_or(5)).bind(f.cursor.as_deref().and_then(|s|s.rsplit_once('|')).map(|x|x.0).unwrap_or("")).bind(f.cursor.as_deref().and_then(|s|s.rsplit_once('|')).map(|x|x.1).unwrap_or("")).bind(limit).fetch_all(&a.db).await?;
    let summaries = rows
        .iter()
        .map(|r| {
            let p: Value = r.get("public");
            let score = p.get("difficulty_score").and_then(Value::as_u64).unwrap_or(
                match p["difficulty"].as_str() {
                    Some("easy") => 2,
                    Some("medium") => 3,
                    _ => 4,
                },
            ) as u8;
            Ok(crate::contract::ProblemSummary {
                id: r.get("id"),
                version: r.get("version"),
                title: serde_json::from_value(p["title"].clone())?,
                summary: serde_json::from_value(p.get("summary").cloned().unwrap_or(Value::Null))?,
                difficulty: Problem::difficulty_band(score).into(),
                difficulty_score: score,
                tags: serde_json::from_value(p["tags"].clone())?,
                cursor: format!(
                    "{}|{}",
                    p["title"].as_str().unwrap_or_default(),
                    r.get::<Uuid, _>("id")
                ),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(summaries))
}
async fn problem(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::contract::ProblemDetail>> {
    browse(&a, &h).await?;
    let r=sqlx::query("SELECT v.id,v.public FROM problems p JOIN versions v ON v.id=p.current_version WHERE p.id=$1").bind(id).fetch_optional(&a.db).await?.ok_or_else(||Error(StatusCode::NOT_FOUND,"problem not found".into()))?;
    let p: Value = r.get("public");
    let definition: Problem = serde_json::from_value(p.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let starter = |language: Language| {
        language.starter_with_definitions(&definition.interface, &definition.type_definitions)
    };
    let starters = Language::iter()
        .map(|language| Ok((language, starter(language)?)))
        .collect::<anyhow::Result<_>>()?;
    Ok(Json(crate::contract::ProblemDetail {
        id,
        version: r.get("id"),
        problem: definition,
        starters,
    }))
}

#[derive(Deserialize)]
struct SolutionDraft {
    source: String,
}
async fn get_solution(
    State(a): State<App>,
    h: HeaderMap,
    Path((version, language)): Path<(Uuid, Language)>,
) -> Result<Response> {
    let u = user(&a, &h, false).await?;
    let row = sqlx::query("SELECT source,updated_at FROM solution_drafts WHERE user_id=$1 AND version_id=$2 AND language=$3")
        .bind(u.id).bind(version).bind(language.identifier()).fetch_optional(&a.db).await?;
    let Some(row) = row else {
        return Ok((
            [(header::CACHE_CONTROL, "no-store")],
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":"solution not found"})),
            ),
        )
            .into_response());
    };
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(json!({"source":row.get::<String,_>("source"),"updated_at":row.get::<chrono::DateTime<chrono::Utc>,_>("updated_at")}))).into_response())
}
async fn put_solution(
    State(a): State<App>,
    h: HeaderMap,
    Path((version, language)): Path<(Uuid, Language)>,
    Json(draft): Json<SolutionDraft>,
) -> Result<StatusCode> {
    let u = user(&a, &h, true).await?;
    if draft.source.len() > 100000 {
        return Err(bad("source exceeds 100000 bytes"));
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM versions WHERE id=$1)")
        .bind(version)
        .fetch_one(&a.db)
        .await?;
    if !exists {
        return Err(Error(StatusCode::NOT_FOUND, "version not found".into()));
    }
    sqlx::query("INSERT INTO solution_drafts(user_id,version_id,language,source) VALUES($1,$2,$3,$4) ON CONFLICT(user_id,version_id,language) DO UPDATE SET source=excluded.source,updated_at=now()")
        .bind(u.id).bind(version).bind(language.identifier()).bind(draft.source).execute(&a.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn drafts(State(a): State<App>, h: HeaderMap) -> Result<Json<Vec<CatalogDraft>>> {
    admin(&a, &h, false).await?;
    let rows = sqlx::query("SELECT p.id,p.draft,p.current_version,(i.problem_id IS NOT NULL) AS managed FROM problems p LEFT JOIN problem_imports i ON i.problem_id=p.id ORDER BY p.draft->>'title'")
            .fetch_all(&a.db)
            .await?;
    Ok(Json(
        rows.iter()
            .map(|r| {
                Ok(CatalogDraft {
                    id: r.get("id"),
                    draft: serde_json::from_value(r.get("draft"))?,
                    version: r.get("current_version"),
                    managed: r.get("managed"),
                })
            })
            .collect::<anyhow::Result<_>>()?,
    ))
}
pub(crate) async fn new_draft(
    State(a): State<App>,
    h: HeaderMap,
    Json(p): Json<Problem>,
) -> Result<Json<CreatedProblem>> {
    admin(&a, &h, true).await?;
    create_draft_service(&a.db, p).await
}
pub async fn create_draft_service(db: &PgPool, p: Problem) -> Result<Json<CreatedProblem>> {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO problems(id,draft) VALUES($1,$2)")
        .bind(id)
        .bind(json!(p))
        .execute(db)
        .await?;
    Ok(Json(CreatedProblem { id }))
}
pub(crate) async fn save_draft(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
    Json(p): Json<Problem>,
) -> Result<StatusCode> {
    admin(&a, &h, true).await?;
    save_draft_service(&a.db, id, p).await
}
pub async fn save_draft_service(db: &PgPool, id: Uuid, p: Problem) -> Result<StatusCode> {
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM problem_imports WHERE problem_id=$1)",
    )
    .bind(id)
    .fetch_one(db)
    .await?
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "Git-managed problems are read-only".into(),
        ));
    }
    let r = sqlx::query("UPDATE problems SET draft=$2 WHERE id=$1")
        .bind(id)
        .bind(json!(p))
        .execute(db)
        .await?;
    if r.rows_affected() == 0 {
        return Err(Error(StatusCode::NOT_FOUND, "draft not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
pub(crate) async fn validate_definition(
    State(a): State<App>,
    h: HeaderMap,
    Json(p): Json<Problem>,
) -> Result<Json<ValidatedDefinition>> {
    admin(&a, &h, true).await?;
    let bytes = serde_json::to_vec(&p).map_err(anyhow::Error::from)?;
    let (_, hash) = crate::catalog::validate_problem(&bytes).map_err(|e| bad(&e.to_string()))?;
    Ok(Json(ValidatedDefinition {
        valid: true,
        content_hash: hash,
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CatalogSettingsUpdate {
    repository_url: String,
    strategy: String,
    revision: String,
    poll_interval_seconds: i32,
    enabled: bool,
    generation: i64,
}
fn validate_catalog_settings(r: &CatalogSettingsUpdate) -> std::result::Result<(), Error> {
    crate::catalog::validate_repository_url(&r.repository_url).map_err(|e| bad(&e.to_string()))?;
    if !matches!(r.strategy.as_str(), "track_branch" | "pinned_commit") {
        return Err(bad("invalid catalog strategy"));
    }
    if !(10..=86400).contains(&r.poll_interval_seconds) {
        return Err(bad("poll interval must be 10-86400 seconds"));
    }
    if r.revision.is_empty()
        || r.revision.len() > 200
        || r.revision.starts_with('-')
        || r.revision.chars().any(char::is_whitespace)
    {
        return Err(bad("invalid catalog revision"));
    }
    if r.strategy == "pinned_commit"
        && (r.revision.len() != 40 || !r.revision.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(bad(
            "pinned revision must be a full 40-character commit hash",
        ));
    }
    Ok(())
}
async fn catalog_status(State(a): State<App>, h: HeaderMap) -> Result<Json<Value>> {
    admin(&a, &h, false).await?;
    let setting=sqlx::query("SELECT repository_url,strategy::text,revision,poll_interval_seconds,enabled,generation,updated_at FROM catalog_settings WHERE singleton").fetch_optional(&a.db).await?;
    let runs=sqlx::query("SELECT id,settings_generation,requested_revision,resolved_commit,source_checksum,result,created_count,changed_count,unchanged_count,removed_count,diagnostics,started_at,finished_at FROM catalog_runs ORDER BY started_at DESC LIMIT 20").fetch_all(&a.db).await?;
    let settings=setting.map(|r|json!({"repository_url":r.get::<String,_>("repository_url"),"strategy":r.get::<String,_>("strategy"),"revision":r.get::<String,_>("revision"),"poll_interval_seconds":r.get::<i32,_>("poll_interval_seconds"),"enabled":r.get::<bool,_>("enabled"),"generation":r.get::<i64,_>("generation"),"updated_at":r.get::<chrono::DateTime<chrono::Utc>,_>("updated_at")}));
    let recent:Vec<Value>=runs.iter().map(|r|json!({"id":r.get::<Uuid,_>("id"),"settings_generation":r.get::<i64,_>("settings_generation"),"requested_revision":r.get::<String,_>("requested_revision"),"resolved_commit":r.get::<Option<String>,_>("resolved_commit"),"source_checksum":r.get::<Option<String>,_>("source_checksum"),"result":r.get::<String,_>("result"),"counts":{"created":r.get::<i32,_>("created_count"),"changed":r.get::<i32,_>("changed_count"),"unchanged":r.get::<i32,_>("unchanged_count"),"removed":r.get::<i32,_>("removed_count")},"diagnostics":r.get::<Option<String>,_>("diagnostics"),"started_at":r.get::<chrono::DateTime<chrono::Utc>,_>("started_at"),"finished_at":r.get::<Option<chrono::DateTime<chrono::Utc>>,_>("finished_at")})).collect();
    let discovered = recent.iter().find_map(|r| r["resolved_commit"].as_str());
    let applied = recent
        .iter()
        .find(|r| matches!(r["result"].as_str(), Some("applied" | "unchanged")))
        .and_then(|r| r["resolved_commit"].as_str());
    let last_success = recent
        .iter()
        .find(|r| matches!(r["result"].as_str(), Some("applied" | "unchanged")))
        .and_then(|r| r["finished_at"].as_str());
    let last_error = recent
        .iter()
        .find(|r| r["result"] == "failed")
        .and_then(|r| r["diagnostics"].as_str());
    Ok(Json(
        json!({"settings":settings,"state":{"discovered_revision":discovered,"applied_revision":applied,"last_successful_reconciliation":last_success,"last_error":last_error},"runs":recent}),
    ))
}
pub(crate) async fn catalog_update_service(
    db: &PgPool,
    r: CatalogSettingsUpdate,
) -> Result<Json<Value>> {
    validate_catalog_settings(&r)?;
    let mut tx = db.begin().await?;
    sqlx::query("INSERT INTO catalog_settings(singleton,repository_url,strategy,revision,poll_interval_seconds,enabled,generation) SELECT true,$1,$2::catalog_strategy,$3,$4,$5,0 WHERE $6=0 ON CONFLICT DO NOTHING")
        .bind(&r.repository_url).bind(&r.strategy).bind(&r.revision).bind(r.poll_interval_seconds).bind(r.enabled).bind(r.generation).execute(&mut *tx).await?;
    let changed=sqlx::query("UPDATE catalog_settings SET repository_url=$1,strategy=$2::catalog_strategy,revision=$3,poll_interval_seconds=$4,enabled=$5,generation=generation+1,updated_at=now() WHERE singleton AND generation=$6 RETURNING generation").bind(&r.repository_url).bind(&r.strategy).bind(&r.revision).bind(r.poll_interval_seconds).bind(r.enabled).bind(r.generation).fetch_optional(&mut *tx).await?;
    let generation = changed
        .ok_or_else(|| {
            Error(
                StatusCode::CONFLICT,
                "catalog settings changed; reload and retry".into(),
            )
        })?
        .get::<i64, _>("generation");
    sqlx::query("SELECT pg_notify('catalog_settings_changed',$1)")
        .bind(generation.to_string())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"generation":generation})))
}
pub(crate) async fn publish(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<PublishedProblem>> {
    admin(&a, &h, true).await?;
    publish_service(&a.db, id).await
}
pub async fn publish_service(db: &PgPool, id: Uuid) -> Result<Json<PublishedProblem>> {
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM problem_imports WHERE problem_id=$1)",
    )
    .bind(id)
    .fetch_one(db)
    .await?
    {
        return Err(Error(
            StatusCode::CONFLICT,
            "Git-managed problems are published only by the catalog controller".into(),
        ));
    }
    let mut tx = db.begin().await?;
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
    let provenance:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('catalog_key',catalog_key,'repository_url',repository_url,'resolved_commit',resolved_commit,'source_checksum',source_checksum,'artifact_hash',artifact_hash) FROM problem_imports WHERE problem_id=$1").bind(id).fetch_optional(&mut *tx).await?;
    sqlx::query("INSERT INTO versions(id,problem_id,public,tests,catalog_provenance) VALUES($1,$2,$3,$4,$5)")
        .bind(version)
        .bind(id)
        .bind(json!(p))
        .bind(tests)
        .bind(provenance)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE problems SET current_version=$2 WHERE id=$1")
        .bind(id)
        .bind(version)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(PublishedProblem { version }))
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
    let policy = crate::security::Policy::load(&a.db, u.id).await?;
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(829401)")
        .execute(&mut *tx)
        .await?;
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
    let active: bool = sqlx::query_scalar(
        "SELECT NOT suspended AND auth_generation=$2 FROM users WHERE id=$1 FOR UPDATE",
    )
    .bind(u.id)
    .bind(u.generation)
    .fetch_one(&mut *tx)
    .await?;
    if !active {
        return Err(deny());
    }
    let (minute,day):(i64,i64)=sqlx::query_as("SELECT count(*) FILTER (WHERE created_at>clock_timestamp()-interval '60 seconds'),count(*) FILTER (WHERE created_at>=date_trunc('day',clock_timestamp() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC') FROM submissions WHERE user_id=$1 AND created_at>clock_timestamp()-interval '2 days'").bind(u.id).fetch_one(&mut *tx).await?;
    if minute >= policy.submissions_minute || day >= policy.submissions_day {
        return Ok(crate::security::limited(
            if minute >= policy.submissions_minute {
                "submissions_minute"
            } else {
                "submissions_day"
            },
            if minute >= policy.submissions_minute {
                60
            } else {
                86400
            },
        ));
    }
    let global: i64 =
        sqlx::query_scalar("SELECT count(*) FROM submissions WHERE status!='completed'")
            .fetch_one(&mut *tx)
            .await?;
    if global
        >= crate::env("GLOBAL_PENDING_LIMIT", "20")
            .parse()
            .map_err(|_| crate::security::unavailable())?
    {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "submission capacity reached".into(),
        ));
    }
    if count >= policy.pending {
        return Ok(crate::security::limited("pending_submissions", 10));
    }
    let row = sqlx::query("SELECT public FROM versions WHERE id=$1")
        .bind(r.version)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| bad("unknown version"))?;
    let p: Problem = serde_json::from_value(row.get("public")).map_err(|e| anyhow::anyhow!(e))?;
    let id = Uuid::new_v4();
    let custom_cases = if r.mode == "run" {
        let mut cases = r.cases.clone().unwrap_or_else(|| p.tests.clone());
        if cases.is_empty() || cases.len() > 20 {
            return Err(bad("run requires 1–20 cases"));
        }
        for c in &mut cases {
            c.hidden = false;
            if p.validate_case(c).is_err() {
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
        schema: p.schema,
        id,
        language: r.language,
        version: r.version,
        interface: p.interface,
        limits: p.limits,
        mode: r.mode,
        type_definitions: p.type_definitions,
        comparison: p.comparison,
    };
    sqlx::query("INSERT INTO submissions(id,user_id,version_id,idempotency_key,request_hash,source,custom_cases,job,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,clock_timestamp())").bind(id).bind(u.id).bind(r.version).bind(key).bind(hash).bind(r.source).bind(custom_cases).bind(json!(job)).execute(&mut *tx).await?;
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
    let _: String = valkey::cmd("PING")
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
            "j0coder_submissions{{status=\"{}\"}} {}\n",
            r.get::<String, _>("status"),
            r.get::<i64, _>("n")
        ));
    }
    let rows=sqlx::query("SELECT result->>'verdict' AS verdict,count(*) AS n,coalesce(sum((result->>'elapsed_ms')::bigint),0)::bigint AS elapsed FROM submissions WHERE status='completed' GROUP BY result->>'verdict'").fetch_all(&a.db).await?;
    for r in rows {
        let verdict: String = r.get("verdict");
        output.push_str(&format!("j0coder_executions_total{{verdict=\"{verdict}\"}} {}\nj0coder_execution_seconds_total{{verdict=\"{verdict}\"}} {}\n",r.get::<i64,_>("n"),r.get::<i64,_>("elapsed") as f64/1000.0));
    }
    let mut c = crate::queue::connection().await?;
    let _: usize = valkey::cmd("ZREMRANGEBYSCORE")
        .arg("editor:active")
        .arg("-inf")
        .arg(chrono::Utc::now().timestamp())
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let active: usize = valkey::cmd("ZCARD")
        .arg("editor:active")
        .query_async(&mut c)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    output.push_str(&format!("j0coder_editor_sessions {active}\n"));
    Ok(output)
}
pub fn router(a: App) -> Router {
    let mut router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(ready))
        .route(
            "/api/v1/openapi.json",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "application/json")],
                    include_str!("../../../openapi.json"),
                )
            }),
        )
        .route("/api/v1/capabilities", get(crate::accounts::capabilities))
        .route("/api/v1/session", get(me).post(login).delete(logout))
        .route("/api/v1/register", post(crate::accounts::register))
        .route("/api/v1/reset-password", post(crate::accounts::reset))
        .route(
            "/api/v1/me/preferences",
            get(crate::accounts::preferences).patch(crate::accounts::update_preferences),
        )
        .route("/api/v1/me/limits", get(crate::accounts::limits))
        .route(
            "/api/v1/me/password",
            post(crate::accounts::change_password),
        )
        .route("/api/v1/me/logout-all", post(crate::accounts::logout_all))
        .route(
            "/api/v1/me/reauthenticate",
            post(crate::accounts::reauthenticate),
        )
        .route("/api/v1/problems", get(problems))
        .route("/api/v1/problems/{id}", get(problem))
        .route(
            "/api/v1/solutions/{version}/{language}",
            get(get_solution).put(put_solution),
        )
        .route("/api/v1/submissions", get(history).post(submit))
        .route("/api/v1/submissions/{id}", get(submission))
        .route("/api/v1/editor-ticket", post(crate::editor::ticket));
    if a.private {
        router = router
            .route("/metrics", get(metrics))
            .route("/api/v1/admin/problems", get(drafts).post(new_draft))
            .route("/api/v1/admin/problems/validate", post(validate_definition))
            .route("/api/v1/admin/catalog", get(catalog_status))
            .route("/api/v1/admin/problems/{id}", put(save_draft))
            .route("/api/v1/admin/problems/{id}/publish", post(publish));
    }
    router.fallback_service(tower_http::services::ServeDir::new(crate::env("WEB_DIR","web/dist")).not_found_service(tower_http::services::ServeFile::new(format!("{}/index.html",crate::env("WEB_DIR","web/dist")))))
 .layer(DefaultBodyLimit::max(2*1024*1024))
 .layer(axum::middleware::from_fn_with_state(a.clone(),crate::security::admission))
 .layer(axum::middleware::from_fn(|req:axum::extract::Request,next:axum::middleware::Next|async move {
 let cache_private=req.uri().path().starts_with("/api/");
 let mut response=next.run(req).await;let h=response.headers_mut();
 h.insert("x-content-type-options","nosniff".parse().unwrap());h.insert("referrer-policy","no-referrer".parse().unwrap());
 if cache_private {h.insert(header::CACHE_CONTROL,"no-store".parse().unwrap());}
 h.insert("content-security-policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; worker-src 'self' blob:; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'".parse().unwrap());response
 })).with_state(a)
}

#[cfg(test)]
mod admission_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires disposable PostgreSQL; creates a private schema"]
    async fn submissions_do_not_acquire_nested_pool_connections() -> anyhow::Result<()> {
        let url = std::env::var("SECURITY_TEST_DATABASE_URL")?;
        let schema = format!("admission_{}", Uuid::new_v4().simple());
        let root = PgPool::connect(&url).await?;
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&root)
            .await?;
        let search_path = schema.clone();
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .after_connect(move |connection, _| {
                let command = format!("SET search_path TO {search_path}");
                Box::pin(async move {
                    sqlx::query(&command).execute(connection).await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await?;
        sqlx::migrate!("../../migrations").run(&db).await?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO users(id,username,password) VALUES($1,'pool_test','unused')")
            .bind(id)
            .execute(&db)
            .await?;
        let raw = crate::security::token();
        sqlx::query("INSERT INTO sessions(id,user_id,csrf) VALUES($1,$2,'csrf')")
            .bind(crate::security::digest(&raw))
            .bind(id)
            .execute(&db)
            .await?;
        let a = App {
            db: db.clone(),
            origin: "http://test.local".into(),
            secure: false,
            private: false,
        };
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, format!("practice_session={raw}").parse()?);
        headers.insert(header::ORIGIN, "http://test.local".parse()?);
        headers.insert("x-csrf-token", "csrf".parse()?);
        headers.insert("idempotency-key", "pool-test".parse()?);
        let request = || Submit {
            version: Uuid::new_v4(),
            language: Language::Python,
            source: "pass".into(),
            mode: "submit".into(),
            cases: None,
        };
        // A single pool connection makes nested acquisition fail deterministically.
        // Concurrent requests must both reach normal validation without starving it.
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            tokio::join!(
                submit(State(a.clone()), headers.clone(), Json(request())),
                submit(State(a.clone()), headers.clone(), Json(request()))
            )
        })
        .await;
        db.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&root)
            .await?;
        let (first, second) = result?;
        for response in [first, second] {
            assert!(matches!(response, Err(Error(StatusCode::BAD_REQUEST, _))));
        }
        Ok(())
    }
}
