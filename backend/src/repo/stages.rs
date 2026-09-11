//! Stage history for leads and orders. Current stage = latest row (entered_at, then id).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize)]
pub struct StageEntry {
    pub id: i64,
    pub stage_key: String,
    pub label_hu: String,
    pub entered_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
    pub entered_by: Option<i64>,
    pub entered_by_name: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentStage {
    pub stage_key: String,
    pub entered_at: DateTime<Utc>,
}

pub async fn order_history(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<StageEntry>> {
    sqlx::query_as!(
        StageEntry,
        r#"SELECT os.id, os.stage_key, sd.label_hu, os.entered_at,
                  lead(os.entered_at) OVER (ORDER BY os.entered_at, os.id) AS left_at,
                  os.entered_by, u.display_name AS "entered_by_name?", os.note
           FROM order_stages os
           JOIN stage_definitions sd ON sd.entity = os.stage_entity AND sd.key = os.stage_key
           LEFT JOIN users u ON u.id = os.entered_by
           WHERE os.order_id = $1
           ORDER BY os.entered_at, os.id"#,
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn lead_history(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Vec<StageEntry>> {
    sqlx::query_as!(
        StageEntry,
        r#"SELECT ls.id, ls.stage_key, sd.label_hu, ls.entered_at,
                  lead(ls.entered_at) OVER (ORDER BY ls.entered_at, ls.id) AS left_at,
                  ls.entered_by, u.display_name AS "entered_by_name?", ls.note
           FROM lead_stages ls
           JOIN stage_definitions sd ON sd.entity = ls.stage_entity AND sd.key = ls.stage_key
           LEFT JOIN users u ON u.id = ls.entered_by
           WHERE ls.lead_id = $1
           ORDER BY ls.entered_at, ls.id"#,
        lead_id
    )
    .fetch_all(db)
    .await
}

pub async fn current_order_stage(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Option<CurrentStage>> {
    sqlx::query_as!(
        CurrentStage,
        "SELECT stage_key, entered_at FROM order_stages WHERE order_id = $1 ORDER BY entered_at DESC, id DESC LIMIT 1",
        order_id
    )
    .fetch_optional(db)
    .await
}

pub async fn current_lead_stage(
    db: impl PgExecutor<'_>,
    lead_id: i64,
) -> sqlx::Result<Option<CurrentStage>> {
    sqlx::query_as!(
        CurrentStage,
        "SELECT stage_key, entered_at FROM lead_stages WHERE lead_id = $1 ORDER BY entered_at DESC, id DESC LIMIT 1",
        lead_id
    )
    .fetch_optional(db)
    .await
}

// clock_timestamp(), not now(): now() is the transaction start, which can precede a
// concurrently committed stage change and scramble the history order.

pub async fn insert_order_stage(
    db: impl PgExecutor<'_>,
    order_id: i64,
    stage_key: &str,
    entered_by: Option<i64>,
    note: Option<&str>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO order_stages (order_id, stage_key, entered_at, entered_by, note)
         VALUES ($1, $2, clock_timestamp(), $3, $4) RETURNING id",
        order_id,
        stage_key,
        entered_by,
        note
    )
    .fetch_one(db)
    .await
}

pub async fn insert_lead_stage(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    stage_key: &str,
    entered_by: Option<i64>,
    note: Option<&str>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO lead_stages (lead_id, stage_key, entered_at, entered_by, note)
         VALUES ($1, $2, clock_timestamp(), $3, $4) RETURNING id",
        lead_id,
        stage_key,
        entered_by,
        note
    )
    .fetch_one(db)
    .await
}
