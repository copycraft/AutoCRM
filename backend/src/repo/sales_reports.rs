//! Sales and marketing reports (0049): the funnel, each salesperson's numbers, how fast
//! leads hear back, revenue by country and year, cumulative flow, and newsletter trends.
//! Runtime-checked queries, like the other recent report tables.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// One lead stage of the funnel: how many of the period's new leads got this far.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FunnelStage {
    pub key: String,
    pub label: String,
    pub position: i32,
    /// An exit (lost) stage: counted as "ended here", not as progress.
    pub is_exit: bool,
    pub leads: i64,
}

/// Leads created in [from, to) and the furthest stage each reached. A lead counts for every
/// stage up to the furthest it got to (it passed them), exits are counted on their own.
pub async fn funnel(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<FunnelStage>> {
    sqlx::query_as(
        "WITH cohort AS (SELECT id FROM leads WHERE created_at >= $1 AND created_at < $2),
              furthest AS (
                  SELECT ls.lead_id, max(s.position) AS pos
                    FROM lead_stages ls
                    JOIN stage_definitions s ON s.entity = 'lead' AND s.key = ls.stage_key AND NOT s.is_exit
                   WHERE ls.lead_id IN (SELECT id FROM cohort)
                   GROUP BY ls.lead_id)
         SELECT sd.key, sd.label_hu AS label, sd.position, sd.is_exit,
                CASE WHEN sd.is_exit THEN
                         (SELECT count(DISTINCT ls.lead_id) FROM lead_stages ls
                           WHERE ls.stage_key = sd.key AND ls.lead_id IN (SELECT id FROM cohort))
                     ELSE (SELECT count(*) FROM furthest f WHERE f.pos >= sd.position)
                END AS leads
           FROM stage_definitions sd
          WHERE sd.entity = 'lead' AND (sd.is_active OR sd.is_exit)
          ORDER BY sd.is_exit, sd.position, sd.id",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

/// The cohort's headline numbers.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FunnelTotals {
    pub created: i64,
    /// A price was quoted.
    pub quoted: i64,
    /// Turned into an order.
    pub won: i64,
    /// The won orders' value, normalised to HUF.
    pub won_huf_minor: i64,
}

pub async fn funnel_totals(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<FunnelTotals> {
    sqlx::query_as(
        "SELECT count(*) AS created,
                count(*) FILTER (WHERE l.quoted_value_minor IS NOT NULL) AS quoted,
                count(o.id) AS won,
                coalesce(sum(ov.total_huf_minor), 0)::bigint AS won_huf_minor
           FROM leads l
           LEFT JOIN orders o ON o.lead_id = l.id
           LEFT JOIN order_values ov ON ov.order_id = o.id
          WHERE l.created_at >= $1 AND l.created_at < $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(db)
    .await
}

/// One salesperson's period.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SalespersonRow {
    pub user_id: Option<i64>,
    /// "Nincs felelős" for unassigned leads.
    pub name: String,
    pub leads: i64,
    pub quoted: i64,
    pub won: i64,
    pub lost: i64,
    pub won_huf_minor: i64,
    /// Median hours from a lead arriving to its first human answer.
    pub median_response_hours: Option<f64>,
}

pub async fn salespeople(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<SalespersonRow>> {
    sqlx::query_as(&format!(
        "WITH cohort AS (SELECT * FROM leads WHERE created_at >= $1 AND created_at < $2),
              response AS ({FIRST_RESPONSE})
         SELECT l.assigned_to AS user_id, coalesce(u.display_name, 'Nincs felelős') AS name,
                count(*) AS leads,
                count(*) FILTER (WHERE l.quoted_value_minor IS NOT NULL) AS quoted,
                count(o.id) AS won,
                count(*) FILTER (WHERE sd.is_exit) AS lost,
                coalesce(sum(ov.total_huf_minor), 0)::bigint AS won_huf_minor,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY r.hours) AS median_response_hours
           FROM cohort l
           LEFT JOIN users u ON u.id = l.assigned_to
           LEFT JOIN orders o ON o.lead_id = l.id
           LEFT JOIN order_values ov ON ov.order_id = o.id
           LEFT JOIN lead_current_stage cs ON cs.lead_id = l.id
           LEFT JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
           LEFT JOIN response r ON r.lead_id = l.id
          GROUP BY l.assigned_to, u.display_name
          ORDER BY won_huf_minor DESC, leads DESC"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

/// Per cohort lead: hours from arriving to the first human answer — a hand-written email
/// about it, or a stage moved by a person. Leads nobody answered yet are absent.
const FIRST_RESPONSE: &str = "
    SELECT c.id AS lead_id,
           extract(epoch FROM (least(
               (SELECT min(coalesce(m.sent_at, m.queued_at)) FROM email_messages m
                 WHERE m.lead_id = c.id AND m.sent_by IS NOT NULL AND m.status::text <> 'cancelled'),
               (SELECT min(ls.entered_at) FROM lead_stages ls
                 WHERE ls.lead_id = c.id AND ls.entered_by IS NOT NULL
                   AND ls.entered_at > c.created_at + interval '1 minute')
           ) - c.created_at)) / 3600.0 AS hours
      FROM cohort c";

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ResponseRow {
    /// The lead source key, or "" for none.
    pub source: String,
    pub leads: i64,
    pub answered: i64,
    pub median_hours: Option<f64>,
    pub p90_hours: Option<f64>,
    /// Answered within 24 hours.
    pub within_day: i64,
}

/// Time to first response, by source; the row with source "*" is everyone.
pub async fn first_response(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<ResponseRow>> {
    sqlx::query_as(&format!(
        "WITH cohort AS (SELECT * FROM leads WHERE created_at >= $1 AND created_at < $2),
              response AS ({FIRST_RESPONSE})
         SELECT coalesce(CASE WHEN GROUPING(c.source) = 1 THEN '*' ELSE c.source END, '') AS source,
                count(*) AS leads,
                count(r.hours) AS answered,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY r.hours) AS median_hours,
                percentile_cont(0.9) WITHIN GROUP (ORDER BY r.hours) AS p90_hours,
                count(*) FILTER (WHERE r.hours <= 24) AS within_day
           FROM cohort c
           LEFT JOIN response r ON r.lead_id = c.id
          GROUP BY ROLLUP (c.source)
          ORDER BY GROUPING(c.source) DESC, count(*) DESC"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RevenueRow {
    pub year: i32,
    /// ISO country code of the customer.
    pub country: String,
    pub orders: i64,
    /// Normalised to HUF at each order's valuation-date rate.
    pub huf_minor: i64,
    /// Orders without a rate, left out of the sum.
    pub missing_fx: i64,
}

/// Order value per customer country and year (cancelled orders left out).
pub async fn revenue_by_country(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<RevenueRow>> {
    sqlx::query_as(
        "SELECT extract(year FROM o.valuation_date)::int AS year, p.country,
                count(*) AS orders,
                coalesce(sum(ov.total_huf_minor), 0)::bigint AS huf_minor,
                count(*) FILTER (WHERE ov.total_huf_minor IS NULL) AS missing_fx
           FROM orders o
           JOIN partners p ON p.id = o.partner_id
           JOIN order_values ov ON ov.order_id = o.id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
          WHERE o.valuation_date BETWEEN $1 AND $2 AND NOT sd.is_exit
          GROUP BY 1, 2
          ORDER BY 1, huf_minor DESC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FlowPoint {
    /// The week's last day.
    pub day: NaiveDate,
    pub stage_key: String,
    /// Orders sitting in that stage at the end of the day.
    pub orders: i64,
}

/// Cumulative flow: at the end of each week in the range, how many orders were in each
/// stage.
pub async fn cumulative_flow(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<FlowPoint>> {
    sqlx::query_as(
        "SELECT d::date AS day, i.stage_key, count(*) AS orders
           FROM generate_series($2::date, $1::date, interval '-7 days') d
           JOIN order_stage_intervals i
             ON i.entered_at < (d::date + 1)::timestamp AT TIME ZONE 'Europe/Budapest'
            AND (i.left_at IS NULL OR i.left_at >= (d::date + 1)::timestamp AT TIME ZONE 'Europe/Budapest')
          GROUP BY 1, 2
          ORDER BY 1, 2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SubscriberMonth {
    /// YYYY-MM.
    pub month: String,
    pub joined: i64,
    pub left: i64,
    /// Confirmed and still subscribed at the month's end.
    pub active_at_end: i64,
}

pub async fn subscriber_months(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<SubscriberMonth>> {
    sqlx::query_as(
        "SELECT to_char(m, 'YYYY-MM') AS month,
                (SELECT count(*) FROM newsletter_subscriptions s
                  WHERE s.confirmed_at >= m AND s.confirmed_at < m + interval '1 month') AS joined,
                (SELECT count(*) FROM newsletter_subscriptions s
                  WHERE s.unsubscribed_at >= m AND s.unsubscribed_at < m + interval '1 month') AS left,
                (SELECT count(*) FROM newsletter_subscriptions s
                  WHERE s.confirmed_at < m + interval '1 month'
                    AND (s.unsubscribed_at IS NULL OR s.unsubscribed_at >= m + interval '1 month')) AS active_at_end
           FROM generate_series(date_trunc('month', $1::date), date_trunc('month', $2::date), interval '1 month') m
          ORDER BY 1",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SendTrend {
    pub send_id: i64,
    pub subject: String,
    pub send_at: DateTime<Utc>,
    pub sent: i64,
    pub opened: i64,
    pub clicked: i64,
}

/// Each tracked send in the period with its reach, for open and click rate trends.
pub async fn send_trends(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<SendTrend>> {
    sqlx::query_as(
        "SELECT s.id AS send_id, s.subject, s.send_at,
                count(m.id) FILTER (WHERE m.status::text = 'sent') AS sent,
                count(m.id) FILTER (WHERE m.opened_at IS NOT NULL) AS opened,
                count(DISTINCT c.email_id) AS clicked
           FROM newsletter_sends s
           LEFT JOIN email_messages m ON m.newsletter_send_id = s.id
           LEFT JOIN email_clicks c ON c.email_id = m.id
          WHERE s.cancelled_at IS NULL AND s.send_at >= $1 AND s.send_at < $2
          GROUP BY s.id
          ORDER BY s.send_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}
