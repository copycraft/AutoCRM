use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

use crate::domain::role::Role;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "session_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    Web,
    Mobile,
}

pub struct NewSession<'a> {
    pub user_id: i64,
    pub token_hash: &'a [u8],
    pub kind: SessionKind,
    pub device_label: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub ip: Option<&'a str>,
    pub expires_at: DateTime<Utc>,
}

pub async fn insert(db: impl PgExecutor<'_>, s: NewSession<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO sessions (user_id, token_hash, kind, device_label, user_agent, ip, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        s.user_id,
        s.token_hash,
        s.kind as SessionKind,
        s.device_label,
        s.user_agent,
        s.ip,
        s.expires_at
    )
    .fetch_one(db)
    .await
}

pub struct ActiveSession {
    pub session_id: i64,
    pub user_id: i64,
    pub kind: SessionKind,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub must_change_password: bool,
    pub hr_access: bool,
}

pub async fn find_active(
    db: impl PgExecutor<'_>,
    token_hash: &[u8],
) -> sqlx::Result<Option<ActiveSession>> {
    sqlx::query_as!(
        ActiveSession,
        r#"SELECT s.id AS session_id, s.user_id, s.kind AS "kind: SessionKind", s.created_at, s.last_seen_at,
                  u.email, u.display_name, u.role AS "role: Role", u.must_change_password, u.hr_access
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now() AND u.is_active"#,
        token_hash
    )
    .fetch_optional(db)
    .await
}

pub async fn touch(
    db: impl PgExecutor<'_>,
    id: i64,
    expires_at: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE sessions SET last_seen_at = now(), expires_at = $2 WHERE id = $1",
        id,
        expires_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn revoke(db: impl PgExecutor<'_>, id: i64, user_id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE sessions SET revoked_at = now() WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Revokes every live session for a user, optionally keeping one (the caller's own).
pub async fn revoke_all(
    db: impl PgExecutor<'_>,
    user_id: i64,
    except: Option<i64>,
) -> sqlx::Result<u64> {
    let r = sqlx::query!(
        "UPDATE sessions SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND ($2::bigint IS NULL OR id <> $2)",
        user_id,
        except
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SessionInfo {
    pub id: i64,
    pub kind: SessionKind,
    pub device_label: Option<String>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn list_active(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<Vec<SessionInfo>> {
    sqlx::query_as!(
        SessionInfo,
        r#"SELECT id, kind AS "kind: SessionKind", device_label, user_agent, ip, created_at, last_seen_at, expires_at
           FROM sessions
           WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
           ORDER BY last_seen_at DESC"#,
        user_id
    )
    .fetch_all(db)
    .await
}
