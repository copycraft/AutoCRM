//! Newsletter subscriptions: the list the office sees under settings, and the audience
//! of a BCC blast.
//!
//! Website signups arrive through the public endpoint (source `website`); hand-added rows
//! say `office`. Unsubscribing sets `unsubscribed_at` rather than deleting: a deleted
//! address would silently resubscribe on the next import. Matching is case-insensitive —
//! `Janos@X.HU` and `janos@x.hu` are the same subscriber.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::domain::email::normalize_address;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Subscription {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub source: String,
    pub subscribed_at: DateTime<Utc>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Subscription>> {
    sqlx::query_as!(
        Subscription,
        r#"SELECT id, email, name, source, subscribed_at, unsubscribed_at
           FROM newsletter_subscriptions ORDER BY subscribed_at DESC"#,
    )
    .fetch_all(db)
    .await
}

pub async fn count_active(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "SELECT count(*) FROM newsletter_subscriptions WHERE unsubscribed_at IS NULL"
    )
    .fetch_one(db)
    .await
    .map(|c: Option<i64>| c.unwrap_or(0))
}

/// Active subscriber addresses, lowercased and deduplicated. The blast filters these
/// against the global suppression list before sending.
pub async fn active_emails(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!(
        "SELECT DISTINCT lower(email) FROM newsletter_subscriptions WHERE unsubscribed_at IS NULL"
    )
    .fetch_all(db)
    .await
    .map(|rows| rows.into_iter().flatten().collect())
}

/// Subscribe (or resubscribe) an address. Idempotent: subscribing twice is not an error,
///
/// and a returning address is welcomed back — unsubscribed_at is cleared.
pub async fn subscribe(
    db: impl PgExecutor<'_>,
    email: &str,
    name: &str,
    source: &str,
) -> sqlx::Result<Subscription> {
    let Some(normalized) = normalize_address(email) else {
        return Err(sqlx::Error::RowNotFound);
    };
    let name = name.trim();
    sqlx::query_as!(
        Subscription,
        r#"INSERT INTO newsletter_subscriptions (email, name, source)
           VALUES ($1, $2, $3)
           ON CONFLICT (email) DO UPDATE SET name = excluded.name, unsubscribed_at = NULL
           RETURNING id, email, name, source, subscribed_at, unsubscribed_at"#,
        normalized,
        name,
        source,
    )
    .fetch_one(db)
    .await
}

pub async fn unsubscribe_by_token(
    db: impl PgExecutor<'_>,
    token: &str,
) -> sqlx::Result<Option<Subscription>> {
    sqlx::query_as!(
        Subscription,
        r#"UPDATE newsletter_subscriptions SET unsubscribed_at = now()
           WHERE unsubscribe_token = $1 AND unsubscribed_at IS NULL
           RETURNING id, email, name, source, subscribed_at, unsubscribed_at"#,
        token,
    )
    .fetch_optional(db)
    .await
}

pub async fn remove(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        "DELETE FROM newsletter_subscriptions WHERE id = $1 RETURNING id",
        id
    )
    .fetch_optional(db)
    .await
    .map(|r: Option<i64>| r.is_some())
}
