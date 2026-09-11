//! Operational visibility: dead jobs, stuck email, FX coverage.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, page_limit};
use crate::AppState;
use crate::config::EmailTransportConfig;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::jobs::kinds;
use crate::repo::jobs::{self, Job};
use crate::repo::{config, emails, fx};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(list_jobs))
        .routes(routes!(retry_job))
        .routes(routes!(fetch_fx))
        .routes(routes!(run_now))
}

#[derive(Serialize, ToSchema)]
struct JobQueued {
    /// Null when an identical job was already queued.
    job_id: Option<i64>,
}

/// Runs a periodic job now instead of waiting for the scheduler ("send the nudges now").
#[utoipa::path(
    post, path = "/admin/run/{kind}", tag = "admin",
    params(("kind" = String, Path, description = "`nudge_blockers` or `stalled_orders`")),
    responses((status = 202, body = JobQueued))
)]
async fn run_now(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(kind): ApiPath<String>,
) -> AppResult<(StatusCode, Json<JobQueued>)> {
    me.require(Capability::OperateSystem)?;
    let kind = match kind.as_str() {
        kinds::NUDGE_BLOCKERS => kinds::NUDGE_BLOCKERS,
        kinds::STALLED_ORDERS => kinds::STALLED_ORDERS,
        other => {
            return Err(AppError::validation(format!(
                "'{other}' cannot be run manually"
            )));
        }
    };
    let key = format!("{kind}:manual:{}", chrono::Utc::now().timestamp());
    let job_id = jobs::enqueue(&state.db, kind, json!({}), None, Some(&key)).await?;
    tracing::info!(
        kind,
        user_id = me.user_id,
        "periodic job triggered manually"
    );
    Ok((StatusCode::ACCEPTED, Json(JobQueued { job_id })))
}

#[derive(Serialize, ToSchema)]
struct AdminStatus {
    environment: String,
    /// `dry_run` or `smtp`.
    email_mode: &'static str,
    smtp_host: Option<String>,
    /// Set during staging or the parallel run: all mail goes to this one address.
    email_redirect_to: Option<String>,
    automatic_email_enabled: bool,
    failed_jobs: i64,
    pending_jobs: i64,
    emails_needing_attention: i64,
    orders_missing_fx_rate: i64,
    latest_eur_rate_day: Option<NaiveDate>,
}

#[utoipa::path(
    get, path = "/admin/status", tag = "admin",
    responses((status = 200, body = AdminStatus))
)]
async fn status(State(state): State<AppState>, Auth(me): Auth) -> AppResult<Json<AdminStatus>> {
    me.require(Capability::OperateSystem)?;
    let settings = config::settings(&state.db).await?;
    Ok(Json(AdminStatus {
        environment: format!("{:?}", state.config.env).to_lowercase(),
        email_mode: match state.config.email.transport {
            EmailTransportConfig::DryRun => "dry_run",
            EmailTransportConfig::Smtp(_) => "smtp",
        },
        smtp_host: match &state.config.email.transport {
            EmailTransportConfig::Smtp(smtp) => Some(format!("{}:{}", smtp.host, smtp.port)),
            EmailTransportConfig::DryRun => None,
        },
        email_redirect_to: state.config.email.redirect_to.clone(),
        automatic_email_enabled: settings.automatic_email_enabled,
        failed_jobs: jobs::count_failed(&state.db).await?,
        pending_jobs: jobs::count_pending(&state.db).await?,
        emails_needing_attention: emails::count_needing_attention(&state.db).await?,
        orders_missing_fx_rate: fx::orders_missing_rates(&state.db).await?,
        latest_eur_rate_day: fx::latest_day(&state.db, "EUR").await?,
    }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct JobsQuery {
    /// "failed" (default) or "pending"
    state: Option<String>,
    limit: Option<i64>,
}

#[utoipa::path(
    get, path = "/admin/jobs", tag = "admin",
    params(JobsQuery),
    responses((status = 200, body = Items<Job>))
)]
async fn list_jobs(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<JobsQuery>,
) -> AppResult<Json<Items<Job>>> {
    me.require(Capability::OperateSystem)?;
    let limit = page_limit(q.limit);
    let rows = match q.state.as_deref() {
        None | Some("failed") => jobs::list_failed(&state.db, limit).await?,
        Some("pending") => jobs::list_pending(&state.db, limit).await?,
        Some(other) => return Err(AppError::validation(format!("unknown job state '{other}'"))),
    };
    Ok(Items::new(rows))
}

#[utoipa::path(
    post, path = "/admin/jobs/{id}/retry", tag = "admin",
    params(("id" = i64, Path)),
    responses((status = 202, description = "Re-queued"))
)]
async fn retry_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::OperateSystem)?;
    if !jobs::retry_failed(&state.db, id).await? {
        return Err(AppError::conflict(
            "not_failed",
            "only dead-lettered jobs can be retried",
        ));
    }
    tracing::info!(job_id = id, user_id = me.user_id, "job manually retried");
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize, ToSchema)]
struct FxFetch {
    from: NaiveDate,
    to: NaiveDate,
}

#[utoipa::path(
    post, path = "/admin/fx/fetch", tag = "admin",
    request_body = FxFetch,
    responses((status = 202, body = JobQueued))
)]
async fn fetch_fx(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<FxFetch>,
) -> AppResult<(StatusCode, Json<JobQueued>)> {
    me.require(Capability::OperateSystem)?;
    if b.from > b.to {
        return Err(AppError::validation("from must not be after to"));
    }
    let key = format!("{}:manual:{}:{}", kinds::FETCH_FX_RATES, b.from, b.to);
    let job_id = jobs::enqueue(
        &state.db,
        kinds::FETCH_FX_RATES,
        json!({ "from": b.from, "to": b.to }),
        None,
        Some(&key),
    )
    .await?;
    Ok((StatusCode::ACCEPTED, Json(JobQueued { job_id })))
}
