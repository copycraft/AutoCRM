use std::collections::HashMap;

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
use crate::domain::attribution;
use crate::domain::email::normalize_address;
use crate::domain::lead_tag;
use crate::domain::money::Currency;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::attribution::Attribution;
use crate::repo::documents::{self, Document};
use crate::repo::lead_tags::{self, LeadTagRef};
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
        .routes(routes!(expiring_quotes))
        .routes(routes!(bulk_action))
}

#[derive(Deserialize, ToSchema)]
struct BulkActionBody {
    ids: Vec<i64>,
    #[serde(flatten)]
    action: BulkAction,
}

/// One bulk operation over a ticked set of leads. Stage changes run the same per-lead
/// rules as one-by-one changes, so an invalid transition skips that lead rather than
/// failing the whole batch.
#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum BulkAction {
    Assign { assigned_to: i64 },
    AddTag { tag_id: i64 },
    RemoveTag { tag_id: i64 },
    Stage { stage: String },
}

#[derive(Serialize, ToSchema)]
struct BulkResult {
    applied: i64,
    skipped: Vec<BulkSkip>,
}

#[derive(Serialize, ToSchema)]
struct BulkSkip {
    id: i64,
    error: String,
}

#[utoipa::path(
    post, path = "/leads/bulk-actions", tag = "leads",
    request_body = BulkActionBody,
    responses((status = 200, body = BulkResult))
)]
async fn bulk_action(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<BulkActionBody>,
) -> AppResult<Json<BulkResult>> {
    me.require(Capability::EditLeads)?;
    let mut applied = 0;
    let mut skipped = Vec::new();
    for id in b.ids.iter().copied().take(500) {
        let outcome: AppResult<()> = match &b.action {
            BulkAction::Assign { assigned_to } => {
                leads::set_assigned(&state.db, id, Some(*assigned_to))
                    .await
                    .map_err(|e| e.into())
            }
            BulkAction::AddTag { tag_id } => {
                crate::repo::lead_tags::link(&state.db, id, *tag_id, None, Some(me.user_id))
                    .await
                    .map_err(|e| e.into())
            }
            BulkAction::RemoveTag { tag_id } => {
                crate::repo::lead_tags::remove_tag(&state.db, id, *tag_id)
                    .await
                    .map_err(|e| e.into())
            }
            BulkAction::Stage { stage } => {
                service::stages::change_lead_stage(&state.db, &me, id, stage.trim(), None)
                    .await
                    .map(|_| ())
            }
        };
        match outcome {
            Ok(()) => applied += 1,
            Err(e) => skipped.push(BulkSkip { id, error: e.to_string() }),
        }
    }
    Ok(Json(BulkResult { applied, skipped }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ExpiringQuery {
    /// How many days around today to cover; defaults to 7.
    days: Option<i64>,
    assigned_to: Option<i64>,
}

#[utoipa::path(
    get, path = "/leads/quotes/expiring", tag = "leads",
    params(ExpiringQuery),
    responses((status = 200, body = Items<service::reminders::ExpiringQuote>))
)]
async fn expiring_quotes(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ExpiringQuery>,
) -> AppResult<Json<Items<service::reminders::ExpiringQuote>>> {
    let today = service::business_today(state.config.business_tz);
    let days = q.days.unwrap_or(7).clamp(1, 90);
    Ok(Items::new(
        service::reminders::expiring_quotes(&state.db, today, days, q.assigned_to).await?,
    ))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    q: Option<String>,
    /// Stage key.
    stage: Option<String>,
    assigned_to: Option<i64>,
    /// Only leads carrying this tag.
    tag: Option<i64>,
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
    responses((status = 200, body = Items<LeadRow>))
)]
async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<LeadRow>>> {
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
        q.tag,
        q.open,
        &sort_key,
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let mut tags: HashMap<i64, Vec<LeadTagRef>> = HashMap::new();
    for (lead_id, tag) in lead_tags::for_leads(&state.db, &ids).await? {
        tags.entry(lead_id).or_default().push(tag);
    }
    let rows = rows
        .into_iter()
        .map(|lead| LeadRow {
            tags: tags.remove(&lead.id).unwrap_or_default(),
            lead,
        })
        .collect();
    Ok(Items::new(rows))
}

/// A lead in the list, with its tags.
#[derive(Serialize, ToSchema)]
struct LeadRow {
    #[serde(flatten)]
    lead: LeadSummary,
    tags: Vec<LeadTagRef>,
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
    /// Create only: the tags to file the lead under. Tags whose domain the source names
    /// are added on their own. Change them later with `PUT /leads/{id}/tags`.
    #[serde(default)]
    tag_ids: Vec<i64>,
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
    ApiJson(mut b): ApiJson<LeadBody>,
) -> AppResult<(StatusCode, Json<Lead>)> {
    me.require(Capability::EditLeads)?;
    let tag_ids = std::mem::take(&mut b.tag_ids);
    let input = merge(None, b)?;
    let lead = service::leads::create(&state.db, &me, input, &tag_ids).await?;
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
    /// UTM tags from the landing URL (`?utm_source=...`), if the visitor arrived tagged.
    utm_source: Option<String>,
    utm_medium: Option<String>,
    utm_campaign: Option<String>,
    /// The page that sent the visitor (`document.referrer` at their first visit).
    referrer: Option<String>,
    /// The first page of the visit, which is not always the page the form is on.
    landing_page: Option<String>,
    /// The website the form is on (`hutoautok.hu` or its URL), when several sites post
    /// here. Lead tags claiming that domain are put on the lead.
    site: Option<String>,
}

const MAX_NAME: usize = 200;
const MAX_FIELD: usize = 300;
const MAX_MESSAGE: usize = 5000;
const MAX_URL: usize = 500;

fn capped(field: &str, value: Option<String>, max: usize) -> AppResult<Option<String>> {
    let value = optional(value);
    match &value {
        Some(v) if v.chars().count() > max => Err(AppError::validation(format!(
            "{field} is too long (at most {max} characters)"
        ))),
        _ => Ok(value),
    }
}

/// A website form, as the lead to file, where it came from, and every host known about the
/// visit for recognising lead tags. `origin` and `referer` are the request headers: a form
/// posted straight from the browser carries the site in them.
fn lead_from_website(
    b: WebsiteLead,
    origin: Option<&str>,
    referer: Option<&str>,
) -> AppResult<(LeadInput, Attribution, Vec<String>)> {
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
    let utm_source = capped("utm_source", b.utm_source, MAX_FIELD)?;
    let utm_medium = capped("utm_medium", b.utm_medium, MAX_FIELD)?;
    let utm_campaign = capped("utm_campaign", b.utm_campaign, MAX_FIELD)?;
    let referrer = capped("referrer", b.referrer, MAX_URL)?;
    let landing_page = capped("landing_page", b.landing_page, MAX_URL)?;
    let site = capped("site", b.site, MAX_URL)?;
    // Strongest evidence first: the site the form is on, then where the visit began, then
    // whatever sent the visitor.
    let hosts = lead_tag::hosts_of([
        site.as_deref(),
        page.as_deref(),
        landing_page.as_deref(),
        origin,
        referer,
        utm_source.as_deref(),
        referrer.as_deref(),
    ]);
    let attribution = Attribution {
        channel: attribution::classify(
            utm_source.as_deref(),
            utm_medium.as_deref(),
            referrer.as_deref(),
        )
        .as_str()
        .to_string(),
        utm_source,
        utm_medium,
        utm_campaign,
        referrer,
        landing_page,
    };

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
    let input = LeadInput {
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
    };
    Ok((input, attribution, hosts))
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
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let (input, attribution, hosts) =
        lead_from_website(b, header("origin"), header("referer"))?;
    let lead =
        service::leads::create_from_website(&state.db, input, &attribution, &hosts).await?;
    // The lead is already committed; a failed alert is logged, never the visitor's problem.
    if let Err(e) = service::email::website_lead_alert(&state, &lead).await {
        tracing::error!(lead_id = lead.id, error = %e, "could not queue the website lead alert");
    }
    if let Err(e) = service::notifications::lead_arrived(&state.db, &lead).await {
        tracing::error!(lead_id = lead.id, error = %e, "could not raise the website lead notification");
    }
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
    /// Where the lead came from, for leads the website filed.
    attribution: Option<Attribution>,
    /// The lead tags, each with the domain that put it there when the server did.
    tags: Vec<LeadTagRef>,
    /// Why it was lost, when it was.
    lost_reason: Option<crate::repo::lost_reasons::LostReason>,
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
    let attribution = crate::repo::attribution::find(&state.db, id).await?;
    let tags = lead_tags::for_lead(&state.db, id).await?;
    Ok(Json(LeadDetail {
        lead,
        stage,
        history,
        orders,
        documents,
        attribution,
        tags,
        lost_reason: crate::repo::lost_reasons::for_lead(&state.db, id).await?,
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
    /// Why it was lost, when moving to `lost`: an id from `/lost-reasons`.
    pub lost_reason_id: Option<i64>,
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
    if let Some(reason) = b.lost_reason_id
        && b.stage.trim() == "lost"
        && crate::repo::lost_reasons::find(&state.db, reason).await?.is_none()
    {
        return Err(AppError::validation("lost_reason_id does not exist"));
    }
    let change =
        service::stages::change_lead_stage(&state.db, &me, id, b.stage.trim(), note.as_deref())
            .await?;
    // The reason belongs to the lost stage: set on the way in, cleared on the way out.
    let reason = if b.stage.trim() == "lost" { b.lost_reason_id } else { None };
    crate::repo::lost_reasons::set_for_lead(&state.db, id, reason).await?;
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
