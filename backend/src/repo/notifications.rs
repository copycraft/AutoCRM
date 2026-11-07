//! The per-user notification feed. Runtime-checked queries.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use crate::domain::role::Capability;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NotificationRow {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    pub link: Option<String>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

/// One notification for every active user who holds `need`. Returns how many.
///
/// The audience is a right, not a list of roles: whoever the admin's grid or a per-user
/// grant lets do the work hears about it, and nobody else. The rule is
/// `domain::role::holds`, in SQL — admins; the role's set in the grid; per-user grants of a
/// grantable one; `hr_access` for the HR module and its parts.
pub async fn broadcast(
    db: impl PgExecutor<'_>,
    need: Capability,
    kind: &str,
    title: &str,
    body: Option<&str>,
    link: Option<&str>,
) -> sqlx::Result<u64> {
    // Null for one that cannot be granted (AccessHr, ManageUsers): neither the grid nor a
    // grant can hold it, so only admins and, for HR, the flag count.
    let grantable = need.grantable().then(|| need.key());
    let result = sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, body, link)
         SELECT u.id, $1, $2, $3, $4 FROM users u
         WHERE u.is_active AND (
             u.role::text = 'admin'
             OR ($6 AND u.hr_access)
             OR $5 = ANY(u.permissions)
             OR EXISTS (SELECT 1 FROM role_permissions rp
                        WHERE rp.role = u.role AND rp.capability = $5)
         )",
    )
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(link)
    .bind(grantable)
    .bind(need.comes_with_hr_access())
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// One notification for one user, if still active. Returns how many (0 or 1).
pub async fn notify_user(
    db: impl PgExecutor<'_>,
    user_id: i64,
    kind: &str,
    title: &str,
    body: Option<&str>,
    link: Option<&str>,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "INSERT INTO notifications (user_id, kind, title, body, link)
         SELECT id, $2, $3, $4, $5 FROM users WHERE id = $1 AND is_active",
    )
    .bind(user_id)
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(link)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// The user's newest notifications, optionally only those after `after_id` and/or unread.
pub async fn list(
    db: impl PgExecutor<'_>,
    user_id: i64,
    after_id: Option<i64>,
    unread_only: bool,
    limit: i64,
) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as(
        "SELECT id, kind, title, body, link, created_at, read_at FROM notifications
         WHERE user_id = $1 AND ($2::bigint IS NULL OR id > $2) AND (NOT $3 OR read_at IS NULL)
         ORDER BY id DESC LIMIT $4",
    )
    .bind(user_id)
    .bind(after_id)
    .bind(unread_only)
    .bind(limit)
    .fetch_all(db)
    .await
}

pub async fn unread_count(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
        .bind(user_id)
        .fetch_one(db)
        .await
}

/// Marks the user's own notifications read (all of them when `ids` is None).
pub async fn mark_read(
    db: impl PgExecutor<'_>,
    user_id: i64,
    ids: Option<&[i64]>,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE notifications SET read_at = now()
         WHERE user_id = $1 AND read_at IS NULL AND ($2::bigint[] IS NULL OR id = ANY($2))",
    )
    .bind(user_id)
    .bind(ids)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Housekeeping: the feed is for recent news, not an archive.
pub async fn purge_older_than_days(db: impl PgExecutor<'_>, days: i32) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "DELETE FROM notifications WHERE created_at < now() - make_interval(days => $1)",
    )
    .bind(days)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}
