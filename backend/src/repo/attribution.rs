//! Where website leads came from. Runtime-checked queries, like the other HR/website tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// What the website told us about one lead, plus the channel the server worked out.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Attribution {
    /// `paid` | `organic` | `social` | `email` | `referral` | `direct`.
    pub channel: String,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub referrer: Option<String>,
    pub landing_page: Option<String>,
}

pub async fn insert(db: impl PgExecutor<'_>, lead_id: i64, a: &Attribution) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO lead_attribution
             (lead_id, channel, utm_source, utm_medium, utm_campaign, referrer, landing_page)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(lead_id)
    .bind(&a.channel)
    .bind(&a.utm_source)
    .bind(&a.utm_medium)
    .bind(&a.utm_campaign)
    .bind(&a.referrer)
    .bind(&a.landing_page)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn find(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Option<Attribution>> {
    sqlx::query_as(
        "SELECT channel, utm_source, utm_medium, utm_campaign, referrer, landing_page
         FROM lead_attribution WHERE lead_id = $1",
    )
    .bind(lead_id)
    .fetch_optional(db)
    .await
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ChannelRow {
    pub channel: String,
    pub leads: i64,
    /// Leads now in the `won` stage.
    pub won: i64,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct CampaignRow {
    pub channel: String,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub leads: i64,
    pub won: i64,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct PageRow {
    pub landing_page: String,
    pub leads: i64,
    pub won: i64,
}

const FROM: &str = "FROM lead_attribution a
    JOIN leads l ON l.id = a.lead_id
    JOIN lead_current_stage cs ON cs.lead_id = l.id
    WHERE l.created_at >= $1 AND l.created_at < $2";

pub async fn by_channel(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<ChannelRow>> {
    sqlx::query_as(&format!(
        "SELECT a.channel, count(*) AS leads, count(*) FILTER (WHERE cs.stage_key = 'won') AS won
         {FROM} GROUP BY a.channel ORDER BY leads DESC, a.channel"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

/// Only leads that carried a UTM source or campaign: untagged traffic has no campaign.
pub async fn by_campaign(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<CampaignRow>> {
    sqlx::query_as(&format!(
        "SELECT a.channel, a.utm_source, a.utm_medium, a.utm_campaign,
                count(*) AS leads, count(*) FILTER (WHERE cs.stage_key = 'won') AS won
         {FROM} AND (a.utm_source IS NOT NULL OR a.utm_campaign IS NOT NULL)
         GROUP BY a.channel, a.utm_source, a.utm_medium, a.utm_campaign
         ORDER BY leads DESC, a.utm_campaign, a.utm_source LIMIT 50"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

pub async fn by_page(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<PageRow>> {
    sqlx::query_as(&format!(
        "SELECT a.landing_page, count(*) AS leads,
                count(*) FILTER (WHERE cs.stage_key = 'won') AS won
         {FROM} AND a.landing_page IS NOT NULL
         GROUP BY a.landing_page ORDER BY leads DESC, a.landing_page LIMIT 20"
    ))
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}
