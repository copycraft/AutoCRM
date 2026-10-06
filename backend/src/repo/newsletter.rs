//! Newsletter subscriptions: the list the office sees under settings, and the audience
//! of a BCC blast.
//!
//! Website signups arrive through the public endpoint (source `website`) as a pending
//! request; the address joins the audience only when its owner clicks the confirmation
//! link (double opt-in). Hand-added rows say `office` and are confirmed on insert.
//! Unsubscribing sets `unsubscribed_at` rather than deleting: a deleted address would
//! silently resubscribe on the next import. Matching is case-insensitive — `Janos@X.HU`
//! and `janos@x.hu` are the same subscriber.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::domain::email::normalize_address;

/// How long a confirmation link stays valid.
pub const CONFIRM_TTL_DAYS: i32 = 7;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Subscription {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub source: String,
    pub subscribed_at: DateTime<Utc>,
    /// Null while a website signup waits for its confirmation click.
    pub confirmed_at: Option<DateTime<Utc>>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Subscription>> {
    sqlx::query_as!(
        Subscription,
        r#"SELECT id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at
           FROM newsletter_subscriptions ORDER BY subscribed_at DESC"#,
    )
    .fetch_all(db)
    .await
}

pub async fn count_active(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "SELECT count(*) FROM newsletter_subscriptions
          WHERE confirmed_at IS NOT NULL AND unsubscribed_at IS NULL"
    )
    .fetch_one(db)
    .await
    .map(|c: Option<i64>| c.unwrap_or(0))
}

/// Subscribe an address, confirmed. For the office hand-add, where the office holds the
/// consent. Idempotent: subscribing twice is not an error, and a returning address is
/// welcomed back — unsubscribed_at is cleared. The caller decides whether it may be.
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
        r#"INSERT INTO newsletter_subscriptions (email, name, source, confirmed_at)
           VALUES ($1, $2, $3, now())
           ON CONFLICT (email) DO UPDATE
              SET name = excluded.name, unsubscribed_at = NULL,
                  confirmed_at = coalesce(newsletter_subscriptions.confirmed_at, now()),
                  confirm_token = NULL, confirm_requested_at = NULL
           RETURNING id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at"#,
        normalized,
        name,
        source,
    )
    .fetch_one(db)
    .await
}

/// A website signup waiting for its confirmation click.
#[derive(Debug)]
pub struct PendingConfirmation {
    pub id: i64,
    pub email: String,
    pub token: String,
}

/// Records a website signup and returns the confirmation to mail, or None when the
/// address is already an active subscriber (nothing to confirm, nothing to send).
///
/// Nothing here makes the address active or clears an earlier opt-out: only
/// [`confirm`] does, when the owner of the address clicks the link. A still-valid
/// token is reused, so repeated signups do not invalidate a link already sent; the
/// name of an active subscriber is never overwritten by an unauthenticated form.
pub async fn request_confirmation(
    db: impl PgExecutor<'_>,
    email: &str,
    name: &str,
) -> sqlx::Result<Option<PendingConfirmation>> {
    let Some(normalized) = normalize_address(email) else {
        return Err(sqlx::Error::RowNotFound);
    };
    let row = sqlx::query!(
        r#"INSERT INTO newsletter_subscriptions
                  (email, name, source, confirm_token, confirm_requested_at)
           VALUES ($1, $2, 'website', encode(gen_random_bytes(24), 'hex'), now())
           ON CONFLICT (email) DO UPDATE SET
               name = CASE WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                                AND newsletter_subscriptions.unsubscribed_at IS NULL
                           THEN newsletter_subscriptions.name ELSE excluded.name END,
               confirm_token = CASE
                   WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                        AND newsletter_subscriptions.unsubscribed_at IS NULL THEN NULL
                   WHEN newsletter_subscriptions.confirm_token IS NOT NULL
                        AND newsletter_subscriptions.confirm_requested_at
                            > now() - make_interval(days => $3)
                        THEN newsletter_subscriptions.confirm_token
                   ELSE excluded.confirm_token END,
               confirm_requested_at = CASE
                   WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                        AND newsletter_subscriptions.unsubscribed_at IS NULL THEN NULL
                   ELSE now() END
           RETURNING id, email, confirm_token"#,
        normalized,
        name.trim(),
        CONFIRM_TTL_DAYS,
    )
    .fetch_one(db)
    .await?;
    Ok(row.confirm_token.map(|token| PendingConfirmation {
        id: row.id,
        email: row.email,
        token,
    }))
}

/// The confirmation click: the address joins the audience, and an earlier opt-out is
/// lifted because its owner asked. None for an unknown, used or expired token.
pub async fn confirm(db: impl PgExecutor<'_>, token: &str) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!(
        "UPDATE newsletter_subscriptions
            SET confirmed_at = now(), unsubscribed_at = NULL,
                confirm_token = NULL, confirm_requested_at = NULL
          WHERE confirm_token = $1
            AND confirm_requested_at > now() - make_interval(days => $2)
          RETURNING id",
        token,
        CONFIRM_TTL_DAYS,
    )
    .fetch_optional(db)
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
           RETURNING id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at"#,
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
