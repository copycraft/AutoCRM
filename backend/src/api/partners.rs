use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::money::Currency;
use crate::domain::partner::{PartnerKind, normalize_country, normalize_hu_tax_number};
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::contacts::{Contact, ContactInput};
use crate::repo::leads::LeadSummary;
use crate::repo::orders::{OrderFilter, OrderSummary};
use crate::repo::partners::{Partner, PartnerInput};
use crate::repo::{audit, contacts, leads, like_pattern, orders, parse_sort, partners, phone_pattern};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(search, create))
        .routes(routes!(detail, update))
        .routes(routes!(archive))
        .routes(routes!(unarchive))
        .routes(routes!(list_contacts, create_contact))
        .routes(routes!(update_contact))
        .routes(routes!(archive_contact))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    /// Matches name, tax number, e-mail, city — and phone in any
    /// +36/06/0036 spacing variant.
    q: Option<String>,
    kind: Option<PartnerKind>,
    /// V2.6: `customer` or `supplier`. A partner with no role set counts as a customer,
    /// so the picker never hides someone; `both` matches either filter.
    role: Option<String>,
    #[serde(default)]
    include_archived: bool,
    /// Sort key, `-` prefix for descending: name, created_at.
    sort: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/partners", tag = "partners",
    params(SearchQuery),
    responses((status = 200, body = Items<Partner>))
)]
async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<Partner>>> {
    let pattern = q.q.as_deref().and_then(like_pattern);
    let phone = q.q.as_deref().and_then(phone_pattern);
    let sort_key = parse_sort(
        q.sort.as_deref(),
        partners::PARTNER_SORTS,
        partners::DEFAULT_SORT,
    )
    .map_err(AppError::validation)?;
    if let Some(role) = q.role.as_deref()
        && !matches!(role, "customer" | "supplier" | "both")
    {
        return Err(AppError::validation(
            "role must be customer, supplier or both",
        ));
    }
    let rows = partners::search(
        &state.db,
        pattern.as_deref(),
        phone.as_deref(),
        q.kind,
        q.role.as_deref(),
        q.include_archived,
        &sort_key,
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    Ok(Items::new(rows))
}

/// Every field of a partner, pre-validation. Create and PATCH both funnel through this.
struct PartnerDraft {
    kind: PartnerKind,
    name: String,
    tax_number: Option<String>,
    eu_tax_number: Option<String>,
    country: String,
    default_currency: Currency,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    postal_code: Option<String>,
    city: Option<String>,
    address_line: Option<String>,
    notes: Option<String>,
    role: Option<String>,
}

fn validate(d: PartnerDraft) -> AppResult<PartnerInput> {
    let country = normalize_country(&d.country)
        .ok_or_else(|| AppError::validation("country must be a two-letter ISO code"))?;
    let tax_number = match optional(d.tax_number) {
        Some(t) if country == "HU" => {
            Some(normalize_hu_tax_number(&t).map_err(AppError::validation)?)
        }
        other => other,
    };
    let email = match optional(d.email) {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("email is not a valid address"))?,
        ),
        None => None,
    };
    Ok(PartnerInput {
        kind: d.kind,
        name: required("name", &d.name)?,
        tax_number,
        eu_tax_number: optional(d.eu_tax_number).map(|t| t.replace(' ', "").to_uppercase()),
        country,
        default_currency: d.default_currency.code().to_string(),
        email,
        phone: optional(d.phone),
        website: optional(d.website),
        postal_code: optional(d.postal_code),
        city: optional(d.city),
        address_line: optional(d.address_line),
        notes: optional(d.notes),
        role: match optional(d.role) {
            Some(r) if matches!(r.as_str(), "customer" | "supplier" | "both") => Some(r),
            Some(_) => {
                return Err(AppError::validation(
                    "role must be customer, supplier or both",
                ));
            }
            None => None,
        },
    })
}

#[derive(Deserialize, ToSchema)]
struct CreatePartner {
    kind: PartnerKind,
    name: String,
    /// Hungarian tax numbers are normalised to `12345678-1-23`.
    tax_number: Option<String>,
    eu_tax_number: Option<String>,
    /// ISO 3166-1 alpha-2; defaults to `HU`.
    country: Option<String>,
    /// Defaults to `HUF`.
    default_currency: Option<Currency>,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    postal_code: Option<String>,
    city: Option<String>,
    address_line: Option<String>,
    notes: Option<String>,
    /// `customer` | `supplier` | `both`. Omitted means unclassified, which the pickers
    /// treat as a customer.
    role: Option<String>,
}

#[utoipa::path(
    post, path = "/partners", tag = "partners",
    request_body = CreatePartner,
    responses((status = 201, body = Partner))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreatePartner>,
) -> AppResult<(StatusCode, Json<Partner>)> {
    me.require(Capability::EditPartners)?;
    let input = validate(PartnerDraft {
        kind: b.kind,
        name: b.name,
        tax_number: b.tax_number,
        eu_tax_number: b.eu_tax_number,
        country: b.country.unwrap_or_else(|| "HU".into()),
        default_currency: b.default_currency.unwrap_or(Currency::HUF),
        email: b.email,
        phone: b.phone,
        website: b.website,
        postal_code: b.postal_code,
        city: b.city,
        address_line: b.address_line,
        notes: b.notes,
        role: b.role,
    })?;
    let mut tx = state.db.begin().await?;
    let partner = partners::insert(&mut *tx, &input).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "partner",
        partner.id,
        "create",
        json!({ "name": partner.name }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(partner)))
}

#[derive(Serialize, ToSchema)]
struct PartnerDetail {
    partner: Partner,
    contacts: Vec<Contact>,
    orders: Vec<OrderSummary>,
    /// Every enquiry from this partner, quoted or not. Without it, someone opening a
    /// partner cannot see they were quoted eight months ago and never followed up.
    leads: Vec<LeadSummary>,
}

#[utoipa::path(
    get, path = "/partners/{id}", tag = "partners",
    params(("id" = i64, Path)),
    responses((status = 200, body = PartnerDetail))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<PartnerDetail>> {
    let partner = partners::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let contacts = contacts::list_for_partner(&state.db, id, true).await?;
    let filter = OrderFilter {
        partner_id: Some(id),
        ..Default::default()
    };
    let orders = orders::search(&state.db, &filter, orders::DEFAULT_SORT, 100, 0).await?;
    let leads = leads::for_partner(&state.db, id).await?;
    Ok(Json(PartnerDetail {
        partner,
        contacts,
        orders,
        leads,
    }))
}

#[derive(Deserialize, ToSchema)]
struct PatchPartner {
    kind: Option<PartnerKind>,
    name: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    tax_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    eu_tax_number: Option<Option<String>>,
    country: Option<String>,
    default_currency: Option<Currency>,
    #[serde(default, deserialize_with = "patch_field")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    website: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    role: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    postal_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    city: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    address_line: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
}

#[utoipa::path(
    patch, path = "/partners/{id}", tag = "partners",
    params(("id" = i64, Path)),
    request_body = PatchPartner,
    responses((status = 200, body = Partner))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchPartner>,
) -> AppResult<Json<Partner>> {
    me.require(Capability::EditPartners)?;
    let mut tx = state.db.begin().await?;
    let current = partners::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let current_currency: Currency = current
        .default_currency
        .parse()
        .map_err(|e| AppError::internal(format!("{e}")))?;
    let input = validate(PartnerDraft {
        kind: p.kind.unwrap_or(current.kind),
        name: p.name.unwrap_or_else(|| current.name.clone()),
        tax_number: patch_text(&current.tax_number, p.tax_number),
        eu_tax_number: patch_text(&current.eu_tax_number, p.eu_tax_number),
        country: p.country.unwrap_or_else(|| current.country.clone()),
        default_currency: p.default_currency.unwrap_or(current_currency),
        email: patch_text(&current.email, p.email),
        phone: patch_text(&current.phone, p.phone),
        website: patch_text(&current.website, p.website),
        postal_code: patch_text(&current.postal_code, p.postal_code),
        city: patch_text(&current.city, p.city),
        address_line: patch_text(&current.address_line, p.address_line),
        notes: patch_text(&current.notes, p.notes),
        role: patch_text(&current.role, p.role),
    })?;
    let updated = partners::update(&mut *tx, id, &input)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let changes = audit::diff(&[
        ("kind", json!(current.kind), json!(updated.kind)),
        ("name", json!(current.name), json!(updated.name)),
        (
            "tax_number",
            json!(current.tax_number),
            json!(updated.tax_number),
        ),
        (
            "eu_tax_number",
            json!(current.eu_tax_number),
            json!(updated.eu_tax_number),
        ),
        ("country", json!(current.country), json!(updated.country)),
        (
            "default_currency",
            json!(current.default_currency),
            json!(updated.default_currency),
        ),
        ("email", json!(current.email), json!(updated.email)),
        ("phone", json!(current.phone), json!(updated.phone)),
        ("website", json!(current.website), json!(updated.website)),
        (
            "postal_code",
            json!(current.postal_code),
            json!(updated.postal_code),
        ),
        ("city", json!(current.city), json!(updated.city)),
        (
            "address_line",
            json!(current.address_line),
            json!(updated.address_line),
        ),
        ("notes", json!(current.notes), json!(updated.notes)),
    ]);
    audit::record(&mut *tx, Some(me.user_id), "partner", id, "update", changes).await?;
    tx.commit().await?;
    Ok(Json(updated))
}

async fn set_archived(
    state: &AppState,
    me: &crate::service::auth::AuthUser,
    id: i64,
    archived: bool,
) -> AppResult<StatusCode> {
    me.require(Capability::EditPartners)?;
    let mut tx = state.db.begin().await?;
    if !partners::set_archived(&mut *tx, id, archived).await? {
        return Err(AppError::NotFound("partner"));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "partner",
        id,
        if archived { "archive" } else { "unarchive" },
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post, path = "/partners/{id}/archive", tag = "partners",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Archived"))
)]
async fn archive(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    set_archived(&state, &me, id, true).await
}

#[utoipa::path(
    post, path = "/partners/{id}/unarchive", tag = "partners",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Restored"))
)]
async fn unarchive(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    set_archived(&state, &me, id, false).await
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ContactsQuery {
    #[serde(default)]
    include_archived: bool,
}

#[utoipa::path(
    get, path = "/partners/{id}/contacts", tag = "partners",
    params(("id" = i64, Path), ContactsQuery),
    responses((status = 200, body = Items<Contact>))
)]
async fn list_contacts(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiQuery(q): ApiQuery<ContactsQuery>,
) -> AppResult<Json<Items<Contact>>> {
    Ok(Items::new(
        contacts::list_for_partner(&state.db, id, q.include_archived).await?,
    ))
}

/// Create requires `name`. On PATCH every field is optional; `null` clears.
#[derive(Deserialize, ToSchema)]
struct ContactBody {
    name: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    position: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
}

fn merge_contact(current: Option<&Contact>, b: ContactBody) -> AppResult<ContactInput> {
    let name = b
        .name
        .or_else(|| current.map(|c| c.name.clone()))
        .unwrap_or_default();
    let email = patch_text(&current.and_then(|c| c.email.clone()), b.email);
    let email = match email {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("email is not a valid address"))?,
        ),
        None => None,
    };
    Ok(ContactInput {
        name: required("name", &name)?,
        email,
        phone: patch_text(&current.and_then(|c| c.phone.clone()), b.phone),
        position: patch_text(&current.and_then(|c| c.position.clone()), b.position),
        notes: patch_text(&current.and_then(|c| c.notes.clone()), b.notes),
    })
}

#[utoipa::path(
    post, path = "/partners/{id}/contacts", tag = "partners",
    params(("id" = i64, Path)),
    request_body = ContactBody,
    responses((status = 201, body = Contact))
)]
async fn create_contact(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(partner_id): ApiPath<i64>,
    ApiJson(b): ApiJson<ContactBody>,
) -> AppResult<(StatusCode, Json<Contact>)> {
    me.require(Capability::EditPartners)?;
    let input = merge_contact(None, b)?;
    let mut tx = state.db.begin().await?;
    partners::find(&mut *tx, partner_id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let contact = contacts::insert(&mut *tx, partner_id, &input).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "contact",
        contact.id,
        "create",
        json!({ "partner_id": partner_id, "name": contact.name }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(contact)))
}

#[utoipa::path(
    patch, path = "/contacts/{id}", tag = "partners",
    params(("id" = i64, Path)),
    request_body = ContactBody,
    responses((status = 200, body = Contact))
)]
async fn update_contact(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ContactBody>,
) -> AppResult<Json<Contact>> {
    me.require(Capability::EditPartners)?;
    let mut tx = state.db.begin().await?;
    let current = contacts::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("contact"))?;
    let input = merge_contact(Some(&current), b)?;
    let updated = contacts::update(&mut *tx, id, &input)
        .await?
        .ok_or(AppError::NotFound("contact"))?;
    let changes = audit::diff(&[
        ("name", json!(current.name), json!(updated.name)),
        ("email", json!(current.email), json!(updated.email)),
        ("phone", json!(current.phone), json!(updated.phone)),
        ("position", json!(current.position), json!(updated.position)),
        ("notes", json!(current.notes), json!(updated.notes)),
    ]);
    audit::record(&mut *tx, Some(me.user_id), "contact", id, "update", changes).await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[utoipa::path(
    post, path = "/contacts/{id}/archive", tag = "partners",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Archived"))
)]
async fn archive_contact(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditPartners)?;
    let mut tx = state.db.begin().await?;
    if !contacts::set_archived(&mut *tx, id, true).await? {
        return Err(AppError::NotFound("contact"));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "contact",
        id,
        "archive",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
