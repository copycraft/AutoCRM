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

use super::configuration::{MergedTransport, validate_transport};
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, page_limit};
use crate::AppState;
use crate::config::EmailTransportConfig;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::domain::template::text_to_html;
use crate::error::{AppError, AppResult};
use crate::integrations::email::{Mailer, OutgoingEmail};
use crate::jobs::kinds;
use crate::repo::jobs::{self, Job};
use crate::repo::{config, emails, fx};
use crate::service::email::{self, format_from};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(list_jobs))
        .routes(routes!(retry_job))
        .routes(routes!(fetch_fx))
        .routes(routes!(run_now))
        .routes(routes!(test_email))
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
    // Effective transport (admin settings row, else environment): showing the
    // startup env values while a database override is live would mislead.
    let (email, _) = email::effective_email_config(&state.db, &state.config).await;
    Ok(Json(AdminStatus {
        environment: format!("{:?}", state.config.env).to_lowercase(),
        email_mode: match email.transport {
            EmailTransportConfig::DryRun => "dry_run",
            EmailTransportConfig::Smtp(_) => "smtp",
        },
        smtp_host: match &email.transport {
            EmailTransportConfig::Smtp(smtp) => Some(format!("{}:{}", smtp.host, smtp.port)),
            EmailTransportConfig::DryRun => None,
        },
        email_redirect_to: email.redirect_to.clone(),
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

/// A candidate transport for the test endpoint. Same validation as saving;
/// `smtp_password` absent falls back to the saved secret so hosts can be
/// retried without retyping it. Nothing here is persisted.
#[derive(Deserialize, ToSchema)]
struct EmailTestCandidate {
    mode: String,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_security: Option<String>,
    smtp_username: Option<String>,
    smtp_password: Option<String>,
    smtp_helo_name: Option<String>,
    smtp_force_ipv4: Option<bool>,
    redirect_to: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct EmailTestBody {
    to: String,
    #[serde(default)]
    config: Option<EmailTestCandidate>,
}

#[derive(Serialize, ToSchema)]
struct EmailTestResult {
    /// False means the mail did not go out; `detail` says why.
    ok: bool,
    detail: String,
}

/// Send one test mail through the saved transport, or through an unsaved
/// candidate (validated first, never persisted). Always 200: delivery failure
/// is a diagnostic result, not a request error. Bypasses the queue like the
/// `email-test` CLI; in dry_run mode nothing leaves the machine.
#[utoipa::path(
    post, path = "/admin/email/test", tag = "admin",
    request_body = EmailTestBody,
    responses((status = 200, body = EmailTestResult))
)]
async fn test_email(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<EmailTestBody>,
) -> AppResult<Json<EmailTestResult>> {
    me.require(Capability::OperateSystem)?;
    let to = normalize_address(&b.to)
        .ok_or_else(|| AppError::validation("to is not a valid email address"))?;
    let env = &state.config.email;
    let (transport, redirect_to) = match b.config {
        Some(c) => {
            let merged = MergedTransport {
                mode: Some(c.mode.trim().to_string()),
                host: c.smtp_host,
                port: c.smtp_port,
                security: c.smtp_security,
                username: c.smtp_username,
                helo_name: c.smtp_helo_name,
                force_ipv4: c.smtp_force_ipv4,
                redirect_to: c.redirect_to,
            };
            validate_transport(&merged, state.config.env)?;
            let password = match c.smtp_password.filter(|p| !p.trim().is_empty()) {
                Some(p) => Some(p),
                None => config::email_secret(&state.db)
                    .await?
                    .and_then(|stored| crate::service::secrets::open(&state.config, &stored)),
            };
            let transport = email::transport_from_parts(
                merged.mode.as_deref(),
                merged.host,
                merged.port,
                merged.security.as_deref(),
                merged.username,
                password,
                merged.helo_name,
                merged.force_ipv4,
                &env.message_id_domain,
            )
            .ok_or_else(|| AppError::validation("candidate transport is incomplete"))?;
            (transport, merged.redirect_to)
        }
        None => {
            let (effective, _) = email::effective_email_config(&state.db, &state.config).await;
            (effective.transport, effective.redirect_to)
        }
    };
    let candidate = crate::config::EmailConfig {
        transport,
        redirect_to,
        ..env.clone()
    };
    let mailer = match Mailer::from_config(&candidate) {
        Ok(m) => m,
        Err(e) => {
            return Ok(Json(EmailTestResult {
                ok: false,
                detail: format!("transport misconfigured: {e}"),
            }));
        }
    };
    if let Err(e) = mailer.test_connection().await {
        return Ok(Json(EmailTestResult {
            ok: false,
            detail: e,
        }));
    }
    let random: [u8; 6] = rand::random();
    let body = "Ez egy teszt üzenet az AutoCRM admin felületéről.\n\nHa megérkezett, a kézbesítés működik."
        .to_string();
    let result = mailer
        .send(OutgoingEmail {
            message_id: format!(
                "<admin-test.{}@{}>",
                hex::encode(random),
                env.message_id_domain
            ),
            from: format_from(&env.from_name, &env.from_automatic)?,
            reply_to: Some(env.reply_to_default.clone()),
            to: to.clone(),
            cc: vec![],
            bcc: vec![],
            subject: "AutoCRM teszt e-mail".into(),
            body_html: text_to_html(&body),
            body_text: body,
            attachments: vec![],
            automatic: false,
            in_reply_to: None,
            references: None,
        })
        .await;
    match result {
        Ok(()) if mailer.is_dry_run() => Ok(Json(EmailTestResult {
            ok: true,
            detail: "Dry run: az üzenet naplózva, kiküldés nem történt.".into(),
        })),
        Ok(()) => Ok(Json(EmailTestResult {
            ok: true,
            detail: format!("A szerver elfogadta ({to})."),
        })),
        Err(e) => Ok(Json(EmailTestResult {
            ok: false,
            detail: e.to_string(),
        })),
    }
}
