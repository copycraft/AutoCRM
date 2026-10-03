use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::orders::{OrderBody, fields_from_body};
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::money::Currency;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::documents::{self, Document};
use crate::repo::leads::{Lead, LeadInput, LeadSummary};
use crate::repo::orders::Order;
use crate::repo::stages::{CurrentStage, StageEntry};
use crate::repo::{audit, leads, like_pattern, orders, parse_sort, phone_pattern, stages};
use crate::service;
use crate::service::email::QuotationRequest;
use crate::service::stages::{StageChange, TransitionOption};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(search, create))
        .routes(routes!(website))
        .routes(routes!(detail, update))
        .routes(routes!(change_stage))
        .routes(routes!(transitions))
        .routes(routes!(convert))
        .routes(routes!(quotation))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    q: Option<String>,
    /// Stage key.
    stage: Option<String>,
    assigned_to: Option<i64>,
    /// Only leads not in a terminal stage.
    #[serde(default)]
    open: bool,
    /// Sort key, `-` prefix for descending: created_at, title.
    sort: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/leads", tag = "leads",
    params(SearchQuery),
    responses((status = 200, body = Items<LeadSummary>))
)]
async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<LeadSummary>>> {
    let pattern = q.q.as_deref().and_then(like_pattern);
    let phone = q.q.as_deref().and_then(phone_pattern);
    let sort_key = parse_sort(q.sort.as_deref(), leads::LEAD_SORTS, leads::DEFAULT_SORT)
        .map_err(AppError::validation)?;
    let rows = leads::search(
        &state.db,
        pattern.as_deref(),
        phone.as_deref(),
        optional(q.stage).as_deref(),
        q.assigned_to,
        q.open,
        &sort_key,
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    Ok(Items::new(rows))
}

/// Create requires `title`. On PATCH every field is optional; `null` clears.
#[derive(Deserialize, ToSchema)]
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
    /// V2.3: the quoted price in minor units (fillér / eurocent).
    #[serde(default, deserialize_with = "patch_field")]
    quoted_value_minor: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    currency: Option<Option<Currency>>,
    #[serde(default, deserialize_with = "patch_field")]
    quote_valid_until: Option<Option<NaiveDate>>,
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
        quoted_value_minor: keep(
            current.and_then(|l| l.quoted_value_minor),
            b.quoted_value_minor,
        ),
        currency: match b.currency {
            Some(v) => v.map(|c| c.code().to_string()),
            None => current.and_then(|l| l.currency.clone()),
        },
        quote_valid_until: match b.quote_valid_until {
            Some(v) => v,
            None => current.and_then(|l| l.quote_valid_until),
        },
    })
}

#[utoipa::path(
    post, path = "/leads", tag = "leads",
    request_body = LeadBody,
    responses((status = 201, body = Lead))
)]
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

/// An enquiry from the autotherm.hu contact form.
#[derive(Deserialize, ToSchema)]
pub struct WebsiteLead {
    /// Who is asking.
    name: String,
    /// At least one of `email` and `phone` is required, so the office can answer.
    email: Option<String>,
    phone: Option<String>,
    /// The visitor's message.
    message: Option<String>,
    /// What they ask about, e.g. the service. Becomes the lead title.
    subject: Option<String>,
    /// The vehicle, if the form asks for it.
    vehicle: Option<String>,
    /// The page the form was sent from.
    page: Option<String>,
    /// Honeypot: the form renders this hidden. A bot fills it, a person never does; such
    /// a submission is acknowledged and dropped.
    company: Option<String>,
}

const MAX_NAME: usize = 200;
const MAX_FIELD: usize = 300;
const MAX_MESSAGE: usize = 5000;

fn capped(field: &str, value: Option<String>, max: usize) -> AppResult<Option<String>> {
    let value = optional(value);
    match &value {
        Some(v) if v.chars().count() > max => Err(AppError::validation(format!(
            "{field} is too long (at most {max} characters)"
        ))),
        _ => Ok(value),
    }
}

fn lead_from_website(b: WebsiteLead) -> AppResult<LeadInput> {
    let name = capped("name", Some(b.name), MAX_NAME)?
        .ok_or_else(|| AppError::validation("name is required"))?;
    let email = match capped("email", b.email, MAX_FIELD)? {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("email is not a valid address"))?,
        ),
        None => None,
    };
    let phone = capped("phone", b.phone, 50)?;
    if email.is_none() && phone.is_none() {
        return Err(AppError::validation("email or phone is required"));
    }
    let subject = capped("subject", b.subject, MAX_FIELD)?;
    let message = capped("message", b.message, MAX_MESSAGE)?;
    let vehicle = capped("vehicle", b.vehicle, MAX_FIELD)?;
    let page = capped("page", b.page, MAX_FIELD)?;

    let mut parts = Vec::new();
    if let Some(m) = message {
        parts.push(m);
    }
    if let Some(v) = vehicle {
        parts.push(format!("Jármű: {v}"));
    }
    if let Some(p) = page {
        parts.push(format!("Oldal: {p}"));
    }
    Ok(LeadInput {
        title: format!("Weboldal: {}", subject.unwrap_or_else(|| name.clone())),
        partner_id: None,
        contact_id: None,
        contact_name: Some(name),
        contact_email: email,
        contact_phone: phone,
        source: Some("website".into()),
        description: (!parts.is_empty()).then(|| parts.join("\n\n")),
        assigned_to: None,
        quoted_value_minor: None,
        currency: None,
        quote_valid_until: None,
    })
}

/// Website enquiry: `POST` the form with `X-Leads-Key`. No staff login; the key keeps
/// strangers out, and with `LEADS_API_KEY` unset the endpoint is off. The lead lands in
/// the first stage, unassigned, with source `website`.
#[utoipa::path(
    post, path = "/leads/website", tag = "leads",
    request_body = WebsiteLead,
    responses(
        (status = 202, description = "Accepted"),
        (status = 403, description = "Missing or wrong key, or the endpoint is off"),
        (status = 400, description = "Invalid form"),
    )
)]
async fn website(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(b): ApiJson<WebsiteLead>,
) -> AppResult<StatusCode> {
    super::check_api_key(
        state.config.leads_api_key.as_deref(),
        &headers,
        "x-leads-key",
    )?;
    if optional(b.company.clone()).is_some() {
        return Ok(StatusCode::ACCEPTED);
    }
    let input = lead_from_website(b)?;
    service::leads::create_from_website(&state.db, input).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Serialize, ToSchema)]
struct OrderRef {
    id: i64,
    number: String,
}

#[derive(Serialize, ToSchema)]
struct LeadDetail {
    lead: Lead,
    stage: Option<CurrentStage>,
    history: Vec<StageEntry>,
    /// V2.7: every order this enquiry became. Three identical Sprinters from one enquiry
    /// are three orders, and all three point back here.
    orders: Vec<OrderRef>,
    /// V2.4: the quotation and anything else filed against the enquiry itself.
    documents: Vec<Document>,
}

#[utoipa::path(
    get, path = "/leads/{id}", tag = "leads",
    params(("id" = i64, Path)),
    responses((status = 200, body = LeadDetail))
)]
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
    let orders = orders::find_by_lead(&state.db, id)
        .await?
        .into_iter()
        .map(|(id, number)| OrderRef { id, number })
        .collect();
    let documents = documents::list_for_lead(&state.db, id).await?;
    Ok(Json(LeadDetail {
        lead,
        stage,
        history,
        orders,
        documents,
    }))
}

#[utoipa::path(
    patch, path = "/leads/{id}", tag = "leads",
    params(("id" = i64, Path)),
    request_body = LeadBody,
    responses((status = 200, body = Lead))
)]
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
    // Create refuses dangling and mismatched relations; an update that swaps the
    // partner under a kept contact must not smuggle one in (N1). Archived partners
    // take no new work here either, like on orders (N6).
    service::leads::check_relations(&mut tx, input.partner_id, input.contact_id).await?;
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
        // V2.3: the number the sales pipeline is made of. A revised price before
        // acceptance is the case the audit entry exists for.
        (
            "quoted_value_minor",
            json!(current.quoted_value_minor),
            json!(updated.quoted_value_minor),
        ),
        ("currency", json!(current.currency), json!(updated.currency)),
        (
            "quote_valid_until",
            json!(current.quote_valid_until),
            json!(updated.quote_valid_until),
        ),
    ]);
    audit::record(&mut *tx, Some(me.user_id), "lead", id, "update", changes).await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[derive(Deserialize, ToSchema)]
pub struct StageBody {
    /// Target stage key.
    pub stage: String,
    /// Required for backward moves and reopening a terminal stage.
    pub note: Option<String>,
}

#[utoipa::path(
    post, path = "/leads/{id}/stage", tag = "leads",
    params(("id" = i64, Path)),
    request_body = StageBody,
    responses((status = 200, body = StageChange))
)]
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

#[utoipa::path(
    get, path = "/leads/{id}/transitions", tag = "leads",
    params(("id" = i64, Path)),
    responses((status = 200, description = "Manual stage targets with note requirements; `won` is listed with `manual: false`", body = Items<TransitionOption>))
)]
async fn transitions(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<TransitionOption>>> {
    Ok(Items::new(
        service::stages::lead_transitions(&state.db, id).await?,
    ))
}

#[utoipa::path(
    post, path = "/leads/{id}/convert", tag = "leads",
    params(("id" = i64, Path)),
    request_body(content = OrderBody, description = "`partner_id` may be omitted when the lead has one; `title` defaults to the lead's."),
    responses((status = 201, body = Order))
)]
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

/// The queued quotation letter.
#[derive(Serialize, ToSchema)]
pub struct QuotationSent {
    pub email_id: i64,
}

/// Send the quotation letter for a lead: hero band on top, the lead's quotation PDF
/// attached, from the staff member as themselves. Writing to a stranger's money needs a
/// person behind it, so this is `SendEmail` (office), not an automatic trigger.
#[utoipa::path(
    post, path = "/leads/{id}/quotation", tag = "leads",
    params(("id" = i64, Path)),
    request_body = QuotationRequest,
    responses((status = 202, body = QuotationSent))
)]
async fn quotation(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<QuotationRequest>,
) -> AppResult<(StatusCode, Json<QuotationSent>)> {
    me.require(Capability::SendEmail)?;
    let email_id = service::email::send_quotation(&state, &me, id, &b).await?;
    Ok((StatusCode::ACCEPTED, Json(QuotationSent { email_id })))
}
