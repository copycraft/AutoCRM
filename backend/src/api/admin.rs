//! Operational visibility: dead jobs, stuck email, FX coverage.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, page_limit};
use crate::AppState;
use crate::config::EmailTransportConfig;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::jobs::kinds;
use crate::repo::jobs::{self, Job};
use crate::repo::{config, emails, fx};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/status", get(status))
        .route("/admin/jobs", get(list_jobs))
        .route("/admin/jobs/{id}/retry", post(retry_job))
        .route("/admin/fx/fetch", post(fetch_fx))
        .route("/admin/run/{kind}", post(run_now))
}

/// Runs a periodic job now instead of waiting for the scheduler ("send the nudges now").
async fn run_now(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(kind): ApiPath<String>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
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
    Ok((StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))))
}

#[derive(Serialize)]
struct Status {
    environment: String,
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

async fn status(State(state): State<AppState>, Auth(me): Auth) -> AppResult<Json<Status>> {
    me.require(Capability::OperateSystem)?;
    let settings = config::settings(&state.db).await?;
    Ok(Json(Status {
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

#[derive(Deserialize)]
struct JobsQuery {
    /// "failed" (default) or "pending"
    state: Option<String>,
    limit: Option<i64>,
}

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

#[derive(Deserialize)]
struct FxFetch {
    from: NaiveDate,
    to: NaiveDate,
}

async fn fetch_fx(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<FxFetch>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
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
    Ok((StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))))
}
