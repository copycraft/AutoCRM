use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::domain::role::Role;

/// Failed logins before the account is temporarily locked, and for how long.
const LOCK_AFTER_FAILURES: i32 = 10;
const LOCK_MINUTES: i32 = 15;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub is_active: bool,
    pub must_change_password: bool,
    /// Unlocks the HR module on top of the role. Admins have it regardless.
    pub hr_access: bool,
    /// Capability keys (`edit_orders`, ...) granted on top of the role (0050).
    pub permissions: Vec<String>,
    /// Signs in with an authenticator app's code as well (0049).
    pub two_factor: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct Credentials {
    pub id: i64,
    pub password_hash: String,
    pub is_active: bool,
    pub locked_until: Option<DateTime<Utc>>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<User>> {
    sqlx::query_as!(
        User,
        r#"SELECT id, email, display_name, role AS "role: Role", is_active, must_change_password, hr_access, permissions, totp_enabled_at IS NOT NULL AS "two_factor!", created_at, updated_at
           FROM users WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn find_credentials_by_email(
    db: impl PgExecutor<'_>,
    email: &str,
) -> sqlx::Result<Option<Credentials>> {
    sqlx::query_as!(
        Credentials,
        "SELECT id, password_hash, is_active, locked_until FROM users WHERE lower(email) = lower($1)",
        email.trim()
    )
    .fetch_optional(db)
    .await
}

pub async fn find_credentials(
    db: impl PgExecutor<'_>,
    id: i64,
) -> sqlx::Result<Option<Credentials>> {
    sqlx::query_as!(
        Credentials,
        "SELECT id, password_hash, is_active, locked_until FROM users WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<User>> {
    sqlx::query_as!(
        User,
        r#"SELECT id, email, display_name, role AS "role: Role", is_active, must_change_password, hr_access, permissions, totp_enabled_at IS NOT NULL AS "two_factor!", created_at, updated_at
           FROM users ORDER BY is_active DESC, display_name"#
    )
    .fetch_all(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    email: &str,
    display_name: &str,
    role: Role,
    password_hash: &str,
    must_change_password: bool,
) -> sqlx::Result<User> {
    sqlx::query_as!(
        User,
        r#"INSERT INTO users (email, display_name, role, password_hash, must_change_password)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, email, display_name, role AS "role: Role", is_active, must_change_password, hr_access, permissions, totp_enabled_at IS NOT NULL AS "two_factor!", created_at, updated_at"#,
        email,
        display_name,
        role as Role,
        password_hash,
        must_change_password
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    display_name: Option<&str>,
    role: Option<Role>,
    is_active: Option<bool>,
    hr_access: Option<bool>,
    permissions: Option<&[String]>,
) -> sqlx::Result<Option<User>> {
    sqlx::query_as!(
        User,
        r#"UPDATE users
           SET display_name = coalesce($2, display_name),
               role = coalesce($3, role),
               is_active = coalesce($4, is_active),
               hr_access = coalesce($5, hr_access),
               permissions = coalesce($6, permissions)
           WHERE id = $1
           RETURNING id, email, display_name, role AS "role: Role", is_active, must_change_password, hr_access, permissions, totp_enabled_at IS NOT NULL AS "two_factor!", created_at, updated_at"#,
        id,
        display_name,
        role as Option<Role>,
        is_active,
        hr_access,
        permissions
    )
    .fetch_optional(db)
    .await
}

pub async fn set_password(
    db: impl PgExecutor<'_>,
    id: i64,
    password_hash: &str,
    must_change: bool,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "UPDATE users SET password_hash = $2, must_change_password = $3, failed_logins = 0, locked_until = NULL WHERE id = $1",
        id,
        password_hash,
        must_change
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn record_failed_login(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE users
         SET failed_logins = failed_logins + 1,
             locked_until = CASE WHEN failed_logins + 1 >= $2 THEN now() + make_interval(mins => $3) ELSE locked_until END
         WHERE id = $1",
        id,
        LOCK_AFTER_FAILURES,
        LOCK_MINUTES
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn record_login_success(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!("UPDATE users SET failed_logins = 0, locked_until = NULL WHERE id = $1 AND (failed_logins <> 0 OR locked_until IS NOT NULL)", id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn count_active_admins(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM users WHERE role = 'admin' AND is_active"#)
        .fetch_one(db)
        .await
}

/// Two-factor state (0049). `secret` is sealed (service::secrets); it is set while setup is
/// pending and stays once enabled.
pub struct TotpState {
    pub secret: Option<String>,
    pub enabled: bool,
    pub last_step: Option<i64>,
}

pub async fn totp_state(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<TotpState>> {
    sqlx::query_as!(
        TotpState,
        r#"SELECT totp_secret AS secret, totp_enabled_at IS NOT NULL AS "enabled!", totp_last_step AS last_step
           FROM users WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

/// Starts (or restarts) setup: a new secret, not yet required at login.
pub async fn set_pending_totp(db: impl PgExecutor<'_>, id: i64, sealed: &str) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE users SET totp_secret = $2, totp_enabled_at = NULL, totp_last_step = NULL WHERE id = $1",
        id,
        sealed
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn enable_totp(db: impl PgExecutor<'_>, id: i64, step: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE users SET totp_enabled_at = now(), totp_last_step = $2 WHERE id = $1 AND totp_secret IS NOT NULL",
        id,
        step
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn disable_totp(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE users SET totp_secret = NULL, totp_enabled_at = NULL, totp_last_step = NULL WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Burns a time step, so the same code cannot sign in twice. False when a later (or the
/// same) step was already used: two logins raced with one code.
pub async fn use_totp_step(db: impl PgExecutor<'_>, id: i64, step: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE users SET totp_last_step = $2
         WHERE id = $1 AND (totp_last_step IS NULL OR totp_last_step < $2)",
        id,
        step
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Every sealed two-factor secret, for re-sealing at startup (legacy plain values).
pub async fn plain_totp_secrets(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<(i64, String)>> {
    let rows = sqlx::query!(
        r#"SELECT id, totp_secret AS "secret!" FROM users
           WHERE totp_secret IS NOT NULL AND totp_secret NOT LIKE 'v1:%'"#
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.secret)).collect())
}

/// The account behind an address, for sign-in through Google: (id, is_active).
pub async fn find_id_by_email(
    db: impl PgExecutor<'_>,
    email: &str,
) -> sqlx::Result<Option<(i64, bool)>> {
    let row = sqlx::query!(
        "SELECT id, is_active FROM users WHERE lower(email) = lower($1)",
        email.trim()
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| (r.id, r.is_active)))
}

pub async fn reseal_totp(db: impl PgExecutor<'_>, id: i64, sealed: &str) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE users SET totp_secret = $2 WHERE id = $1",
        id,
        sealed
    )
    .execute(db)
    .await?;
    Ok(())
}
