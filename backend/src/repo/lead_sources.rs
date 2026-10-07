//! Where leads come from: the list the office edits (0048). `website` and `minicrm` are the
//! system's own and never offered by hand.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LeadSource {
    pub key: String,
    pub label: String,
    pub position: i32,
    /// Set by the system (website form, MiniCRM import): shown, never picked by hand.
    pub is_system: bool,
    pub archived_at: Option<DateTime<Utc>>,
    /// Leads filed under it.
    pub leads: i64,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    include_archived: bool,
) -> sqlx::Result<Vec<LeadSource>> {
    sqlx::query_as!(
        LeadSource,
        r#"SELECT s.key, s.label, s.position, s.is_system, s.archived_at,
                  (SELECT count(*) FROM leads l WHERE l.source = s.key) AS "leads!"
           FROM lead_sources s
           WHERE $1 OR s.archived_at IS NULL
           ORDER BY s.position, s.label"#,
        include_archived
    )
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, key: &str) -> sqlx::Result<Option<LeadSource>> {
    sqlx::query_as!(
        LeadSource,
        r#"SELECT s.key, s.label, s.position, s.is_system, s.archived_at,
                  (SELECT count(*) FROM leads l WHERE l.source = s.key) AS "leads!"
           FROM lead_sources s WHERE s.key = $1"#,
        key
    )
    .fetch_optional(db)
    .await
}

/// The live source whose key or label (any case) is `text`, if any.
pub async fn matching(db: impl PgExecutor<'_>, text: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!(
        "SELECT key FROM lead_sources
          WHERE archived_at IS NULL AND (key = lower(btrim($1)) OR lower(label) = lower(btrim($1)))
          ORDER BY position LIMIT 1",
        text
    )
    .fetch_optional(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    key: &str,
    label: &str,
    position: i32,
) -> sqlx::Result<LeadSource> {
    sqlx::query_as!(
        LeadSource,
        r#"INSERT INTO lead_sources (key, label, position) VALUES ($1, $2, $3)
           RETURNING key, label, position, is_system, archived_at, 0::bigint AS "leads!""#,
        key,
        label,
        position
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    key: &str,
    label: &str,
    position: i32,
    archived: bool,
) -> sqlx::Result<Option<LeadSource>> {
    sqlx::query_as!(
        LeadSource,
        r#"UPDATE lead_sources
              SET label = $2, position = $3,
                  archived_at = CASE WHEN $4 THEN coalesce(archived_at, now()) ELSE NULL END
            WHERE key = $1
        RETURNING key, label, position, is_system, archived_at,
                  (SELECT count(*) FROM leads l WHERE l.source = lead_sources.key) AS "leads!""#,
        key,
        label,
        position,
        archived
    )
    .fetch_optional(db)
    .await
}
