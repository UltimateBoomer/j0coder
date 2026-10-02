//! Shared account policy, admission control, and revocable editor authorization.
use crate::api::{App, Error};
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

pub fn token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}
pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn valid_token(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub async fn password_slot() -> Result<OwnedSemaphorePermit, Error> {
    static RUNNING: OnceLock<Arc<Semaphore>> = OnceLock::new();
    static WAITING: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let waiting = WAITING
        .get_or_init(|| Arc::new(Semaphore::new(8)))
        .clone()
        .try_acquire_owned()
        .map_err(|_| unavailable())?;
    let permit = tokio::time::timeout(
        Duration::from_secs(1),
        RUNNING
            .get_or_init(|| Arc::new(Semaphore::new(2)))
            .clone()
            .acquire_owned(),
    )
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    drop(waiting);
    Ok(permit)
}
pub fn unavailable() -> Error {
    Error(
        StatusCode::SERVICE_UNAVAILABLE,
        "admission_unavailable".into(),
    )
}
pub fn limited(reason: &str, retry: u64) -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [("Retry-After", retry.max(1).to_string())],
        axum::Json(serde_json::json!({"error":"rate limit exceeded","reason":reason})),
    )
        .into_response()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub api_rate: u32,
    pub save_rate: u32,
    pub submissions_minute: i64,
    pub submissions_day: i64,
    pub pending: i64,
    pub ticket_rate: u32,
    pub editor_sessions: u32,
    pub editor_messages: u32,
    pub editor_bytes: u32,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            api_rate: 180,
            save_rate: 120,
            submissions_minute: 6,
            submissions_day: 100,
            pending: 2,
            ticket_rate: 6,
            editor_sessions: 1,
            editor_messages: 20,
            editor_bytes: 1048576,
        }
    }
}
impl Policy {
    pub fn deployment_defaults() -> anyhow::Result<Self> {
        let configured = crate::env("DEFAULT_USER_POLICY", "{}");
        let p: Self = serde_json::from_str(if configured.is_empty() {
            "{}"
        } else {
            &configured
        })?;
        anyhow::ensure!(
            (1..=10000).contains(&p.api_rate)
                && (1..=10000).contains(&p.save_rate)
                && (1..=1000).contains(&p.submissions_minute)
                && (1..=100000).contains(&p.submissions_day)
                && (1..=100).contains(&p.pending)
                && (1..=1000).contains(&p.ticket_rate)
                && (1..=16).contains(&p.editor_sessions)
                && (1..=1000).contains(&p.editor_messages)
                && (1024..=16777216).contains(&p.editor_bytes),
            "invalid DEFAULT_USER_POLICY"
        );
        Ok(p)
    }
    pub async fn load(db: &PgPool, id: Uuid) -> anyhow::Result<Self> {
        let r = sqlx::query("SELECT * FROM user_policies WHERE user_id=$1")
            .bind(id)
            .fetch_optional(db)
            .await?;
        let mut p = Self::deployment_defaults()?;
        if let Some(r) = r {
            macro_rules! field {
                ($f:ident) => {
                    if let Some(v) = r.get::<Option<i32>, _>(stringify!($f)) {
                        p.$f = v as _;
                    }
                };
            }
            field!(api_rate);
            field!(save_rate);
            field!(submissions_minute);
            field!(submissions_day);
            field!(pending);
            field!(ticket_rate);
            field!(editor_sessions);
            field!(editor_messages);
            field!(editor_bytes);
        }
        Ok(p)
    }
}
/// All bucket dimensions are charged atomically; a denied request charges none.
pub async fn buckets(dimensions: &[(String, u32, u32, u32)]) -> anyhow::Result<u64> {
    let mut c = crate::queue::connection().await?;
    let script = valkey::Script::new(
        r#"local time=redis.call('TIME');local now=time[1]*1000+math.floor(time[2]/1000);local states={};local wait=0;for i,k in ipairs(KEYS) do local rate=tonumber(ARGV[(i-1)*3+1]);local cap=tonumber(ARGV[(i-1)*3+2]);local cost=tonumber(ARGV[(i-1)*3+3]);local old=redis.call('HMGET',k,'tokens','time');local tokens=math.min(cap,tonumber(old[1]) or cap);if old[2] then tokens=math.min(cap,tokens+math.max(0,now-tonumber(old[2]))*rate/1000) end;states[i]={tokens,cap,rate,cost};if tokens<cost then wait=math.max(wait,math.ceil((cost-tokens)/rate)) end end;if wait>0 then return wait end;for i,k in ipairs(KEYS) do local s=states[i];redis.call('HSET',k,'tokens',s[1]-s[4],'time',now);redis.call('PEXPIRE',k,math.ceil(s[2]/s[3]*1000)+60000) end;return 0"#,
    );
    let mut invocation = script.prepare_invoke();
    for (key, _, _, _) in dimensions {
        invocation.key(format!("limit:{key}"));
    }
    // Rates are expressed per minute; fractional tokens per second are accepted by Lua.
    for (_, rate, cap, cost) in dimensions {
        invocation.arg(*rate as f64 / 60.0).arg(cap).arg(cost);
    }
    Ok(invocation.invoke_async::<u64>(&mut c).await?)
}
fn cidr_contains(cidr: &str, ip: IpAddr) -> bool {
    let Some((addr, prefix)) = cidr.split_once('/') else {
        return false;
    };
    let Ok(addr) = addr.parse::<IpAddr>() else {
        return false;
    };
    let Ok(prefix) = prefix.parse::<u32>() else {
        return false;
    };
    match (addr, ip) {
        (IpAddr::V4(a), IpAddr::V4(b)) if prefix <= 32 => {
            prefix == 0 || (u32::from(a) >> (32 - prefix)) == (u32::from(b) >> (32 - prefix))
        }
        (IpAddr::V6(a), IpAddr::V6(b)) if prefix <= 128 => {
            prefix == 0 || (u128::from(a) >> (128 - prefix)) == (u128::from(b) >> (128 - prefix))
        }
        _ => false,
    }
}
pub fn client_ip(peer: IpAddr, headers: &axum::http::HeaderMap) -> IpAddr {
    if !crate::env("TRUSTED_PROXY_CIDRS", "")
        .split(',')
        .any(|c| cidr_contains(c.trim(), peer))
    {
        return peer;
    }
    let mut ip = peer;
    if let Some(chain) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        for part in chain.rsplit(',') {
            if !crate::env("TRUSTED_PROXY_CIDRS", "")
                .split(',')
                .any(|c| cidr_contains(c.trim(), ip))
            {
                break;
            }
            let Ok(next) = part.trim().parse() else {
                return peer;
            };
            ip = next;
        }
    }
    ip
}
pub async fn admission(State(a): State<App>, request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();
    let method = request.method().as_str();
    if !a.private && (path.starts_with("/api/v1/admin/") || path == "/metrics") {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error":"not found"})),
        )
            .into_response();
    }

    // Revocation/recovery remains accessible when normal authenticated budgets run out.
    let recovery =
        (path == "/api/v1/session" && method == "DELETE") || path == "/api/v1/me/logout-all";
    if !path.starts_with("/api/") && path != "/readyz" {
        return next.run(request).await;
    }
    let unauthenticated = path == "/readyz"
        || (method == "GET" && ["/api/v1/capabilities", "/api/v1/openapi.json"].contains(&path))
        || (method == "POST"
            && [
                "/api/v1/session",
                "/api/v1/register",
                "/api/v1/reset-password",
            ]
            .contains(&path))
        || (!a.private
            && method == "GET"
            && (path == "/api/v1/problems" || path.starts_with("/api/v1/problems/"))
            && crate::env("GUEST_BROWSING_ENABLED", "false") == "true");
    let write = !["GET", "HEAD", "OPTIONS"].contains(&method);
    let authenticated = if a.private && path.starts_with("/api/v1/admin/") {
        crate::api::admin(&a, request.headers(), write).await
    } else {
        crate::api::user(&a, request.headers(), write && !unauthenticated).await
    };
    if !unauthenticated && let Err(error) = &authenticated {
        return (error.0, axum::Json(serde_json::json!({"error":error.1}))).into_response();
    }
    if recovery {
        return next.run(request).await;
    }
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|p| p.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let ip = digest(&client_ip(peer, request.headers()).to_string());
    let mut dimensions = vec![];
    if !recovery {
        if let Ok(u) = authenticated {
            let p = match Policy::load(&a.db, u.id).await {
                Ok(p) => p,
                Err(_) => return unavailable().into_response(),
            };
            dimensions.push((format!("user:{}:api", u.id), p.api_rate, 30, 1));
            if path.starts_with("/api/v1/solutions/") && method == "PUT" {
                dimensions.push((format!("user:{}:save", u.id), p.save_rate, 20, 1));
            }
            if path == "/api/v1/editor-ticket" {
                dimensions.push((format!("user:{}:ticket", u.id), p.ticket_rate, 2, 1));
            }
        } else {
            dimensions.push((format!("ip:{ip}:read"), 60, 10, 1));
        }
    }
    if path == "/api/v1/session" && method == "POST" {
        dimensions.push((format!("ip:{ip}:login"), 5, 5, 1));
    }
    if path == "/api/v1/register" || path == "/api/v1/reset-password" {
        dimensions.push((format!("ip:{ip}:redeem"), 3, 3, 1)); // explicit hourly bucket below
        let mut c = match crate::queue::connection().await {
            Ok(c) => c,
            Err(_) => return unavailable().into_response(),
        };
        let n:valkey::RedisResult<i64>=valkey::Script::new("local n=redis.call('INCR',KEYS[1]);if n==1 then redis.call('EXPIRE',KEYS[1],3600) end;return n").key(format!("redeem:{ip}")).invoke_async(&mut c).await;
        match n {
            Ok(n) if n <= 3 => {}
            Ok(_) => return limited("redemption_hour", 3600),
            Err(_) => return unavailable().into_response(),
        }
    }
    match buckets(&dimensions).await {
        Ok(0) => next.run(request).await,
        Ok(wait) => limited("request_rate", wait),
        Err(_) => unavailable().into_response(),
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLease {
    pub session: String,
    pub generation: i64,
    pub user: Uuid,
    pub policy: Policy,
}
pub async fn refresh_editors(db: &PgPool) -> anyhow::Result<()> {
    let mut c = crate::queue::connection().await?;
    let rows=sqlx::query("SELECT s.id,s.user_id,s.generation FROM sessions s JOIN users u ON u.id=s.user_id LEFT JOIN user_preferences p ON p.user_id=u.id WHERE coalesce(p.semantic_completion,true) AND NOT u.suspended AND s.generation=u.auth_generation AND s.audience='public' AND s.expires>now() AND s.last_seen>now()-interval '24 hours'").fetch_all(db).await?;
    for r in rows {
        let id: Uuid = r.get("user_id");
        let sid: String = r.get("id");
        let policy = Policy::load(db, id).await?;
        // Recheck immediately before publication, rather than renewing an old snapshot.
        let generation: Option<i64> = sqlx::query_scalar("SELECT s.generation FROM sessions s JOIN users u ON u.id=s.user_id LEFT JOIN user_preferences p ON p.user_id=u.id WHERE s.id=$1 AND coalesce(p.semantic_completion,true) AND NOT u.suspended AND s.generation=u.auth_generation AND s.audience='public' AND s.expires>clock_timestamp() AND s.last_seen>clock_timestamp()-interval '24 hours'")
            .bind(&sid).fetch_optional(db).await?;
        let Some(generation) = generation else {
            let _: () = valkey::cmd("DEL")
                .arg(format!("auth:session:{sid}"))
                .query_async(&mut c)
                .await?;
            continue;
        };
        let lease = EditorLease {
            session: sid.clone(),
            generation,
            user: id,
            policy,
        };
        let _: () = valkey::cmd("SET")
            .arg(format!("auth:session:{sid}"))
            .arg(serde_json::to_string(&lease)?)
            .arg("EX")
            .arg(45)
            .query_async(&mut c)
            .await?;
    }
    sqlx::query("DELETE FROM sessions WHERE expires<=now() OR last_seen<=now()-CASE WHEN audience='admin' THEN interval '15 minutes' ELSE interval '24 hours' END").execute(db).await?;
    sqlx::query("DELETE FROM account_tokens WHERE expires<=now()")
        .execute(db)
        .await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cidrs_and_tokens() {
        assert!(cidr_contains("127.0.0.0/8", "127.0.0.1".parse().unwrap()));
        assert!(!cidr_contains("127.0.0.0/8", "10.0.0.1".parse().unwrap()));
        assert!(cidr_contains("::1/128", "::1".parse().unwrap()));
        assert!(!cidr_contains("::1/129", "::1".parse().unwrap()));
        assert!(valid_token(&token()));
        assert!(!valid_token("x"));
    }
}
