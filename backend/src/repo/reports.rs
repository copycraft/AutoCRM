//! Reporting queries. Plain SQL over plain views; at this data volume every one runs in
//! milliseconds. Durations are reported as fractional days (f64): statistics, not money.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct VolumeRow {
    pub key: String,
    pub label: String,
    pub orders: i64,
    pub huf_minor: i64,
    pub eur_minor: i64,
    /// Everything normalised to HUF at each order's valuation-date MNB rate.
    pub normalized_huf_minor: i64,
    /// Orders excluded from the normalised sum because no rate was available.
    pub missing_fx: i64,
}

pub async fn volume_by_month(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    include_cancelled: bool,
) -> sqlx::Result<Vec<VolumeRow>> {
    sqlx::query_as!(
        VolumeRow,
        r#"SELECT to_char(o.valuation_date, 'YYYY-MM') AS "key!", to_char(o.valuation_date, 'YYYY-MM') AS "label!",
                  count(*) AS "orders!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'HUF'), 0)::bigint AS "huf_minor!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'EUR'), 0)::bigint AS "eur_minor!",
                  coalesce(sum(ov.total_huf_minor), 0)::bigint AS "normalized_huf_minor!",
                  count(*) FILTER (WHERE ov.total_huf_minor IS NULL) AS "missing_fx!"
           FROM orders o
           JOIN order_values ov ON ov.order_id = o.id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           WHERE o.valuation_date BETWEEN $1 AND $2 AND ($3 OR NOT sd.is_exit)
           GROUP BY 1, 2 ORDER BY 1"#,
        from,
        to,
        include_cancelled
    )
    .fetch_all(db)
    .await
}

pub async fn volume_by_partner(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    include_cancelled: bool,
) -> sqlx::Result<Vec<VolumeRow>> {
    sqlx::query_as!(
        VolumeRow,
        r#"SELECT p.id::text AS "key!", p.name AS "label!",
                  count(*) AS "orders!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'HUF'), 0)::bigint AS "huf_minor!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'EUR'), 0)::bigint AS "eur_minor!",
                  coalesce(sum(ov.total_huf_minor), 0)::bigint AS "normalized_huf_minor!",
                  count(*) FILTER (WHERE ov.total_huf_minor IS NULL) AS "missing_fx!"
           FROM orders o
           JOIN partners p ON p.id = o.partner_id
           JOIN order_values ov ON ov.order_id = o.id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           WHERE o.valuation_date BETWEEN $1 AND $2 AND ($3 OR NOT sd.is_exit)
           GROUP BY p.id, p.name ORDER BY 6 DESC, 2"#,
        from,
        to,
        include_cancelled
    )
    .fetch_all(db)
    .await
}

pub async fn volume_by_project_type(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    include_cancelled: bool,
) -> sqlx::Result<Vec<VolumeRow>> {
    sqlx::query_as!(
        VolumeRow,
        r#"SELECT coalesce(pt.key, 'unassigned') AS "key!", coalesce(pt.label_hu, '(nincs megadva)') AS "label!",
                  count(*) AS "orders!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'HUF'), 0)::bigint AS "huf_minor!",
                  coalesce(sum(ov.total_minor) FILTER (WHERE o.currency = 'EUR'), 0)::bigint AS "eur_minor!",
                  coalesce(sum(ov.total_huf_minor), 0)::bigint AS "normalized_huf_minor!",
                  count(*) FILTER (WHERE ov.total_huf_minor IS NULL) AS "missing_fx!"
           FROM orders o
           LEFT JOIN project_types pt ON pt.id = o.project_type_id
           JOIN order_values ov ON ov.order_id = o.id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           WHERE o.valuation_date BETWEEN $1 AND $2 AND ($3 OR NOT sd.is_exit)
           GROUP BY pt.key, pt.label_hu ORDER BY 6 DESC, 2"#,
        from,
        to,
        include_cancelled
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StageDurationRow {
    pub stage_key: String,
    pub label: String,
    pub position: i32,
    /// Stage visits that started in the period.
    pub visits: i64,
    pub currently_in_stage: i64,
    /// Statistics over finished visits only; open visits would understate them.
    pub avg_days: Option<f64>,
    pub median_days: Option<f64>,
    pub p90_days: Option<f64>,
}

pub async fn stage_durations(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    project_type_id: Option<i64>,
) -> sqlx::Result<Vec<StageDurationRow>> {
    sqlx::query_as!(
        StageDurationRow,
        r#"SELECT i.stage_key AS "stage_key!", sd.label_hu AS "label!", sd.position AS "position!",
                  count(*) AS "visits!",
                  count(*) FILTER (WHERE i.left_at IS NULL) AS "currently_in_stage!",
                  (avg(extract(epoch FROM i.duration)) FILTER (WHERE i.left_at IS NOT NULL) / 86400)::float8 AS avg_days,
                  (percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM i.duration)) FILTER (WHERE i.left_at IS NOT NULL) / 86400)::float8 AS median_days,
                  (percentile_cont(0.9) WITHIN GROUP (ORDER BY extract(epoch FROM i.duration)) FILTER (WHERE i.left_at IS NOT NULL) / 86400)::float8 AS p90_days
           FROM order_stage_intervals i
           JOIN orders o ON o.id = i.order_id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = i.stage_key
           WHERE i.entered_at >= $1 AND i.entered_at < $2 AND NOT sd.is_terminal
             AND ($3::bigint IS NULL OR o.project_type_id = $3)
           GROUP BY i.stage_key, sd.label_hu, sd.position
           ORDER BY sd.position"#,
        from,
        to,
        project_type_id
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ThroughputRow {
    pub period: String,
    pub completed: i64,
    /// First stage entry → completion, in days.
    pub median_lead_days: Option<f64>,
    pub avg_lead_days: Option<f64>,
}

pub async fn throughput(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    tz: &str,
) -> sqlx::Result<Vec<ThroughputRow>> {
    sqlx::query_as!(
        ThroughputRow,
        r#"WITH done AS (
               SELECT o.id, fin.entered_at AS completed_at,
                      (SELECT min(s.entered_at) FROM order_stages s WHERE s.order_id = o.id) AS started_at
               FROM orders o
               JOIN order_current_stage fin ON fin.order_id = o.id
               JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = fin.stage_key
               WHERE sd.is_terminal AND NOT sd.is_exit AND fin.entered_at >= $1 AND fin.entered_at < $2
           )
           SELECT to_char(date_trunc('month', completed_at AT TIME ZONE $3), 'YYYY-MM') AS "period!",
                  count(*) AS "completed!",
                  (percentile_cont(0.5) WITHIN GROUP (ORDER BY extract(epoch FROM completed_at - started_at)) / 86400)::float8 AS median_lead_days,
                  (avg(extract(epoch FROM completed_at - started_at)) / 86400)::float8 AS avg_lead_days
           FROM done
           GROUP BY 1 ORDER BY 1"#,
        from,
        to,
        tz
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StalledOrder {
    pub order_id: i64,
    pub number: String,
    pub title: String,
    pub partner_name: String,
    pub stage_key: String,
    pub stage_label: String,
    pub entered_at: DateTime<Utc>,
    pub days_in_stage: i32,
    pub stall_after_days: i32,
    pub open_blockers: i64,
}

pub async fn stalled_orders(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<StalledOrder>> {
    sqlx::query_as!(
        StalledOrder,
        r#"SELECT o.id AS order_id, o.number, o.title, p.name AS "partner_name!",
                  cs.stage_key AS "stage_key!", sd.label_hu AS "stage_label!", cs.entered_at AS "entered_at!",
                  extract(day FROM now() - cs.entered_at)::int AS "days_in_stage!",
                  sd.stall_after_days AS "stall_after_days!",
                  (SELECT count(*) FROM blockers b WHERE b.order_id = o.id AND b.resolved_at IS NULL) AS "open_blockers!"
           FROM orders o
           JOIN partners p ON p.id = o.partner_id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           WHERE NOT sd.is_terminal AND sd.stall_after_days IS NOT NULL
             AND cs.entered_at < now() - make_interval(days => sd.stall_after_days)
           ORDER BY cs.entered_at"#
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct BlockerLoadRow {
    pub partner_id: Option<i64>,
    pub responsible: String,
    pub blockers: i64,
    pub open: i64,
    pub overdue: i64,
    pub nudges: i64,
    /// Sum of (resolved or now − created) across blockers, in days.
    pub waiting_days: f64,
}

pub async fn blocker_load(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    today: NaiveDate,
) -> sqlx::Result<Vec<BlockerLoadRow>> {
    sqlx::query_as!(
        BlockerLoadRow,
        r#"SELECT b.responsible_partner_id AS partner_id,
                  coalesce(p.name, b.responsible_email, '(nincs felelős)') AS "responsible!",
                  count(*) AS "blockers!",
                  count(*) FILTER (WHERE b.resolved_at IS NULL) AS "open!",
                  count(*) FILTER (WHERE b.resolved_at IS NULL AND b.due_date < $3) AS "overdue!",
                  coalesce(sum(b.nudge_count), 0)::bigint AS "nudges!",
                  (sum(extract(epoch FROM coalesce(b.resolved_at, now()) - b.created_at)) / 86400)::float8 AS "waiting_days!"
           FROM blockers b
           LEFT JOIN partners p ON p.id = b.responsible_partner_id
           WHERE b.created_at >= $1 AND b.created_at < $2
           GROUP BY b.responsible_partner_id, coalesce(p.name, b.responsible_email, '(nincs felelős)')
           ORDER BY 7 DESC"#,
        from,
        to,
        today
    )
    .fetch_all(db)
    .await
}

/// One order's workshop interval, in business-tz calendar dates: the day it was placed
/// and, once it reaches a terminal stage, the day it left. The per-day buckets are built
/// in Rust (ranges are weeks, not years), so SQL only selects candidate intervals.
#[derive(Debug, Clone)]
pub struct WorkloadInterval {
    pub placed: NaiveDate,
    pub completed: Option<NaiveDate>,
}

pub async fn workload_intervals(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    tz: &str,
) -> sqlx::Result<Vec<WorkloadInterval>> {
    sqlx::query_as!(
        WorkloadInterval,
        r#"SELECT (o.created_at AT TIME ZONE $3)::date AS "placed!",
                  CASE WHEN sd.is_terminal
                       THEN (fin.entered_at AT TIME ZONE $3)::date
                       ELSE NULL::date END AS "completed?"
           FROM orders o
           JOIN order_current_stage fin ON fin.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = fin.stage_key
           WHERE (o.created_at AT TIME ZONE $3)::date <= $2
             AND (NOT sd.is_terminal OR (fin.entered_at AT TIME ZONE $3)::date >= $1)"#,
        from,
        to,
        tz
    )
    .fetch_all(db)
    .await
}
