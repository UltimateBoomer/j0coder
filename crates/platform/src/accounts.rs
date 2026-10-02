use crate::{
    api::{self, App, Error, User},
    security,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub theme: String,
    pub default_language: String,
    pub semantic_completion: bool,
    pub font_size: i32,
    pub tab_width: i32,
    pub word_wrap: bool,
    pub minimap: bool,
    pub blind_mode: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            default_language: "cpp".into(),
            semantic_completion: true,
            font_size: 14,
            tab_width: 4,
            word_wrap: false,
            minimap: false,
            blind_mode: false,
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            ["system", "light", "dark"].contains(&self.theme.as_str())
                && ["cpp", "python", "java", "kotlin"].contains(&self.default_language.as_str())
                && (10..=24).contains(&self.font_size)
                && [2, 4, 8].contains(&self.tab_width),
            "invalid preferences"
        );
        Ok(())
    }
    pub async fn load(db: &PgPool, id: Uuid) -> anyhow::Result<Self> {
        let value: Option<Value> = sqlx::query_scalar(
            "SELECT to_jsonb(p)-'user_id' FROM user_preferences p WHERE user_id=$1",
        )
        .bind(id)
        .fetch_optional(db)
        .await?;
        Ok(value
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default())
    }
}
pub async fn preferences(State(a): State<App>, h: HeaderMap) -> Result<Json<Preferences>, Error> {
    let u = api::user(&a, &h, false).await?;
    Ok(Json(Preferences::load(&a.db, u.id).await?))
}
pub async fn update_preferences(
    State(a): State<App>,
    h: HeaderMap,
    Json(patch): Json<Value>,
) -> Result<Json<Preferences>, Error> {
    let u = api::user(&a, &h, true).await?;
    let mut tx = a.db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    let value: Option<Value> =
        sqlx::query_scalar("SELECT to_jsonb(p)-'user_id' FROM user_preferences p WHERE user_id=$1")
            .bind(u.id)
            .fetch_optional(&mut *tx)
            .await?;
    let mut merged = value.unwrap_or(serde_json::to_value(Preferences::default()).unwrap());
    let Some(patch) = patch.as_object() else {
        return Err(Error(
            StatusCode::BAD_REQUEST,
            "expected preferences object".into(),
        ));
    };
    for (k, v) in patch {
        merged.as_object_mut().unwrap().insert(k.clone(), v.clone());
    }
    let p: Preferences = serde_json::from_value(merged)
        .map_err(|_| Error(StatusCode::BAD_REQUEST, "invalid preferences".into()))?;
    p.validate()
        .map_err(|_| Error(StatusCode::BAD_REQUEST, "invalid preferences".into()))?;
    sqlx::query("INSERT INTO user_preferences(user_id,theme,default_language,semantic_completion,font_size,tab_width,word_wrap,minimap,blind_mode) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(user_id) DO UPDATE SET theme=$2,default_language=$3,semantic_completion=$4,font_size=$5,tab_width=$6,word_wrap=$7,minimap=$8,blind_mode=$9")
 .bind(u.id).bind(&p.theme).bind(&p.default_language).bind(p.semantic_completion).bind(p.font_size).bind(p.tab_width).bind(p.word_wrap).bind(p.minimap).bind(p.blind_mode).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(p))
}
pub async fn limits(State(a): State<App>, h: HeaderMap) -> Result<Json<security::Policy>, Error> {
    let u = api::user(&a, &h, false).await?;
    Ok(Json(security::Policy::load(&a.db, u.id).await?))
}
pub async fn capabilities(State(a): State<App>) -> Json<Value> {
    Json(
        json!({"registration":crate::env("REGISTRATION_MODE","invite"),"guest_browsing":!a.private && crate::env("GUEST_BROWSING_ENABLED","false")=="true","web_admin":a.private}),
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Register {
    pub token: String,
    pub username: String,
    pub password: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reset {
    pub token: String,
    pub password: String,
}
fn invalid_token() -> Error {
    Error(StatusCode::BAD_REQUEST, "invalid or expired token".into())
}
pub async fn issue_token(db: &PgPool, purpose: &str, user: Option<Uuid>) -> anyhow::Result<String> {
    let raw = security::token();
    sqlx::query("INSERT INTO account_tokens(token_hash,purpose,user_id,expires) VALUES($1,$2,$3,now()+CASE WHEN $2='invite' THEN interval '7 days' ELSE interval '1 hour' END)").bind(security::digest(&raw)).bind(purpose).bind(user).execute(db).await?;
    Ok(raw)
}
async fn token_preflight(db: &PgPool, raw: &str, purpose: &str) -> Result<(), Error> {
    if !security::valid_token(raw) {
        return Err(invalid_token());
    }
    let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM account_tokens WHERE token_hash=$1 AND purpose=$2 AND expires>now())").bind(security::digest(raw)).bind(purpose).fetch_one(db).await?;
    if !exists {
        return Err(invalid_token());
    }
    Ok(())
}
pub async fn register(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<Register>,
) -> Result<StatusCode, Error> {
    api::origin(&a, &h)?;
    if a.private || crate::env("REGISTRATION_MODE", "invite") != "invite" {
        return Err(Error(StatusCode::FORBIDDEN, "registration closed".into()));
    }
    token_preflight(&a.db, &r.token, "invite").await?;
    api::validate_username(&r.username)
        .map_err(|_| Error(StatusCode::BAD_REQUEST, "invalid username".into()))?;
    let hash = api::password_hash_async(r.password).await?;
    let mut tx = a.db.begin().await?;
    // Shares bootstrap's table lock so invitations cannot win a bootstrap race.
    sqlx::query("LOCK TABLE users IN ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let consumed=sqlx::query("DELETE FROM account_tokens WHERE token_hash=$1 AND purpose='invite' AND expires>clock_timestamp() RETURNING token_hash").bind(security::digest(&r.token)).fetch_optional(&mut *tx).await?;
    if consumed.is_none() {
        return Err(invalid_token());
    }
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO users(id,username,password) VALUES($1,$2,$3) ON CONFLICT(username) DO NOTHING",
    )
    .bind(id)
    .bind(r.username)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(Error(StatusCode::CONFLICT, "username unavailable".into()));
    }
    sqlx::query("INSERT INTO security_audit(action,user_id) VALUES('signup',$1)")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::CREATED)
}
pub async fn revoke(db: &PgPool, user: Uuid, action: &str) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("UPDATE users SET auth_generation=auth_generation+1 WHERE id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(user)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO security_audit(action,user_id) VALUES($1,$2)")
        .bind(action)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn reset(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<Reset>,
) -> Result<Json<Value>, Error> {
    api::origin(&a, &h)?;
    token_preflight(&a.db, &r.token, "reset").await?;
    let hash = api::password_hash_async(r.password).await?;
    let mut tx = a.db.begin().await?;
    // Lock the account before the token, matching password-change lock ordering.
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id=(SELECT user_id FROM account_tokens WHERE token_hash=$1 AND purpose='reset' AND expires>clock_timestamp()) FOR UPDATE")
        .bind(security::digest(&r.token)).fetch_optional(&mut *tx).await?;
    owner.ok_or_else(invalid_token)?;
    let id:Option<Uuid>=sqlx::query_scalar("DELETE FROM account_tokens WHERE token_hash=$1 AND purpose='reset' AND expires>clock_timestamp() RETURNING user_id").bind(security::digest(&r.token)).fetch_optional(&mut *tx).await?;
    let id = id.ok_or_else(invalid_token)?;
    sqlx::query("UPDATE users SET password=$2,auth_generation=auth_generation+1 WHERE id=$1")
        .bind(id)
        .bind(hash)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM account_tokens WHERE user_id=$1 AND purpose='reset'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO security_audit(action,user_id) VALUES('password_reset',$1)")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordChange {
    pub current_password: String,
    pub new_password: String,
}
pub async fn verify_password(db: &PgPool, u: &User, password: String) -> Result<(), Error> {
    if password.len() > 256 {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "authentication required".into(),
        ));
    }
    let hash: String = sqlx::query_scalar("SELECT password FROM users WHERE id=$1")
        .bind(u.id)
        .fetch_one(db)
        .await?;
    let _slot = security::password_slot().await?;
    let valid = tokio::task::spawn_blocking(move || {
        use argon2::PasswordVerifier;
        argon2::password_hash::PasswordHash::new(&hash).is_ok_and(|hash| {
            argon2::Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .map_err(|_| security::unavailable())?;
    if !valid {
        return Err(Error(
            StatusCode::UNAUTHORIZED,
            "authentication required".into(),
        ));
    }
    Ok(())
}
pub async fn change_password(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<PasswordChange>,
) -> Result<Json<Value>, Error> {
    let u = api::user(&a, &h, true).await?;
    verify_password(&a.db, &u, r.current_password).await?;
    let hash = api::password_hash_async(r.new_password).await?;
    let mut tx = a.db.begin().await?;
    let changed=sqlx::query("UPDATE users SET password=$2,auth_generation=auth_generation+1 WHERE id=$1 AND auth_generation=$3 AND NOT suspended").bind(u.id).bind(hash).bind(u.generation).execute(&mut *tx).await?;
    if changed.rows_affected() == 0 {
        return Err(Error(StatusCode::UNAUTHORIZED, "session revoked".into()));
    }
    sqlx::query("DELETE FROM sessions WHERE user_id=$1")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM account_tokens WHERE user_id=$1 AND purpose='reset'")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO security_audit(action,user_id) VALUES('password_change',$1)")
        .bind(u.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn logout_all(State(a): State<App>, h: HeaderMap) -> Result<Json<Value>, Error> {
    let u = api::user(&a, &h, true).await?;
    revoke(&a.db, u.id, "logout_all").await?;
    Ok(Json(json!({"ok":true})))
}
pub async fn reauthenticate(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<api::Login>,
) -> Result<Response, Error> {
    let u = api::user(&a, &h, true).await?;
    verify_password(&a.db, &u, r.password).await?;
    sqlx::query("UPDATE sessions SET authenticated_at=now() WHERE id=$1")
        .bind(u.session)
        .execute(&a.db)
        .await?;
    Ok(Json(json!({"ok":true})).into_response())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_input_is_bounded() {
        assert!(Preferences::default().validate().is_ok());
        assert!(serde_json::from_value::<Preferences>(json!({"admin":true})).is_err());
        let p = Preferences {
            font_size: 999,
            ..Default::default()
        };
        assert!(p.validate().is_err());
    }
}
