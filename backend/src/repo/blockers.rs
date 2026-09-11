use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Blocker {
    pub id: i64,
    pub order_id: i64,
    pub order_number: String,
    pub what: String,
    pub responsible_partner_id: Option<i64>,
    pub responsible_partner_name: Option<String>,
    pub responsible_email: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub nudge_enabled: bool,
    pub last_nudged_at: Option<DateTime<Utc>>,
    pub nudge_count: i32,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<i64>,
    pub resolution_note: Option<String>,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Past due date and still open, measured against the caller's business day
    /// (not UTC midnight — the whole point of this flag).
    pub is_overdue: bool,
}

#[derive(Debug, Clone)]
pub struct BlockerInput {
    pub what: String,
    pub responsible_partner_id: Option<i64>,
    pub responsible_email: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub nudge_enabled: bool,
}

pub async fn find(
    db: impl PgExecutor<'_>,
    id: i64,
    today: NaiveDate,
) -> sqlx::Result<Option<Blocker>> {
    sqlx::query_as!(
        Blocker,
        r#"SELECT b.id, b.order_id, o.number AS order_number, b.what, b.responsible_partner_id,
                  p.name AS "responsible_partner_name?", b.responsible_email, b.due_date, b.notes, b.nudge_enabled,
                  b.last_nudged_at, b.nudge_count, b.resolved_at, b.resolved_by, b.resolution_note,
                  b.created_by, b.created_at, b.updated_at,
                  (b.due_date < $2 AND b.resolved_at IS NULL) AS "is_overdue!"
           FROM blockers b
           JOIN orders o ON o.id = b.order_id
           LEFT JOIN partners p ON p.id = b.responsible_partner_id
           WHERE b.id = $1"#,
        id,
        today
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
    today: NaiveDate,
) -> sqlx::Result<Vec<Blocker>> {
    sqlx::query_as!(
        Blocker,
        r#"SELECT b.id, b.order_id, o.number AS order_number, b.what, b.responsible_partner_id,
                  p.name AS "responsible_partner_name?", b.responsible_email, b.due_date, b.notes, b.nudge_enabled,
                  b.last_nudged_at, b.nudge_count, b.resolved_at, b.resolved_by, b.resolution_note,
                  b.created_by, b.created_at, b.updated_at,
                  (b.due_date < $2 AND b.resolved_at IS NULL) AS "is_overdue!"
           FROM blockers b
           JOIN orders o ON o.id = b.order_id
           LEFT JOIN partners p ON p.id = b.responsible_partner_id
           WHERE b.order_id = $1
           ORDER BY b.resolved_at NULLS FIRST, b.due_date NULLS LAST, b.id"#,
        order_id,
        today
    )
    .fetch_all(db)
    .await
}

pub async fn list_open(
    db: impl PgExecutor<'_>,
    responsible_partner_id: Option<i64>,
    limit: i64,
    offset: i64,
    today: NaiveDate,
) -> sqlx::Result<Vec<Blocker>> {
    sqlx::query_as!(
        Blocker,
        r#"SELECT b.id, b.order_id, o.number AS order_number, b.what, b.responsible_partner_id,
                  p.name AS "responsible_partner_name?", b.responsible_email, b.due_date, b.notes, b.nudge_enabled,
                  b.last_nudged_at, b.nudge_count, b.resolved_at, b.resolved_by, b.resolution_note,
                  b.created_by, b.created_at, b.updated_at,
                  (b.due_date < $4 AND b.resolved_at IS NULL) AS "is_overdue!"
           FROM blockers b
           JOIN orders o ON o.id = b.order_id
           LEFT JOIN partners p ON p.id = b.responsible_partner_id
           WHERE b.resolved_at IS NULL AND ($1::bigint IS NULL OR b.responsible_partner_id = $1)
           ORDER BY b.due_date NULLS LAST, b.id
           LIMIT $2 OFFSET $3"#,
        responsible_partner_id,
        limit,
        offset,
        today
    )
    .fetch_all(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    order_id: i64,
    b: &BlockerInput,
    created_by: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO blockers (order_id, what, responsible_partner_id, responsible_email, due_date, notes, nudge_enabled, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        order_id,
        b.what,
        b.responsible_partner_id,
        b.responsible_email,
        b.due_date,
        b.notes,
        b.nudge_enabled,
        created_by
    )
    .fetch_one(db)
    .await
}

pub async fn update(db: impl PgExecutor<'_>, id: i64, b: &BlockerInput) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE blockers
         SET what = $2, responsible_partner_id = $3, responsible_email = $4, due_date = $5, notes = $6, nudge_enabled = $7
         WHERE id = $1",
        id,
        b.what,
        b.responsible_partner_id,
        b.responsible_email,
        b.due_date,
        b.notes,
        b.nudge_enabled
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn resolve(
    db: impl PgExecutor<'_>,
    id: i64,
    user_id: i64,
    note: Option<&str>,
) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE blockers SET resolved_at = now(), resolved_by = $2, resolution_note = $3
         WHERE id = $1 AND resolved_at IS NULL",
        id,
        user_id,
        note
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Open, nudge-enabled blockers whose due date has passed, with the address a nudge would go to.
pub struct NudgeCandidate {
    pub id: i64,
    pub order_id: i64,
    pub due_date: Option<NaiveDate>,
    pub nudge_enabled: bool,
    pub last_nudged_at: Option<DateTime<Utc>>,
    pub nudge_count: i32,
    pub recipient: Option<String>,
}

pub async fn nudge_candidates(
    db: impl PgExecutor<'_>,
    today: NaiveDate,
) -> sqlx::Result<Vec<NudgeCandidate>> {
    sqlx::query_as!(
        NudgeCandidate,
        r#"SELECT b.id, b.order_id, b.due_date, b.nudge_enabled, b.last_nudged_at, b.nudge_count,
                  coalesce(b.responsible_email, p.email) AS recipient
           FROM blockers b
           LEFT JOIN partners p ON p.id = b.responsible_partner_id
           WHERE b.resolved_at IS NULL AND b.nudge_enabled AND b.due_date < $1
           ORDER BY b.due_date, b.id"#,
        today
    )
    .fetch_all(db)
    .await
}

/// Locks the blocker row and returns its current nudge count, if still open and nudgeable.
pub async fn lock_for_nudge(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<i32>> {
    sqlx::query_scalar!(
        "SELECT nudge_count FROM blockers WHERE id = $1 AND resolved_at IS NULL AND nudge_enabled FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn record_nudge(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE blockers SET nudge_count = nudge_count + 1, last_nudged_at = now() WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reopen(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE blockers SET resolved_at = NULL, resolved_by = NULL, resolution_note = NULL
         WHERE id = $1 AND resolved_at IS NOT NULL",
        id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}
