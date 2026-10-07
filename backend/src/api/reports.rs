//! The reports. A handful of real endpoints, not a report builder; add more when asked.

use axum::Json;
use axum::extract::State;
use chrono::{DateTime, NaiveDate, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiQuery, Auth};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::repo::attribution::{self, CampaignRow, ChannelRow, PageRow};
use crate::repo::fx::{self, FxRate};
use crate::repo::lead_tags::{self, SiteFunnel};
use crate::repo::reports::{
    self, BlockerLoadRow, StageDurationRow, StalledOrder, ThroughputRow, VolumeRow,
};
use crate::service::business_today;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(volume))
        .routes(routes!(stage_durations))
        .routes(routes!(throughput))
        .routes(routes!(workload))
        .routes(routes!(stalled))
        .routes(routes!(blocker_load))
        .routes(routes!(fx_rates))
        .routes(routes!(lead_sources))
        .routes(routes!(website_conversion))
        // 0049
        .routes(routes!(sales_funnel))
        .routes(routes!(salespeople))
        .routes(routes!(first_response))
        .routes(routes!(revenue_by_country))
        .routes(routes!(cumulative_flow))
        .routes(routes!(newsletter_trends))
        .routes(routes!(get_weekly_recipients, put_weekly_recipients))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct Range {
    /// Defaults to 365 days before `to`.
    from: Option<NaiveDate>,
    /// Defaults to today.
    to: Option<NaiveDate>,
}

#[derive(Serialize, ToSchema)]
struct Period {
    from: NaiveDate,
    to: NaiveDate,
}

/// Default period: the last 12 months up to today (inclusive).
fn period(state: &AppState, from: Option<NaiveDate>, to: Option<NaiveDate>) -> AppResult<Period> {
    let to = to.unwrap_or_else(|| business_today(state.config.business_tz));
    let from = from.unwrap_or(to - TimeDelta::days(365));
    if from > to {
        return Err(AppError::validation("from must not be after to"));
    }
    Ok(Period { from, to })
}

/// Local-midnight bounds [from, to+1) in the business time zone, as UTC instants.
fn utc_bounds(tz: Tz, p: &Period) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = |d: NaiveDate| {
        tz.from_local_datetime(&d.and_hms_opt(0, 0, 0).expect("midnight"))
            .earliest()
            .expect("midnight exists")
            .with_timezone(&Utc)
    };
    (start(p.from), start(p.to + TimeDelta::days(1)))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct VolumeQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    /// month (default) | partner | project_type
    group: Option<String>,
    #[serde(default)]
    include_cancelled: bool,
}

#[derive(Serialize, ToSchema)]
struct VolumeReport {
    period: Period,
    group: String,
    rows: Vec<VolumeRow>,
    totals: VolumeRow,
}

#[utoipa::path(
    get, path = "/reports/volume", tag = "reports",
    params(VolumeQuery),
    responses((status = 200, body = VolumeReport))
)]
async fn volume(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<VolumeQuery>,
) -> AppResult<Json<VolumeReport>> {
    let p = period(&state, q.from, q.to)?;
    let group = q.group.unwrap_or_else(|| "month".into());
    let rows = match group.as_str() {
        "month" => reports::volume_by_month(&state.db, p.from, p.to, q.include_cancelled).await?,
        "partner" => {
            reports::volume_by_partner(&state.db, p.from, p.to, q.include_cancelled).await?
        }
        "project_type" => {
            reports::volume_by_project_type(&state.db, p.from, p.to, q.include_cancelled).await?
        }
        other => return Err(AppError::validation(format!("unknown group '{other}'"))),
    };
    let totals = rows.iter().fold(
        VolumeRow {
            key: "total".into(),
            label: "Összesen".into(),
            orders: 0,
            huf_minor: 0,
            eur_minor: 0,
            normalized_huf_minor: 0,
            missing_fx: 0,
        },
        |mut acc, r| {
            acc.orders += r.orders;
            acc.huf_minor += r.huf_minor;
            acc.eur_minor += r.eur_minor;
            acc.normalized_huf_minor += r.normalized_huf_minor;
            acc.missing_fx += r.missing_fx;
            acc
        },
    );
    Ok(Json(VolumeReport {
        period: p,
        group,
        rows,
        totals,
    }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct DurationQuery {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    project_type_id: Option<i64>,
}

#[derive(Serialize, ToSchema)]
struct DurationReport {
    period: Period,
    rows: Vec<StageDurationRow>,
}

#[utoipa::path(
    get, path = "/reports/stage-durations", tag = "reports",
    params(DurationQuery),
    responses((status = 200, body = DurationReport))
)]
async fn stage_durations(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<DurationQuery>,
) -> AppResult<Json<DurationReport>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    let rows = reports::stage_durations(&state.db, from, to, q.project_type_id).await?;
    Ok(Json(DurationReport { period: p, rows }))
}

#[derive(Serialize, ToSchema)]
struct ThroughputReport {
    period: Period,
    rows: Vec<ThroughputRow>,
}

#[utoipa::path(
    get, path = "/reports/throughput", tag = "reports",
    params(Range),
    responses((status = 200, body = ThroughputReport))
)]
async fn throughput(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<ThroughputReport>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    let rows = reports::throughput(&state.db, from, to, state.config.business_tz.name()).await?;
    Ok(Json(ThroughputReport { period: p, rows }))
}

#[utoipa::path(
    get, path = "/reports/stalled", tag = "reports",
    responses((status = 200, description = "Open orders past their stage's `stall_after_days`", body = Items<StalledOrder>))
)]
async fn stalled(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<StalledOrder>>> {
    Ok(Items::new(reports::stalled_orders(&state.db).await?))
}

#[derive(Serialize, ToSchema)]
struct BlockerLoadEntry {
    #[serde(flatten)]
    row: BlockerLoadRow,
    /// Fraction (0–1) of all blocker waiting time in the period.
    share_of_waiting: f64,
}

#[derive(Serialize, ToSchema)]
struct BlockerLoadReport {
    period: Period,
    total_waiting_days: f64,
    rows: Vec<BlockerLoadEntry>,
}

#[utoipa::path(
    get, path = "/reports/blocker-load", tag = "reports",
    params(Range),
    responses((status = 200, body = BlockerLoadReport))
)]
async fn blocker_load(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<BlockerLoadReport>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    let today = business_today(state.config.business_tz);
    let rows = reports::blocker_load(&state.db, from, to, today).await?;
    let total: f64 = rows.iter().map(|r| r.waiting_days).sum();
    let rows = rows
        .into_iter()
        .map(|row| BlockerLoadEntry {
            share_of_waiting: if total > 0.0 {
                row.waiting_days / total
            } else {
                0.0
            },
            row,
        })
        .collect();
    Ok(Json(BlockerLoadReport {
        period: p,
        total_waiting_days: total,
        rows,
    }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct FxQuery {
    /// Defaults to EUR.
    base: Option<String>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

#[utoipa::path(
    get, path = "/reports/fx-rates", tag = "reports",
    params(FxQuery),
    responses((status = 200, description = "Stored MNB rates", body = Items<FxRate>))
)]
async fn fx_rates(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<FxQuery>,
) -> AppResult<Json<Items<FxRate>>> {
    let p = period(&state, q.from, q.to)?;
    let base = q.base.unwrap_or_else(|| "EUR".into()).to_uppercase();
    Ok(Items::new(fx::list(&state.db, &base, p.from, p.to).await?))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct WorkloadQuery {
    /// Defaults to 6 days before `to` (one week).
    from: Option<NaiveDate>,
    /// Defaults to today.
    to: Option<NaiveDate>,
}

#[derive(Serialize, ToSchema)]
struct WorkloadDay {
    date: NaiveDate,
    /// Orders placed (created) that day.
    placed: i64,
    /// Orders that reached a terminal stage that day.
    completed: i64,
    /// Orders sitting in the workshop that day (placed, not yet completed).
    in_workshop: i64,
}

#[derive(Serialize, ToSchema)]
struct WorkloadReport {
    from: NaiveDate,
    to: NaiveDate,
    days: Vec<WorkloadDay>,
}

/// Daily workshop load: placed, completed and in-workshop cars per business-tz day.
/// Ranges cap at 62 days — this feeds bar charts, not history exports.
#[utoipa::path(
    get, path = "/reports/workload", tag = "reports",
    params(WorkloadQuery),
    responses((status = 200, body = WorkloadReport))
)]
async fn workload(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<WorkloadQuery>,
) -> AppResult<Json<WorkloadReport>> {
    let to =
        q.to.unwrap_or_else(|| business_today(state.config.business_tz));
    let from = q.from.unwrap_or(to - TimeDelta::days(6));
    if from > to {
        return Err(AppError::validation("from must not be after to"));
    }
    if workload_span_days(from, to) > 62 {
        return Err(AppError::validation("workload range caps at 62 days"));
    }
    let intervals =
        reports::workload_intervals(&state.db, from, to, state.config.business_tz.name()).await?;
    let mut days = Vec::new();
    let mut day = from;
    while day <= to {
        let placed = intervals.iter().filter(|r| r.placed == day).count() as i64;
        let completed = intervals
            .iter()
            .filter(|r| r.completed == Some(day))
            .count() as i64;
        let in_workshop = intervals
            .iter()
            .filter(|r| r.placed <= day && r.completed.map(|c| c >= day).unwrap_or(true))
            .count() as i64;
        days.push(WorkloadDay {
            date: day,
            placed,
            completed,
            in_workshop,
        });
        day += TimeDelta::days(1);
    }
    Ok(Json(WorkloadReport { from, to, days }))
}

/// The inclusive calendar-day span of a workload window. The cap is on days the chart
/// draws (REP-L2): `from` = Jan 1, `to` = Mar 4 is 63 days, not 62.
fn workload_span_days(from: NaiveDate, to: NaiveDate) -> i64 {
    (to - from).num_days() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_workload_cap_counts_calendar_days() {
        let jan1 = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        assert_eq!(workload_span_days(jan1, jan1), 1);
        assert_eq!(
            workload_span_days(jan1, NaiveDate::from_ymd_opt(2026, 3, 3).unwrap()),
            62
        );
        assert_eq!(
            workload_span_days(jan1, NaiveDate::from_ymd_opt(2026, 3, 4).unwrap()),
            63
        );
    }
}

#[derive(Serialize, ToSchema)]
struct LeadSources {
    period: Period,
    /// Website leads in the period (those carrying attribution).
    total: i64,
    /// Leads now in the won stage.
    won: i64,
    by_channel: Vec<ChannelRow>,
    /// Tagged traffic only (a UTM source or campaign was sent).
    by_campaign: Vec<CampaignRow>,
    /// The most common first pages of a visit.
    by_page: Vec<PageRow>,
}

/// Where website leads come from, and how many of them were won. Leads entered by hand have
/// no source and are not counted.
#[utoipa::path(
    get, path = "/reports/lead-sources", tag = "reports",
    params(Range),
    responses((status = 200, body = LeadSources))
)]
async fn lead_sources(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<LeadSources>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    let by_channel = attribution::by_channel(&state.db, from, to).await?;
    let by_campaign = attribution::by_campaign(&state.db, from, to).await?;
    let by_page = attribution::by_page(&state.db, from, to).await?;
    Ok(Json(LeadSources {
        total: by_channel.iter().map(|r| r.leads).sum(),
        won: by_channel.iter().map(|r| r.won).sum(),
        period: p,
        by_channel,
        by_campaign,
        by_page,
    }))
}

#[derive(Serialize, ToSchema)]
struct WebsiteConversion {
    period: Period,
    rows: Vec<SiteFunnel>,
    /// Why the period's lost leads were lost.
    lost_reasons: Vec<crate::repo::lost_reasons::ReasonCount>,
}

/// Leads → quotes → orders per source website (the domain the lead was tagged from).
#[utoipa::path(
    get, path = "/reports/website-conversion", tag = "reports",
    params(Range),
    responses((status = 200, body = WebsiteConversion))
)]
async fn website_conversion(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<WebsiteConversion>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    Ok(Json(WebsiteConversion {
        rows: lead_tags::by_site(&state.db, from, to).await?,
        lost_reasons: crate::repo::lost_reasons::breakdown(&state.db, from, to).await?,
        period: p,
    }))
}

// --- Sales and marketing reports (0049) ---

use crate::repo::sales_reports::{
    self, FlowPoint, FunnelStage, FunnelTotals, ResponseRow, RevenueRow, SalespersonRow, SendTrend,
    SubscriberMonth,
};

#[derive(Serialize, ToSchema)]
struct SalesFunnel {
    period: Period,
    totals: FunnelTotals,
    /// Progress stages in order, then the exit (lost) stages.
    stages: Vec<FunnelStage>,
}

/// The period's new leads, and how far they got.
#[utoipa::path(
    get, path = "/reports/sales-funnel", tag = "reports",
    params(Range),
    responses((status = 200, body = SalesFunnel))
)]
async fn sales_funnel(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<SalesFunnel>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    Ok(Json(SalesFunnel {
        totals: sales_reports::funnel_totals(&state.db, from, to).await?,
        stages: sales_reports::funnel(&state.db, from, to).await?,
        period: p,
    }))
}

#[derive(Serialize, ToSchema)]
struct Salespeople {
    period: Period,
    rows: Vec<SalespersonRow>,
}

/// Each salesperson's leads from the period: quoted, won, lost, value, response time.
#[utoipa::path(
    get, path = "/reports/salespeople", tag = "reports",
    params(Range),
    responses((status = 200, body = Salespeople))
)]
async fn salespeople(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<Salespeople>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    Ok(Json(Salespeople {
        rows: sales_reports::salespeople(&state.db, from, to).await?,
        period: p,
    }))
}

#[derive(Serialize, ToSchema)]
struct FirstResponse {
    period: Period,
    /// The row with source `*` is all leads together.
    rows: Vec<ResponseRow>,
}

/// How long new leads waited for a first human answer (a hand-written email or a stage
/// moved by a person), by source.
#[utoipa::path(
    get, path = "/reports/first-response", tag = "reports",
    params(Range),
    responses((status = 200, body = FirstResponse))
)]
async fn first_response(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<FirstResponse>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    Ok(Json(FirstResponse {
        rows: sales_reports::first_response(&state.db, from, to).await?,
        period: p,
    }))
}

#[derive(Serialize, ToSchema)]
struct RevenueByCountry {
    period: Period,
    rows: Vec<RevenueRow>,
}

/// Order value per customer country and year (the export markets next to Hungary).
#[utoipa::path(
    get, path = "/reports/revenue-by-country", tag = "reports",
    params(Range),
    responses((status = 200, body = RevenueByCountry))
)]
async fn revenue_by_country(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<RevenueByCountry>> {
    let p = period(&state, q.from, q.to)?;
    Ok(Json(RevenueByCountry {
        rows: sales_reports::revenue_by_country(&state.db, p.from, p.to).await?,
        period: p,
    }))
}

#[derive(Serialize, ToSchema)]
struct CumulativeFlow {
    period: Period,
    /// One point per week-end and stage.
    points: Vec<FlowPoint>,
}

/// How many orders sat in each stage at the end of each week: widening bands are
/// bottlenecks.
#[utoipa::path(
    get, path = "/reports/cumulative-flow", tag = "reports",
    params(Range),
    responses((status = 200, body = CumulativeFlow))
)]
async fn cumulative_flow(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<CumulativeFlow>> {
    let p = period(&state, q.from, q.to)?;
    if (p.to - p.from).num_days() > 3 * 366 {
        return Err(AppError::validation("at most three years at once"));
    }
    Ok(Json(CumulativeFlow {
        points: sales_reports::cumulative_flow(&state.db, p.from, p.to).await?,
        period: p,
    }))
}

#[derive(Serialize, ToSchema)]
struct NewsletterTrends {
    period: Period,
    months: Vec<SubscriberMonth>,
    sends: Vec<SendTrend>,
}

/// Subscribers joining and leaving by month, and each tracked send's reach.
#[utoipa::path(
    get, path = "/reports/newsletter-trends", tag = "reports",
    params(Range),
    responses((status = 200, body = NewsletterTrends))
)]
async fn newsletter_trends(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<Range>,
) -> AppResult<Json<NewsletterTrends>> {
    let p = period(&state, q.from, q.to)?;
    let (from, to) = utc_bounds(state.config.business_tz, &p);
    Ok(Json(NewsletterTrends {
        months: sales_reports::subscriber_months(&state.db, p.from, p.to).await?,
        sends: sales_reports::send_trends(&state.db, from, to).await?,
        period: p,
    }))
}

#[derive(Serialize, Deserialize, ToSchema)]
struct WeeklyRecipients {
    /// Who gets the Monday report; empty turns it off.
    recipients: Vec<String>,
}

#[utoipa::path(
    get, path = "/reports/weekly/recipients", tag = "reports",
    responses((status = 200, body = WeeklyRecipients))
)]
async fn get_weekly_recipients(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<WeeklyRecipients>> {
    me.require(crate::domain::role::Capability::ManageSettings)?;
    Ok(Json(WeeklyRecipients {
        recipients: crate::repo::config::weekly_report_recipients(&state.db).await?,
    }))
}

#[utoipa::path(
    put, path = "/reports/weekly/recipients", tag = "reports",
    request_body = WeeklyRecipients,
    responses((status = 200, body = WeeklyRecipients))
)]
async fn put_weekly_recipients(
    State(state): State<AppState>,
    Auth(me): Auth,
    super::extract::ApiJson(b): super::extract::ApiJson<WeeklyRecipients>,
) -> AppResult<Json<WeeklyRecipients>> {
    me.require(crate::domain::role::Capability::ManageSettings)?;
    let mut recipients = Vec::new();
    for r in b
        .recipients
        .iter()
        .map(|r| r.trim())
        .filter(|r| !r.is_empty())
    {
        let address = crate::domain::email::normalize_address(r)
            .ok_or_else(|| AppError::validation(format!("'{r}' is not a valid address")))?;
        if !recipients.contains(&address) {
            recipients.push(address);
        }
    }
    if recipients.len() > 20 {
        return Err(AppError::validation("at most 20 recipients"));
    }
    crate::repo::config::set_weekly_report_recipients(&state.db, &recipients).await?;
    Ok(Json(WeeklyRecipients { recipients }))
}
