//! Lead tags: the per-market lists the office files enquiries under, and the website
//! domains each tag recognises.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::lead_tag;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::lead_tags::{self, LeadTag, LeadTagRef, TagInput};
use crate::repo::{audit, leads, lost_reasons};
use crate::service;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_tags, create_tag))
        .routes(routes!(update_tag))
        .routes(routes!(reorder_tags))
        .routes(routes!(set_lead_tags))
        .routes(routes!(list_lost_reasons, create_lost_reason))
        .routes(routes!(update_lost_reason))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// The archived tags instead of the live ones.
    #[serde(default)]
    archived: bool,
}

#[utoipa::path(
    get, path = "/lead-tags", tag = "leads",
    params(ListQuery),
    responses((status = 200, body = Items<LeadTag>))
)]
async fn list_tags(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<LeadTag>>> {
    Ok(Items::new(lead_tags::list(&state.db, q.archived).await?))
}

#[derive(Deserialize, ToSchema)]
struct CreateTag {
    /// Two-letter market code: hu, ro, de, it...
    market: String,
    label: String,
    /// `#rrggbb`; a neutral grey when omitted.
    color: Option<String>,
    /// Website domains or URLs; stored as bare hosts.
    #[serde(default)]
    domains: Vec<String>,
}

/// Every field optional; absent keeps the current value.
#[derive(Deserialize, ToSchema)]
struct UpdateTag {
    market: Option<String>,
    label: Option<String>,
    color: Option<String>,
    domains: Option<Vec<String>>,
    /// true archives: the tag leaves the lists and stops matching, leads keep it.
    archived: Option<bool>,
}

/// Checks and normalises a tag, refusing a domain another live tag already claims: two
/// tags for one site would leave which one a lead gets to list order.
async fn validated(
    state: &AppState,
    market: &str,
    label: &str,
    color: &str,
    domains: &[String],
    // The tag being edited, or None for a new one; an archived tag claims nothing.
    except: Option<i64>,
    claims_domains: bool,
) -> AppResult<TagInput> {
    let market = market.trim().to_lowercase();
    if !lead_tag::valid_market(&market) {
        return Err(AppError::validation(
            "market must be a two-letter code such as hu or de",
        ));
    }
    let label = super::required("label", label)?;
    if label.chars().count() > 100 {
        return Err(AppError::validation("label is too long (at most 100 characters)"));
    }
    let color = lead_tag::normalize_color(color)
        .ok_or_else(|| AppError::validation("color must look like #a33122"))?;
    let mut hosts = Vec::new();
    for d in domains.iter().filter(|d| !d.trim().is_empty()) {
        let host = lead_tag::normalize_domain(d)
            .ok_or_else(|| AppError::validation(format!("{} is not a domain", d.trim())))?;
        if !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    let owner = if claims_domains {
        lead_tags::domain_owner(&state.db, &hosts, except).await?
    } else {
        None
    };
    if let Some((owner, domain)) = owner {
        return Err(AppError::conflict(
            "domain_taken",
            format!("{domain} already belongs to the tag {owner}"),
        ));
    }
    Ok(TagInput {
        market,
        label,
        color,
        domains: hosts,
    })
}

#[utoipa::path(
    post, path = "/lead-tags", tag = "leads",
    request_body = CreateTag,
    responses((status = 201, body = LeadTag), (status = 409, description = "A domain belongs to another tag, or the label is taken in that market"))
)]
async fn create_tag(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateTag>,
) -> AppResult<(StatusCode, Json<LeadTag>)> {
    me.require(Capability::EditLeads)?;
    let input = validated(
        &state,
        &b.market,
        &b.label,
        b.color.as_deref().unwrap_or("#64748b"),
        &b.domains,
        None,
        true,
    )
    .await?;
    let id = lead_tags::insert(&state.db, &input).await?;
    audit::record(
        &state.db,
        Some(me.user_id),
        "lead_tag",
        id,
        "create",
        json!({ "market": input.market, "label": input.label, "domains": input.domains }),
    )
    .await?;
    let tag = lead_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead tag"))?;
    Ok((StatusCode::CREATED, Json(tag)))
}

#[utoipa::path(
    patch, path = "/lead-tags/{id}", tag = "leads",
    params(("id" = i64, Path)),
    request_body = UpdateTag,
    responses((status = 200, body = LeadTag), (status = 409, description = "A domain belongs to another tag, or the label is taken in that market"))
)]
async fn update_tag(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<UpdateTag>,
) -> AppResult<Json<LeadTag>> {
    me.require(Capability::EditLeads)?;
    let current = lead_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead tag"))?;
    let archived = b.archived.unwrap_or(current.archived_at.is_some());
    let input = validated(
        &state,
        b.market.as_deref().unwrap_or(&current.market),
        b.label.as_deref().unwrap_or(&current.label),
        b.color.as_deref().unwrap_or(&current.color),
        b.domains.as_deref().unwrap_or(&current.domains),
        Some(id),
        !archived,
    )
    .await?;
    lead_tags::update(&state.db, id, &input, archived).await?;
    let changes = audit::diff(&[
        ("market", json!(current.market), json!(input.market)),
        ("label", json!(current.label), json!(input.label)),
        ("color", json!(current.color), json!(input.color)),
        ("domains", json!(current.domains), json!(input.domains)),
        (
            "archived",
            json!(current.archived_at.is_some()),
            json!(archived),
        ),
    ]);
    audit::record(&state.db, Some(me.user_id), "lead_tag", id, "update", changes).await?;
    let tag = lead_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead tag"))?;
    Ok(Json(tag))
}

#[derive(Deserialize, ToSchema)]
struct ReorderTags {
    market: String,
    /// The market's tags, top first.
    ids: Vec<i64>,
}

#[utoipa::path(
    put, path = "/lead-tags/order", tag = "leads",
    request_body = ReorderTags,
    responses((status = 204, description = "Reordered"))
)]
async fn reorder_tags(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ReorderTags>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditLeads)?;
    lead_tags::reorder(&state.db, b.market.trim(), &b.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct LeadTagsBody {
    /// The lead's tags after the change; every other tag is removed.
    tag_ids: Vec<i64>,
}

#[utoipa::path(
    put, path = "/leads/{id}/tags", tag = "leads",
    params(("id" = i64, Path)),
    request_body = LeadTagsBody,
    responses((status = 200, body = Items<LeadTagRef>))
)]
async fn set_lead_tags(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LeadTagsBody>,
) -> AppResult<Json<Items<LeadTagRef>>> {
    me.require(Capability::EditLeads)?;
    leads::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    Ok(Items::new(
        service::leads::set_tags(&state.db, &me, id, &b.tag_ids).await?,
    ))
}

// ── Lost reasons ────────────────────────────────────────────────────────────

/// Why leads are lost: the choices offered when a lead moves to "Elveszett".
#[utoipa::path(
    get, path = "/lost-reasons", tag = "leads",
    params(ListQuery),
    responses((status = 200, body = Items<lost_reasons::LostReason>))
)]
async fn list_lost_reasons(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<lost_reasons::LostReason>>> {
    Ok(Items::new(lost_reasons::list(&state.db, q.archived).await?))
}

#[derive(Deserialize, ToSchema)]
struct LostReasonBody {
    label: String,
    /// true archives: it leaves the choices, lost leads keep it.
    #[serde(default)]
    archived: bool,
}

fn reason_label(label: &str) -> AppResult<String> {
    let label = super::required("label", label)?;
    if label.chars().count() > 100 {
        return Err(AppError::validation("label is too long (at most 100 characters)"));
    }
    Ok(label)
}

fn unique_label(e: sqlx::Error) -> AppError {
    match &e {
        sqlx::Error::Database(d) if d.is_unique_violation() => {
            AppError::conflict("duplicate", "there is already a reason with this name")
        }
        _ => e.into(),
    }
}

#[utoipa::path(
    post, path = "/lost-reasons", tag = "leads",
    request_body = LostReasonBody,
    responses((status = 201, body = lost_reasons::LostReason))
)]
async fn create_lost_reason(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<LostReasonBody>,
) -> AppResult<(StatusCode, Json<lost_reasons::LostReason>)> {
    me.require(Capability::EditLeads)?;
    let label = reason_label(&b.label)?;
    let id = lost_reasons::insert(&state.db, &label)
        .await
        .map_err(unique_label)?;
    let reason = lost_reasons::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lost reason"))?;
    Ok((StatusCode::CREATED, Json(reason)))
}

#[utoipa::path(
    put, path = "/lost-reasons/{id}", tag = "leads",
    params(("id" = i64, Path)),
    request_body = LostReasonBody,
    responses((status = 200, body = lost_reasons::LostReason))
)]
async fn update_lost_reason(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LostReasonBody>,
) -> AppResult<Json<lost_reasons::LostReason>> {
    me.require(Capability::EditLeads)?;
    let label = reason_label(&b.label)?;
    if !lost_reasons::update(&state.db, id, &label, b.archived)
        .await
        .map_err(unique_label)?
    {
        return Err(AppError::NotFound("lost reason"));
    }
    let reason = lost_reasons::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("lost reason"))?;
    Ok(Json(reason))
}
