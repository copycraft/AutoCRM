//! Quote follow-ups: the default sequence (1 hét, 2 hét, 1 hónap...) and each lead's
//! scheduled letters. Reading needs a login; changing anything needs `SendEmail`.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, required};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::followups::{self, Followup, FollowupStep};
use crate::repo::{audit, leads};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_steps, create_step))
        .routes(routes!(update_step))
        .routes(routes!(list_for_lead, schedule))
        .routes(routes!(cancel_all))
        .routes(routes!(cancel_one))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct StepsQuery {
    /// quote (default): letters after a quotation; invoice: payment reminders.
    kind: Option<String>,
}

fn kind_of(kind: Option<&str>) -> AppResult<&str> {
    match kind.unwrap_or("quote") {
        "quote" => Ok("quote"),
        "invoice" => Ok("invoice"),
        _ => Err(AppError::validation("kind must be quote or invoice")),
    }
}

#[utoipa::path(
    get, path = "/followup-steps", tag = "followups",
    params(StepsQuery),
    responses((status = 200, body = Items<FollowupStep>))
)]
async fn list_steps(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<StepsQuery>,
) -> AppResult<Json<Items<FollowupStep>>> {
    Ok(Items::new(followups::steps(&state.db, kind_of(q.kind.as_deref())?).await?))
}

#[derive(Deserialize, ToSchema)]
struct StepBody {
    label: String,
    /// Days after the quotation is sent (quote) or after the payment deadline (invoice).
    delay_days: i32,
    /// An email template key (see /email-templates).
    template_key: String,
    /// quote (default) or invoice.
    kind: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct StepPatch {
    label: Option<String>,
    delay_days: Option<i32>,
    template_key: Option<String>,
    /// Off: new quotations do not schedule it; letters already scheduled stay.
    is_active: Option<bool>,
}

async fn checked(state: &AppState, label: &str, delay_days: i32, template_key: &str) -> AppResult<String> {
    let label = required("label", label)?;
    if !(1..=365).contains(&delay_days) {
        return Err(AppError::validation("delay_days must be between 1 and 365"));
    }
    if !followups::template_exists(&state.db, template_key).await? {
        return Err(AppError::validation(format!("no email template '{template_key}'")));
    }
    Ok(label)
}

#[utoipa::path(
    post, path = "/followup-steps", tag = "followups",
    request_body = StepBody,
    responses((status = 201, body = FollowupStep))
)]
async fn create_step(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<StepBody>,
) -> AppResult<(StatusCode, Json<FollowupStep>)> {
    me.require(Capability::SendEmail)?;
    let label = checked(&state, &b.label, b.delay_days, &b.template_key).await?;
    let kind = kind_of(b.kind.as_deref())?;
    let id = followups::insert_step(&state.db, &label, b.delay_days, &b.template_key, kind).await?;
    audit::record(&state.db, Some(me.user_id), "followup_step", id, "create",
        json!({ "label": label, "delay_days": b.delay_days, "template_key": b.template_key })).await?;
    let step = followups::step(&state.db, id).await?.ok_or(AppError::NotFound("follow-up step"))?;
    Ok((StatusCode::CREATED, Json(step)))
}

#[utoipa::path(
    patch, path = "/followup-steps/{id}", tag = "followups",
    params(("id" = i64, Path)),
    request_body = StepPatch,
    responses((status = 200, body = FollowupStep))
)]
async fn update_step(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<StepPatch>,
) -> AppResult<Json<FollowupStep>> {
    me.require(Capability::SendEmail)?;
    let current = followups::step(&state.db, id).await?.ok_or(AppError::NotFound("follow-up step"))?;
    let delay_days = b.delay_days.unwrap_or(current.delay_days);
    let template_key = b.template_key.unwrap_or_else(|| current.template_key.clone());
    let label = checked(&state, b.label.as_deref().unwrap_or(&current.label), delay_days, &template_key).await?;
    let is_active = b.is_active.unwrap_or(current.is_active);
    followups::update_step(&state.db, id, &label, delay_days, &template_key, is_active).await?;
    let changes = audit::diff(&[
        ("label", json!(current.label), json!(label)),
        ("delay_days", json!(current.delay_days), json!(delay_days)),
        ("template_key", json!(current.template_key), json!(template_key)),
        ("is_active", json!(current.is_active), json!(is_active)),
    ]);
    audit::record(&state.db, Some(me.user_id), "followup_step", id, "update", changes).await?;
    Ok(Json(followups::step(&state.db, id).await?.ok_or(AppError::NotFound("follow-up step"))?))
}

#[utoipa::path(
    get, path = "/leads/{id}/followups", tag = "followups",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Followup>))
)]
async fn list_for_lead(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<Followup>>> {
    Ok(Items::new(followups::for_lead(&state.db, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct ScheduleBody {
    /// Days from now.
    delay_days: i32,
    template_key: String,
    /// Defaults to "N nap".
    label: Option<String>,
}

/// One more follow-up for this lead, counted from now.
#[utoipa::path(
    post, path = "/leads/{id}/followups", tag = "followups",
    params(("id" = i64, Path)),
    request_body = ScheduleBody,
    responses((status = 201, body = Followup))
)]
async fn schedule(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ScheduleBody>,
) -> AppResult<(StatusCode, Json<Followup>)> {
    me.require(Capability::SendEmail)?;
    leads::find(&state.db, id).await?.ok_or(AppError::NotFound("lead"))?;
    let label = b.label.unwrap_or_else(|| format!("{} nap", b.delay_days));
    let label = checked(&state, &label, b.delay_days, &b.template_key).await?;
    let due = Utc::now() + TimeDelta::days(i64::from(b.delay_days));
    let mut tx = state.db.begin().await?;
    let fid = followups::schedule(&mut *tx, id, &label, &b.template_key, due, Some(me.user_id)).await?;
    audit::record(&mut *tx, Some(me.user_id), "lead", id, "followup_scheduled",
        json!({ "label": label, "due_at": due, "template_key": b.template_key })).await?;
    tx.commit().await?;
    let row = followups::find(&state.db, fid).await?.ok_or(AppError::NotFound("follow-up"))?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Serialize, ToSchema)]
struct Cancelled {
    cancelled: u64,
}

/// Stops every follow-up still waiting for this lead.
#[utoipa::path(
    post, path = "/leads/{id}/followups/cancel", tag = "followups",
    params(("id" = i64, Path)),
    responses((status = 200, body = Cancelled))
)]
async fn cancel_all(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Cancelled>> {
    me.require(Capability::SendEmail)?;
    let mut tx = state.db.begin().await?;
    let n = followups::cancel_for_lead(&mut *tx, id, &format!("leállította: {}", me.display_name)).await?;
    if n > 0 {
        audit::record(&mut *tx, Some(me.user_id), "lead", id, "followup_cancelled", json!({ "count": n })).await?;
    }
    tx.commit().await?;
    Ok(Json(Cancelled { cancelled: n }))
}

#[utoipa::path(
    post, path = "/followups/{id}/cancel", tag = "followups",
    params(("id" = i64, Path)),
    responses((status = 200, body = Followup), (status = 422, description = "Not scheduled any more"))
)]
async fn cancel_one(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Followup>> {
    me.require(Capability::SendEmail)?;
    let row = followups::find(&state.db, id).await?.ok_or(AppError::NotFound("follow-up"))?;
    let mut tx = state.db.begin().await?;
    if !followups::cancel(&mut *tx, id, &format!("leállította: {}", me.display_name)).await? {
        return Err(AppError::rule("not_cancellable", "this follow-up is no longer scheduled"));
    }
    audit::record(&mut *tx, Some(me.user_id), "lead", row.lead_id, "followup_cancelled",
        json!({ "label": row.label })).await?;
    tx.commit().await?;
    Ok(Json(followups::find(&state.db, id).await?.ok_or(AppError::NotFound("follow-up"))?))
}
