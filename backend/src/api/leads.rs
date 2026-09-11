use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::orders::{OrderBody, fields_from_body};
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::leads::{Lead, LeadInput, LeadSummary};
use crate::repo::orders::Order;
use crate::repo::stages::{CurrentStage, StageEntry};
use crate::repo::{audit, leads, like_pattern, orders, stages};
use crate::service::{self, stages::StageChange};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/leads", get(search).post(create))
        .route("/leads/{id}", get(detail).patch(update))
        .route("/leads/{id}/stage", post(change_stage))
        .route("/leads/{id}/convert", post(convert))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: Option<String>,
    stage: Option<String>,
    assigned_to: Option<i64>,
    #[serde(default)]
    open: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<LeadSummary>>> {
    let pattern = q.q.as_deref().and_then(like_pattern);
    let rows = leads::search(
        &state.db,
        pattern.as_deref(),
        optional(q.stage).as_deref(),
        q.assigned_to,
        q.open,
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    Ok(Items::new(rows))
}

#[derive(Deserialize)]
struct LeadBody {
    title: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    partner_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    contact_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    contact_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    contact_email: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    contact_phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    source: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    assigned_to: Option<Option<i64>>,
}

fn merge(current: Option<&Lead>, b: LeadBody) -> AppResult<LeadInput> {
    let title = b
        .title
        .or_else(|| current.map(|l| l.title.clone()))
        .unwrap_or_default();
    let contact_email = patch_text(
        &current.and_then(|l| l.contact_email.clone()),
        b.contact_email,
    );
    let contact_email = match contact_email {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("contact_email is not a valid address"))?,
        ),
        None => None,
    };
    let keep = |cur: Option<i64>, p: Option<Option<i64>>| p.unwrap_or(cur);
    Ok(LeadInput {
        title: required("title", &title)?,
        partner_id: keep(current.and_then(|l| l.partner_id), b.partner_id),
        contact_id: keep(current.and_then(|l| l.contact_id), b.contact_id),
        contact_name: patch_text(
            &current.and_then(|l| l.contact_name.clone()),
            b.contact_name,
        ),
        contact_email,
        contact_phone: patch_text(
            &current.and_then(|l| l.contact_phone.clone()),
            b.contact_phone,
        ),
        source: patch_text(&current.and_then(|l| l.source.clone()), b.source),
        description: patch_text(&current.and_then(|l| l.description.clone()), b.description),
        assigned_to: keep(current.and_then(|l| l.assigned_to), b.assigned_to),
    })
}

async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<LeadBody>,
) -> AppResult<(StatusCode, Json<Lead>)> {
    me.require(Capability::EditLeads)?;
    let input = merge(None, b)?;
    let lead = service::leads::create(&state.db, &me, input).await?;
    Ok((StatusCode::CREATED, Json(lead)))
}

#[derive(Serialize)]
struct OrderRef {
    id: i64,
    number: String,
}

#[derive(Serialize)]
struct LeadDetail {
    lead: Lead,
    stage: Option<CurrentStage>,
    history: Vec<StageEntry>,
    order: Option<OrderRef>,
}

async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<LeadDetail>> {
    let lead = leads::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let stage = stages::current_lead_stage(&state.db, id).await?;
    let history = stages::lead_history(&state.db, id).await?;
    let order = orders::find_by_lead(&state.db, id)
        .await?
        .map(|(id, number)| OrderRef { id, number });
    Ok(Json(LeadDetail {
        lead,
        stage,
        history,
        order,
    }))
}

async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LeadBody>,
) -> AppResult<Json<Lead>> {
    me.require(Capability::EditLeads)?;
    let mut tx = state.db.begin().await?;
    let current = leads::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let input = merge(Some(&current), b)?;
    let updated = leads::update(&mut *tx, id, &input)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let changes = audit::diff(&[
        ("title", json!(current.title), json!(updated.title)),
        (
            "partner_id",
            json!(current.partner_id),
            json!(updated.partner_id),
        ),
        (
            "contact_id",
            json!(current.contact_id),
            json!(updated.contact_id),
        ),
        (
            "contact_name",
            json!(current.contact_name),
            json!(updated.contact_name),
        ),
        (
            "contact_email",
            json!(current.contact_email),
            json!(updated.contact_email),
        ),
        (
            "contact_phone",
            json!(current.contact_phone),
            json!(updated.contact_phone),
        ),
        ("source", json!(current.source), json!(updated.source)),
        (
            "description",
            json!(current.description),
            json!(updated.description),
        ),
        (
            "assigned_to",
            json!(current.assigned_to),
            json!(updated.assigned_to),
        ),
    ]);
    audit::record(&mut *tx, Some(me.user_id), "lead", id, "update", changes).await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
pub struct StageBody {
    pub stage: String,
    pub note: Option<String>,
}

async fn change_stage(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<StageBody>,
) -> AppResult<Json<StageChange>> {
    me.require(Capability::EditLeads)?;
    let note = optional(b.note);
    let change =
        service::stages::change_lead_stage(&state.db, &me, id, b.stage.trim(), note.as_deref())
            .await?;
    Ok(Json(change))
}

async fn convert(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<OrderBody>,
) -> AppResult<(StatusCode, Json<Order>)> {
    me.require(Capability::EditLeads)?;
    me.require(Capability::EditOrders)?;
    let today = service::business_today(state.config.business_tz);
    let lead = leads::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let partner_id = b.partner_id.or(lead.partner_id);
    let title_fallback = lead.title.clone();
    let (mut fields, items) =
        fields_from_body(b, partner_id.unwrap_or(0), today, Some(title_fallback))?;
    fields.partner_id = partner_id.unwrap_or(0);
    let order = service::leads::convert(
        &state.db,
        &me,
        id,
        today,
        service::leads::Conversion {
            partner_id,
            fields,
            items,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(order)))
}
