//! Job ads (0064): a job listing run as ads on Facebook, Instagram, Google…, each with its
//! own tracked link, and our own numbers on which ad brings visits and applicants. Nothing
//! is sent to the ad platforms. HR work: `AccessHr`, like the listings themselves.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth};
use crate::AppState;
use crate::domain::attribution;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::job_ads::{self, ChannelStats, JobAd, JobAdInput};
use crate::repo::{audit, recruitment};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_ads, create_ad))
        .routes(routes!(update_ad))
        .routes(routes!(record_visit))
}

const PLATFORMS: [&str; 5] = ["facebook", "instagram", "google", "tiktok", "other"];

/// The UTM medium the ad's link carries, so the channel report sorts it as paid.
fn medium_of(platform: &str) -> &'static str {
    match platform {
        "google" => "cpc",
        "facebook" | "instagram" | "tiktok" => "paid_social",
        _ => "paid",
    }
}

/// Lower-case ASCII, words joined by `-`: a campaign name that survives any URL.
fn campaign_of(name: &str) -> String {
    // Letter by letter: the search fold drops spaces, and the words must stay apart.
    let folded: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                crate::domain::search_terms::fold(&c.to_string())
            } else {
                "-".to_string()
            }
        })
        .collect::<String>()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    folded
        .split('-')
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// The link to paste into the ad: the listing's page, tagged with the platform, the
/// campaign and the ad itself.
fn tracked_link(base: &str, slug: &str, ad: &JobAd) -> String {
    format!(
        "{}/hu/jobs/{slug}?utm_source={}&utm_medium={}&utm_campaign={}&ad={}",
        base.trim_end_matches('/'),
        ad.platform,
        medium_of(&ad.platform),
        campaign_of(&ad.name),
        ad.id
    )
}

/// An ad with its link and what an applicant cost through it.
#[derive(Serialize, ToSchema)]
pub struct JobAdView {
    #[serde(flatten)]
    pub ad: JobAd,
    /// Paste this into the ad: visits and applications through it are counted for the ad.
    pub link: String,
    /// Spend divided by applications, in HUF fillér; null without spend or applications.
    pub cost_per_application_minor: Option<i64>,
}

fn view(base: &str, slug: &str, ad: JobAd) -> JobAdView {
    let cost = match (ad.spend_minor, ad.applications) {
        (Some(spend), n) if n > 0 => Some(spend / n),
        _ => None,
    };
    JobAdView {
        link: tracked_link(base, slug, &ad),
        cost_per_application_minor: cost,
        ad,
    }
}

#[derive(Serialize, ToSchema)]
pub struct JobAdsReport {
    pub ads: Vec<JobAdView>,
    /// Visits and applications that came through no ad of ours, by channel.
    pub other: Vec<ChannelStats>,
}

#[utoipa::path(
    get, path = "/hr/jobs/{id}/ads", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = JobAdsReport))
)]
async fn list_ads(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<JobAdsReport>> {
    me.require(Capability::AccessHr)?;
    let posting = recruitment::find_posting(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let base = &state.config.public_base_url;
    let ads = job_ads::list(&state.db, id)
        .await?
        .into_iter()
        .map(|a| view(base, &posting.slug, a))
        .collect();
    Ok(Json(JobAdsReport {
        ads,
        other: job_ads::other_channels(&state.db, id).await?,
    }))
}

#[derive(Deserialize, ToSchema)]
struct JobAdBody {
    platform: String,
    name: String,
    headline: Option<String>,
    body: Option<String>,
    /// What it cost, in HUF fillér.
    spend_minor: Option<i64>,
    starts_on: Option<NaiveDate>,
    ends_on: Option<NaiveDate>,
    /// PATCH only: archive (true) or bring back (false).
    archived: Option<bool>,
}

fn input(b: &JobAdBody) -> AppResult<JobAdInput> {
    if !PLATFORMS.contains(&b.platform.as_str()) {
        return Err(AppError::validation(format!(
            "platform must be one of {}",
            PLATFORMS.join(", ")
        )));
    }
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 200 {
        return Err(AppError::validation(
            "name is required (at most 200 characters)",
        ));
    }
    if b.spend_minor.is_some_and(|s| s < 0) {
        return Err(AppError::validation("spend_minor must not be negative"));
    }
    if let (Some(from), Some(to)) = (b.starts_on, b.ends_on)
        && to < from
    {
        return Err(AppError::validation("ends_on is before starts_on"));
    }
    let text = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
    };
    Ok(JobAdInput {
        platform: b.platform.clone(),
        name: name.to_string(),
        headline: text(&b.headline),
        body: text(&b.body),
        spend_minor: b.spend_minor,
        starts_on: b.starts_on,
        ends_on: b.ends_on,
    })
}

#[utoipa::path(
    post, path = "/hr/jobs/{id}/ads", tag = "hr",
    params(("id" = i64, Path)),
    request_body = JobAdBody,
    responses((status = 201, body = JobAdView))
)]
async fn create_ad(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<JobAdBody>,
) -> AppResult<(StatusCode, Json<JobAdView>)> {
    me.require(Capability::AccessHr)?;
    let posting = recruitment::find_posting(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let input = input(&b)?;
    let mut tx = state.db.begin().await?;
    let ad_id = job_ads::insert(&mut *tx, id, &input, me.user_id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_posting",
        id,
        "job_ad_add",
        json!({ "ad_id": ad_id, "platform": input.platform, "name": input.name }),
    )
    .await?;
    tx.commit().await?;
    let ad = job_ads::find(&state.db, ad_id)
        .await?
        .ok_or(AppError::NotFound("job ad"))?;
    Ok((
        StatusCode::CREATED,
        Json(view(&state.config.public_base_url, &posting.slug, ad)),
    ))
}

#[utoipa::path(
    patch, path = "/hr/job-ads/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = JobAdBody,
    responses((status = 200, body = JobAdView))
)]
async fn update_ad(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<JobAdBody>,
) -> AppResult<Json<JobAdView>> {
    me.require(Capability::AccessHr)?;
    let current = job_ads::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job ad"))?;
    let posting = recruitment::find_posting(&state.db, current.posting_id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let input = input(&b)?;
    job_ads::update(&state.db, id, &input).await?;
    if let Some(archived) = b.archived {
        job_ads::set_archived(&state.db, id, archived).await?;
    }
    let ad = job_ads::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job ad"))?;
    Ok(Json(view(&state.config.public_base_url, &posting.slug, ad)))
}

/// Where a visitor of the listing's page came from, as the page read it off its own URL.
#[derive(Deserialize, ToSchema, Default)]
pub struct VisitSource {
    /// The `ad` parameter of a tracked link.
    pub ad: Option<i64>,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub referrer: Option<String>,
}

impl VisitSource {
    fn cap(v: &Option<String>, max: usize) -> Option<String> {
        v.as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| t.chars().take(max).collect())
    }

    /// The source checked against the listing: an ad id from another listing is no ad.
    pub async fn resolve(
        &self,
        db: &sqlx::PgPool,
        posting_id: i64,
    ) -> AppResult<recruitment::ApplicationSource> {
        let ad_id = match self.ad {
            Some(id) if job_ads::belongs(db, id, posting_id).await? => Some(id),
            _ => None,
        };
        let utm_source = Self::cap(&self.utm_source, 200);
        let utm_medium = Self::cap(&self.utm_medium, 200);
        let referrer = Self::cap(&self.referrer, 500);
        let channel = attribution::classify(
            utm_source.as_deref(),
            utm_medium.as_deref(),
            referrer.as_deref(),
        );
        Ok(recruitment::ApplicationSource {
            ad_id,
            channel: Some(channel.as_str().to_string()),
            utm_source,
            utm_medium,
            utm_campaign: Self::cap(&self.utm_campaign, 200),
            referrer,
        })
    }
}

/// Count a visit to a listing's page: a number per day, by ad and channel. Nothing about
/// the visitor is kept.
#[utoipa::path(
    post, path = "/public/jobs/{slug}/visit", tag = "recruitment",
    params(("slug" = String, Path)),
    security(()),
    request_body = VisitSource,
    responses((status = 204), (status = 404))
)]
async fn record_visit(
    State(state): State<AppState>,
    ApiPath(slug): ApiPath<String>,
    ApiJson(b): ApiJson<VisitSource>,
) -> AppResult<StatusCode> {
    let posting = recruitment::find_posting_by_slug(&state.db, &slug)
        .await?
        .filter(|p| p.status == "published")
        .ok_or(AppError::NotFound("job"))?;
    let source = b.resolve(&state.db, posting.id).await?;
    job_ads::record_visit(
        &state.db,
        posting.id,
        source.ad_id,
        source.channel.as_deref().unwrap_or("direct"),
        crate::service::business_today(state.config.business_tz),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_campaign_name_survives_a_url() {
        assert_eq!(
            campaign_of("Villanyszerelő – Szeged, 2026 ősz!"),
            "villanyszerelo-szeged-2026-osz"
        );
        assert_eq!(medium_of("google"), "cpc");
        assert_eq!(medium_of("instagram"), "paid_social");
    }
}
