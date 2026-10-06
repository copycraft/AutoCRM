//! Why a lead was lost: a short list the office keeps. Runtime-checked queries.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LostReason {
    pub id: i64,
    pub label: String,
    pub position: i32,
    pub archived_at: Option<DateTime<Utc>>,
    /// Leads lost for this reason.
    pub leads: i64,
}

const SELECT: &str = "SELECT r.id, r.label, r.position, r.archived_at,
        (SELECT count(*) FROM leads l WHERE l.lost_reason_id = r.id) AS leads
    FROM lost_reasons r";

pub async fn list(db: impl PgExecutor<'_>, archived: bool) -> sqlx::Result<Vec<LostReason>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE ($1 OR r.archived_at IS NULL) ORDER BY r.archived_at NULLS FIRST, r.position, r.id"
    ))
    .bind(archived)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<LostReason>> {
    sqlx::query_as(&format!("{SELECT} WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn insert(db: impl PgExecutor<'_>, label: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO lost_reasons (label, position)
         VALUES ($1, coalesce((SELECT max(position) FROM lost_reasons), 0) + 10) RETURNING id",
    )
    .bind(label)
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    label: &str,
    archived: bool,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE lost_reasons SET label = $2,
                archived_at = CASE WHEN $3 THEN coalesce(archived_at, now()) END
         WHERE id = $1",
    )
    .bind(id)
    .bind(label)
    .bind(archived)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Sets (or clears) the reason on a lead.
pub async fn set_for_lead(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    reason_id: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE leads SET lost_reason_id = $2 WHERE id = $1")
        .bind(lead_id)
        .bind(reason_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn for_lead(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Option<LostReason>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE r.id = (SELECT lost_reason_id FROM leads WHERE id = $1)"
    ))
    .bind(lead_id)
    .fetch_optional(db)
    .await
}

/// How many leads were lost for each reason, among leads created in a period.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ReasonCount {
    /// "Nincs megadva" when the lead was lost without a reason.
    pub reason: String,
    pub leads: i64,
}

pub async fn breakdown(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<ReasonCount>> {
    sqlx::query_as(
        "SELECT coalesce(r.label, 'Nincs megadva') AS reason, count(*) AS leads
           FROM leads l
           JOIN lead_current_stage cs ON cs.lead_id = l.id AND cs.stage_key = 'lost'
           LEFT JOIN lost_reasons r ON r.id = l.lost_reason_id
          WHERE l.created_at >= $1 AND l.created_at < $2
          GROUP BY 1 ORDER BY 2 DESC, 1",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}
