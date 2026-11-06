//! Job ads and the visits they bring (0064): our own numbers on which ad brings applicants.
//! Runtime-checked queries, like the rest of the HR module.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct JobAd {
    pub id: i64,
    pub posting_id: i64,
    /// `facebook`, `instagram`, `google`, `tiktok` or `other`.
    pub platform: String,
    pub name: String,
    pub headline: Option<String>,
    pub body: Option<String>,
    /// What it cost, in HUF fillér.
    pub spend_minor: Option<i64>,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    /// Visits to the listing's page through this ad.
    pub visits: i64,
    /// Applications that came through this ad.
    pub applications: i64,
}

/// Visits and applications that came through no ad of ours, by channel.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ChannelStats {
    /// `paid`, `organic`, `social`, `email`, `referral` or `direct`.
    pub channel: String,
    pub visits: i64,
    pub applications: i64,
}

#[derive(Debug, Clone, Default)]
pub struct JobAdInput {
    pub platform: String,
    pub name: String,
    pub headline: Option<String>,
    pub body: Option<String>,
    pub spend_minor: Option<i64>,
    pub starts_on: Option<NaiveDate>,
    pub ends_on: Option<NaiveDate>,
}

const SELECT: &str = "SELECT a.id, a.posting_id, a.platform, a.name, a.headline, a.body,
        a.spend_minor, a.starts_on, a.ends_on, a.created_at, a.archived_at,
        COALESCE((SELECT sum(v.visits) FROM job_visits v WHERE v.ad_id = a.id), 0)::bigint AS visits,
        (SELECT count(*) FROM job_applications j WHERE j.ad_id = a.id) AS applications
     FROM job_ads a";

/// A listing's ads, newest first; archived ones last.
pub async fn list(db: impl PgExecutor<'_>, posting_id: i64) -> sqlx::Result<Vec<JobAd>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE a.posting_id = $1
         ORDER BY a.archived_at IS NOT NULL, a.created_at DESC, a.id DESC"
    ))
    .bind(posting_id)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<JobAd>> {
    sqlx::query_as(&format!("{SELECT} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    posting_id: i64,
    a: &JobAdInput,
    user_id: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO job_ads (posting_id, platform, name, headline, body, spend_minor,
                              starts_on, ends_on, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(posting_id)
    .bind(&a.platform)
    .bind(&a.name)
    .bind(&a.headline)
    .bind(&a.body)
    .bind(a.spend_minor)
    .bind(a.starts_on)
    .bind(a.ends_on)
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn update(db: impl PgExecutor<'_>, id: i64, a: &JobAdInput) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE job_ads SET platform = $2, name = $3, headline = $4, body = $5, spend_minor = $6,
                            starts_on = $7, ends_on = $8
          WHERE id = $1",
    )
    .bind(id)
    .bind(&a.platform)
    .bind(&a.name)
    .bind(&a.headline)
    .bind(&a.body)
    .bind(a.spend_minor)
    .bind(a.starts_on)
    .bind(a.ends_on)
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// Archived, not deleted: the applications it brought keep saying where they came from.
pub async fn set_archived(db: impl PgExecutor<'_>, id: i64, archived: bool) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE job_ads SET archived_at = CASE WHEN $2 THEN now() ELSE NULL END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(db)
    .await?;
    Ok(done.rows_affected() > 0)
}

/// Whether the ad belongs to the listing: a link from another listing's ad is not this one's.
pub async fn belongs(db: impl PgExecutor<'_>, ad_id: i64, posting_id: i64) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM job_ads WHERE id = $1 AND posting_id = $2)")
        .bind(ad_id)
        .bind(posting_id)
        .fetch_one(db)
        .await
}

/// Counts one visit to the listing's page today.
pub async fn record_visit(
    db: impl PgExecutor<'_>,
    posting_id: i64,
    ad_id: Option<i64>,
    channel: &str,
    day: NaiveDate,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO job_visits (posting_id, ad_id, channel, day, visits) VALUES ($1, $2, $3, $4, 1)
         ON CONFLICT (posting_id, COALESCE(ad_id, 0), channel, day)
         DO UPDATE SET visits = job_visits.visits + 1",
    )
    .bind(posting_id)
    .bind(ad_id)
    .bind(channel)
    .bind(day)
    .execute(db)
    .await?;
    Ok(())
}

/// Visits and applications that came through no ad of ours, per channel.
pub async fn other_channels(
    db: impl PgExecutor<'_>,
    posting_id: i64,
) -> sqlx::Result<Vec<ChannelStats>> {
    sqlx::query_as(
        "SELECT channel, sum(visits)::bigint AS visits, sum(applications)::bigint AS applications
           FROM (
               SELECT channel, visits, 0 AS applications FROM job_visits
                WHERE posting_id = $1 AND ad_id IS NULL
               UNION ALL
               SELECT COALESCE(channel, 'direct'), 0, 1 FROM job_applications
                WHERE posting_id = $1 AND ad_id IS NULL
           ) t
          GROUP BY channel
          ORDER BY sum(applications) DESC, sum(visits) DESC, channel",
    )
    .bind(posting_id)
    .fetch_all(db)
    .await
}
