use std::collections::HashMap;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::leads::StageBody;
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::media::ImageCategory;
use crate::domain::money::{Currency, Money};
use crate::domain::order::{normalize_plate, validate_line_item};
use crate::domain::role::Capability;
use crate::domain::stage::{StageEntity, find as find_stage};
use crate::error::{AppError, AppResult};
use crate::repo::audit::AuditEntry;
use crate::repo::blockers::Blocker;
use crate::repo::order_items::OrderItem;
use crate::repo::order_notes::OrderNote;
use crate::repo::order_specs::{OrderSpec, SpecFields};
use crate::repo::orders::{Order, OrderFields, OrderFilter, OrderSummary, OrderValue};
use crate::repo::stages::StageEntry;
use crate::repo::vehicles::{self, Vehicle};
use crate::repo::{
    audit, blockers, config, images, like_pattern, order_items, order_notes, order_specs, orders,
    parse_sort, partners, stages,
};
use crate::service;
use crate::service::orders::{
    NewItem, create_in_tx, items_total, order_currency, validate_references,
};
use crate::service::stages::{StageChange, TransitionOption};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(search, create))
        .routes(routes!(detail, update))
        .routes(routes!(change_stage))
        .routes(routes!(transitions))
        .routes(routes!(stage_history))
        .routes(routes!(audit_trail))
        .routes(routes!(notes))
        .routes(routes!(get_spec, put_spec))
        .routes(routes!(list_items, add_item))
        .routes(routes!(update_item, delete_item))
}

/// Quantities arrive as strings ("1.5") for exactness; plain JSON numbers are accepted too.
pub fn decimal_str_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<Decimal, D::Error> {
    use serde::de::Error;
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::String(s) => s.trim().parse().map_err(D::Error::custom),
        serde_json::Value::Number(n) => n.to_string().parse().map_err(D::Error::custom),
        _ => Err(D::Error::custom("expected a decimal number or string")),
    }
}

fn optional_decimal<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    decimal_str_or_number(d).map(Some)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    /// Matches number, title, partner, VIN, and plate ignoring spaces and dashes.
    q: Option<String>,
    /// Stage key.
    stage: Option<String>,
    partner_id: Option<i64>,
    project_type_id: Option<i64>,
    assigned_to: Option<i64>,
    /// Only orders not in a terminal stage.
    #[serde(default)]
    open: bool,
    /// Sort key, `-` prefix for descending: created_at, due_date, total, number,
    /// stage_entered_at (when the order entered its current stage).
    sort: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/orders", tag = "orders",
    params(SearchQuery),
    responses((status = 200, body = Items<OrderSummary>))
)]
async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<OrderSummary>>> {
    let plate = q.q.as_deref().map(normalize_plate).unwrap_or_default();
    let filter = OrderFilter {
        pattern: q.q.as_deref().and_then(like_pattern),
        plate_pattern: if plate.is_empty() {
            String::new()
        } else {
            format!("%{plate}%")
        },
        stage_key: optional(q.stage),
        partner_id: q.partner_id,
        project_type_id: q.project_type_id,
        assigned_to: q.assigned_to,
        open_only: q.open,
    };
    let sort_key = parse_sort(q.sort.as_deref(), orders::ORDER_SORTS, orders::DEFAULT_SORT)
        .map_err(AppError::validation)?;
    Ok(Items::new(
        orders::search(
            &state.db,
            &filter,
            &sort_key,
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
pub struct ItemBody {
    pub description: String,
    /// Decimal string, e.g. `"2.5"`.
    #[serde(deserialize_with = "decimal_str_or_number")]
    #[schema(value_type = String)]
    pub quantity: Decimal,
    /// Minor units (fillér / eurocent). May be negative for discount lines.
    pub unit_price: i64,
}

#[derive(Deserialize, ToSchema)]
pub struct OrderBody {
    pub title: Option<String>,
    pub partner_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub project_type_id: Option<i64>,
    pub currency: Currency,
    /// Defaults to today. Reports normalise the order's value at this date's MNB rate.
    pub valuation_date: Option<NaiveDate>,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_vin: Option<String>,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub assigned_to: Option<i64>,
    /// V2.2: the job this one repairs or repeats. Set with `relation` or not at all.
    pub related_order_id: Option<i64>,
    /// `warranty` | `rework` | `repeat`.
    pub relation: Option<String>,
    /// Intake slip: odometer at takeover (km). Leaving `intake` requires it.
    pub mileage_in: Option<i32>,
    /// Intake slip: visible condition notes at takeover.
    pub intake_condition: Option<String>,
    /// Intake slip: fuel gauge at takeover — `E`, `1/4`, `1/2`, `3/4` or `F`.
    pub fuel_level: Option<String>,
    /// Intake slip: how many keys were handed over.
    pub key_count: Option<i32>,
    /// Intake slip: whether anyone answered the valuables question. Absent means nobody
    /// did, which is not the same as `false` — "we never asked" is not a defence.
    pub valuables_declared: Option<bool>,
    /// Intake slip: what was left in the vehicle. Only meaningful with
    /// `valuables_declared = true`, and rejected without it.
    pub valuables: Option<String>,
    /// The build specification, when the chosen project type asks for one. Written in the
    /// same transaction as the order so a failed second request cannot leave a build with
    /// no spec.
    pub spec: Option<SpecBody>,
    #[serde(default)]
    pub items: Vec<ItemBody>,
}

/// The build spec as the form sends it. `form` is not taken on trust: it must match what
/// the order's project type asks for, or the row would mean something the office did not
/// choose.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SpecBody {
    pub target_temp_c: Option<String>,
    pub insulation_mm: Option<i32>,
    pub cooling_unit_make: Option<String>,
    pub cooling_unit_model: Option<String>,
    pub atp_class: Option<String>,
    pub compartments: Option<i32>,
    /// `automatic` | `manual` | `hot_gas`.
    pub defrost: Option<String>,
    pub electric_standby: Option<bool>,
    pub heater_make: Option<String>,
    pub heater_model: Option<String>,
    pub heat_output_kw: Option<String>,
    /// `diesel` | `electric` | `lpg` | `engine_coolant`.
    pub fuel: Option<String>,
    pub thermostat: Option<bool>,
    pub notes: Option<String>,
}

fn decimal_field(name: &str, value: Option<String>) -> AppResult<Option<Decimal>> {
    match optional(value) {
        // Hungarian keyboards produce a comma; refusing it would be a UI bug, not a rule.
        Some(s) => Ok(Some(s.replace(',', ".").parse::<Decimal>().map_err(
            |_| AppError::validation(format!("{name} must be a number")),
        )?)),
        None => Ok(None),
    }
}

fn spec_fields(form: &str, b: SpecBody) -> AppResult<SpecFields> {
    if let Some(d) = optional(b.defrost.clone())
        && !matches!(d.as_str(), "automatic" | "manual" | "hot_gas")
    {
        return Err(AppError::validation(
            "defrost must be automatic, manual or hot_gas",
        ));
    }
    if let Some(f) = optional(b.fuel.clone())
        && !matches!(f.as_str(), "diesel" | "electric" | "lpg" | "engine_coolant")
    {
        return Err(AppError::validation(
            "fuel must be diesel, electric, lpg or engine_coolant",
        ));
    }
    Ok(SpecFields {
        form: form.to_string(),
        target_temp_c: decimal_field("target_temp_c", b.target_temp_c)?,
        insulation_mm: b.insulation_mm,
        cooling_unit_make: optional(b.cooling_unit_make),
        cooling_unit_model: optional(b.cooling_unit_model),
        atp_class: optional(b.atp_class).map(|s| s.to_uppercase()),
        compartments: b.compartments,
        defrost: optional(b.defrost),
        electric_standby: b.electric_standby,
        heater_make: optional(b.heater_make),
        heater_model: optional(b.heater_model),
        heat_output_kw: decimal_field("heat_output_kw", b.heat_output_kw)?,
        fuel: optional(b.fuel),
        thermostat: b.thermostat,
        notes: optional(b.notes),
    }
    .for_form())
}

/// The spec form this order's project type asks for. `None` means the type has no build
/// spec (a repair), and a spec sent for it is a validation error rather than a silent
/// write nobody can see.
async fn required_form(
    conn: &mut sqlx::PgConnection,
    project_type_id: Option<i64>,
) -> AppResult<Option<String>> {
    match project_type_id {
        Some(id) => Ok(order_specs::form_for_project_type(&mut *conn, id).await?),
        None => Ok(None),
    }
}

/// The five marks on a fuel gauge, matching the CHECK in 0019_intake_extras.sql. A value
/// the database would reject should come back as a field error, not a 500.
const FUEL_LEVELS: [&str; 5] = ["E", "1/4", "1/2", "3/4", "F"];

fn fuel_level(value: Option<String>) -> AppResult<Option<String>> {
    match optional(value) {
        Some(v) if !FUEL_LEVELS.contains(&v.as_str()) => Err(AppError::validation(
            "fuel_level must be one of E, 1/4, 1/2, 3/4, F",
        )),
        v => Ok(v),
    }
}

/// A description of what is in the car only means something next to a tick saying there
/// is something in the car; the database enforces the same pairing.
fn valuables(
    declared: Option<bool>,
    text: Option<String>,
) -> AppResult<(Option<bool>, Option<String>)> {
    let text = optional(text);
    if text.is_some() && declared != Some(true) {
        return Err(AppError::validation(
            "valuables requires valuables_declared = true",
        ));
    }
    Ok((declared, text))
}

/// Keys are counted, so zero is a real answer and a negative one is a typo.
fn key_count(value: Option<i32>) -> AppResult<Option<i32>> {
    match value {
        Some(k) if k < 0 => Err(AppError::validation("key_count cannot be negative")),
        k => Ok(k),
    }
}

pub fn fields_from_body(
    b: OrderBody,
    partner_id: i64,
    today: NaiveDate,
    title_fallback: Option<String>,
) -> AppResult<(OrderFields, Vec<NewItem>)> {
    let title = optional(b.title).or(title_fallback).unwrap_or_default();
    let valuables_pair = valuables(b.valuables_declared, b.valuables)?;
    let fields = OrderFields {
        title: required("title", &title)?,
        partner_id,
        contact_id: b.contact_id,
        project_type_id: b.project_type_id,
        currency: b.currency.code().to_string(),
        valuation_date: b.valuation_date.unwrap_or(today),
        vehicle_make: optional(b.vehicle_make),
        vehicle_model: optional(b.vehicle_model),
        vehicle_plate: optional(b.vehicle_plate).map(|p| p.to_uppercase()),
        vehicle_vin: optional(b.vehicle_vin).map(|v| v.to_uppercase()),
        description: optional(b.description),
        due_date: b.due_date,
        assigned_to: b.assigned_to,
        related_order_id: b.related_order_id,
        relation: optional(b.relation),
        mileage_in: match b.mileage_in {
            Some(m) if m < 0 => return Err(AppError::validation("mileage_in cannot be negative")),
            m => m,
        },
        intake_condition: optional(b.intake_condition),
        fuel_level: fuel_level(b.fuel_level)?,
        key_count: key_count(b.key_count)?,
        valuables_declared: valuables_pair.0,
        valuables: valuables_pair.1,
    };
    let items = b
        .items
        .into_iter()
        .map(|i| NewItem {
            description: i.description,
            quantity: i.quantity,
            unit_price: i.unit_price,
        })
        .collect();
    Ok((fields, items))
}

#[utoipa::path(
    post, path = "/orders", tag = "orders",
    request_body(content = OrderBody, description = "`partner_id` and `title` are required."),
    responses((status = 201, body = Order))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<OrderBody>,
) -> AppResult<(StatusCode, Json<Order>)> {
    me.require(Capability::EditOrders)?;
    let partner_id = b
        .partner_id
        .ok_or_else(|| AppError::validation("partner_id is required"))?;
    let today = service::business_today(state.config.business_tz);
    let spec = b.spec.clone();
    let project_type_id = b.project_type_id;
    let (fields, items) = fields_from_body(b, partner_id, today, None)?;
    let mut tx = state.db.begin().await?;
    let order = create_in_tx(&mut tx, me.user_id, today, fields, items, None).await?;
    if let Some(spec) = spec {
        let form = required_form(&mut tx, project_type_id)
            .await?
            .ok_or_else(|| AppError::validation("this project type has no build specification"))?;
        let written = order_specs::upsert(&mut *tx, order.id, &spec_fields(&form, spec)?).await?;
        audit::record(
            &mut *tx,
            Some(me.user_id),
            "order",
            order.id,
            "spec_set",
            json!({ "form": written.form }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(order)))
}

#[derive(Serialize, ToSchema)]
struct PartnerRef {
    id: i64,
    name: String,
}

#[derive(Serialize, ToSchema)]
struct StageView {
    key: String,
    label_hu: String,
    entered_at: chrono::DateTime<Utc>,
    days_in_stage: i64,
    is_terminal: bool,
}

#[derive(Serialize, ToSchema)]
pub struct ItemView {
    #[serde(flatten)]
    item: OrderItem,
    /// Backend-computed: round(quantity × unit_price), half away from zero.
    line_total_minor: i64,
}

#[derive(Serialize, ToSchema)]
struct RelatedOrder {
    id: i64,
    number: String,
    title: String,
    /// 'warranty' | 'rework' | 'repeat'.
    relation: String,
}

#[derive(Serialize, ToSchema)]
struct OrderDetail {
    order: Order,
    partner: PartnerRef,
    stage: StageView,
    items: Vec<ItemView>,
    value: OrderValue,
    blockers: Vec<Blocker>,
    /// V2.2: the job this one repairs or repeats, resolved for display.
    related: Option<RelatedOrder>,
    /// V2.1: the vans this job covers. Usually one; three identical Sprinters from one
    /// enquiry is the case the join table exists for.
    vehicles: Vec<Vehicle>,
    /// The build specification, when the project type asks for one.
    spec: Option<OrderSpec>,
    /// Image count per category; categories without images are absent.
    #[schema(value_type = HashMap<String, i64>)]
    image_counts: HashMap<ImageCategory, i64>,
}

fn item_view(item: OrderItem) -> AppResult<ItemView> {
    let currency: Currency = item
        .currency
        .parse()
        .map_err(|e| AppError::internal(format!("{e}")))?;
    let line = Money::new(item.unit_price, currency)
        .times_quantity(item.quantity)
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(ItemView {
        line_total_minor: line.minor(),
        item,
    })
}

#[utoipa::path(
    get, path = "/orders/{id}", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, body = OrderDetail))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<OrderDetail>> {
    let order = orders::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let partner = partners::find(&state.db, order.partner_id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let current = stages::current_order_stage(&state.db, id)
        .await?
        .ok_or_else(|| AppError::internal(format!("order {id} has no stage history")))?;
    let definitions = config::stage_definitions(&state.db, StageEntity::Order).await?;
    let definition = find_stage(&definitions, &current.stage_key);
    let items = order_items::list(&state.db, id).await?;
    // Cross-check: the Money-based total must agree with the SQL view reports use.
    let total = items_total(order_currency(&order)?, &items)?;
    let value = orders::value(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    if value.total_minor != total.minor() {
        tracing::error!(
            order_id = id,
            sql = value.total_minor,
            domain = total.minor(),
            "order total mismatch between SQL and Money"
        );
    }

    Ok(Json(OrderDetail {
        partner: PartnerRef {
            id: partner.id,
            name: partner.name,
        },
        stage: StageView {
            label_hu: definition
                .map(|d| d.label_hu.clone())
                .unwrap_or_else(|| current.stage_key.clone()),
            is_terminal: definition.is_some_and(|d| d.is_terminal),
            days_in_stage: (Utc::now() - current.entered_at).num_days(),
            entered_at: current.entered_at,
            key: current.stage_key,
        },
        items: items.into_iter().map(item_view).collect::<AppResult<_>>()?,
        value,
        blockers: blockers::list_for_order(
            &state.db,
            id,
            service::business_today(state.config.business_tz),
        )
        .await?,
        image_counts: images::count_by_category(&state.db, id).await?,
        related: match (order.related_order_id, order.relation.clone()) {
            (Some(related_id), Some(relation)) => {
                orders::find(&state.db, related_id)
                    .await?
                    .map(|r| RelatedOrder {
                        id: r.id,
                        number: r.number,
                        title: r.title,
                        relation,
                    })
            }
            _ => None,
        },
        vehicles: vehicles::list_for_order(&state.db, id).await?,
        spec: order_specs::find(&state.db, id).await?,
        order,
    }))
}

#[derive(Deserialize, ToSchema)]
struct PatchOrder {
    title: Option<String>,
    /// Changing partner without naming `contact_id` clears the contact.
    partner_id: Option<i64>,
    #[serde(default, deserialize_with = "patch_field")]
    contact_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    project_type_id: Option<Option<i64>>,
    /// Rejected with `422 currency_locked` while the order has line items.
    currency: Option<Currency>,
    valuation_date: Option<NaiveDate>,
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_make: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_model: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_plate: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_vin: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    due_date: Option<Option<NaiveDate>>,
    #[serde(default, deserialize_with = "patch_field")]
    assigned_to: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    related_order_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    relation: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    mileage_in: Option<Option<i32>>,
    #[serde(default, deserialize_with = "patch_field")]
    intake_condition: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    fuel_level: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    key_count: Option<Option<i32>>,
    #[serde(default, deserialize_with = "patch_field")]
    valuables_declared: Option<Option<bool>>,
    #[serde(default, deserialize_with = "patch_field")]
    valuables: Option<Option<String>>,
}

#[utoipa::path(
    patch, path = "/orders/{id}", tag = "orders",
    params(("id" = i64, Path)),
    request_body = PatchOrder,
    responses((status = 200, body = Order))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchOrder>,
) -> AppResult<Json<Order>> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let current = orders::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;

    let partner_changed = p.partner_id.is_some_and(|pid| pid != current.partner_id);
    // Resolved together: unticking the box in the same request that leaves the old
    // description behind would otherwise trip the database's CHECK as a 500.
    let valuables_pair = valuables(
        p.valuables_declared.unwrap_or(current.valuables_declared),
        patch_text(&current.valuables, p.valuables),
    )?;
    let title = p.title.unwrap_or_else(|| current.title.clone());
    let fields = OrderFields {
        title: required("title", &title)?,
        partner_id: p.partner_id.unwrap_or(current.partner_id),
        // A contact belongs to a partner: changing partner without naming a contact clears it.
        contact_id: match p.contact_id {
            Some(v) => v,
            None if partner_changed => None,
            None => current.contact_id,
        },
        project_type_id: p.project_type_id.unwrap_or(current.project_type_id),
        currency: p
            .currency
            .map(|c| c.code().to_string())
            .unwrap_or_else(|| current.currency.clone()),
        valuation_date: p.valuation_date.unwrap_or(current.valuation_date),
        vehicle_make: patch_text(&current.vehicle_make, p.vehicle_make),
        vehicle_model: patch_text(&current.vehicle_model, p.vehicle_model),
        vehicle_plate: patch_text(&current.vehicle_plate, p.vehicle_plate)
            .map(|v| v.to_uppercase()),
        vehicle_vin: patch_text(&current.vehicle_vin, p.vehicle_vin).map(|v| v.to_uppercase()),
        description: patch_text(&current.description, p.description),
        due_date: p.due_date.unwrap_or(current.due_date),
        assigned_to: p.assigned_to.unwrap_or(current.assigned_to),
        related_order_id: p.related_order_id.unwrap_or(current.related_order_id),
        relation: patch_text(&current.relation, p.relation),
        mileage_in: match p.mileage_in {
            Some(Some(m)) if m < 0 => {
                return Err(AppError::validation("mileage_in cannot be negative"));
            }
            Some(v) => v,
            None => current.mileage_in,
        },
        intake_condition: patch_text(&current.intake_condition, p.intake_condition),
        fuel_level: fuel_level(patch_text(&current.fuel_level, p.fuel_level))?,
        key_count: key_count(p.key_count.unwrap_or(current.key_count))?,
        valuables_declared: valuables_pair.0,
        valuables: valuables_pair.1,
    };
    if fields.currency != current.currency && order_items::count(&mut *tx, id).await? > 0 {
        return Err(AppError::rule(
            "currency_locked",
            "an order's currency cannot change while it has line items; remove them first",
        ));
    }
    validate_references(&mut tx, &fields).await?;

    let updated = orders::update(&mut *tx, id, &fields)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    service::orders::sync_vehicle(&mut tx, id, &fields).await?;
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
            "project_type_id",
            json!(current.project_type_id),
            json!(updated.project_type_id),
        ),
        ("currency", json!(current.currency), json!(updated.currency)),
        (
            "valuation_date",
            json!(current.valuation_date),
            json!(updated.valuation_date),
        ),
        (
            "vehicle_make",
            json!(current.vehicle_make),
            json!(updated.vehicle_make),
        ),
        (
            "vehicle_model",
            json!(current.vehicle_model),
            json!(updated.vehicle_model),
        ),
        (
            "vehicle_plate",
            json!(current.vehicle_plate),
            json!(updated.vehicle_plate),
        ),
        (
            "vehicle_vin",
            json!(current.vehicle_vin),
            json!(updated.vehicle_vin),
        ),
        (
            "description",
            json!(current.description),
            json!(updated.description),
        ),
        ("due_date", json!(current.due_date), json!(updated.due_date)),
        (
            "assigned_to",
            json!(current.assigned_to),
            json!(updated.assigned_to),
        ),
        (
            "related_order_id",
            json!(current.related_order_id),
            json!(updated.related_order_id),
        ),
        ("relation", json!(current.relation), json!(updated.relation)),
        (
            "mileage_in",
            json!(current.mileage_in),
            json!(updated.mileage_in),
        ),
        (
            "intake_condition",
            json!(current.intake_condition),
            json!(updated.intake_condition),
        ),
        (
            "fuel_level",
            json!(current.fuel_level),
            json!(updated.fuel_level),
        ),
        (
            "key_count",
            json!(current.key_count),
            json!(updated.key_count),
        ),
        (
            "valuables_declared",
            json!(current.valuables_declared),
            json!(updated.valuables_declared),
        ),
        (
            "valuables",
            json!(current.valuables),
            json!(updated.valuables),
        ),
    ]);
    audit::record(&mut *tx, Some(me.user_id), "order", id, "update", changes).await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[utoipa::path(
    post, path = "/orders/{id}/stage", tag = "orders",
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
    me.require(Capability::ChangeStages)?;
    let note = optional(b.note);
    let change =
        service::stages::change_order_stage(&state, &me, id, b.stage.trim(), note.as_deref())
            .await?;
    Ok(Json(change))
}

#[utoipa::path(
    get, path = "/orders/{id}/transitions", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, description = "Manual stage targets with note and gate requirements", body = Items<TransitionOption>))
)]
async fn transitions(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<TransitionOption>>> {
    Ok(Items::new(
        service::stages::order_transitions(&state, id).await?,
    ))
}

#[utoipa::path(
    get, path = "/orders/{id}/stages", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, description = "Stage history, oldest first", body = Items<StageEntry>))
)]
async fn stage_history(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<StageEntry>>> {
    orders::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    Ok(Items::new(stages::order_history(&state.db, id).await?))
}

/// Imported MiniCRM activity (V1.3). Read-only, and empty for orders created in AutoCRM:
/// this is history, not a task list.
#[utoipa::path(
    get, path = "/orders/{id}/notes", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<OrderNote>))
)]
async fn notes(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<OrderNote>>> {
    Ok(Items::new(
        order_notes::list_for_order(&state.db, id).await?,
    ))
}

#[utoipa::path(
    get, path = "/orders/{id}/spec", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, body = OrderSpec), (status = 404))
)]
async fn get_spec(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<OrderSpec>> {
    order_specs::find(&state.db, id)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound("order spec"))
}

/// Replaces the build spec. The form comes from the order's project type, never from the
/// request: a cooling order cannot be given a heating spec by sending one.
#[utoipa::path(
    put, path = "/orders/{id}/spec", tag = "orders",
    params(("id" = i64, Path)),
    request_body = SpecBody,
    responses((status = 200, body = OrderSpec))
)]
async fn put_spec(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<SpecBody>,
) -> AppResult<Json<OrderSpec>> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let order = orders::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let form = required_form(&mut tx, order.project_type_id)
        .await?
        .ok_or_else(|| AppError::validation("this project type has no build specification"))?;
    let spec = order_specs::upsert(&mut *tx, id, &spec_fields(&form, b)?).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        id,
        "spec_set",
        json!({ "form": spec.form }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(spec))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AuditQuery {
    limit: Option<i64>,
}

#[utoipa::path(
    get, path = "/orders/{id}/audit", tag = "orders",
    params(("id" = i64, Path), AuditQuery),
    responses((status = 200, body = Items<AuditEntry>))
)]
async fn audit_trail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiQuery(q): ApiQuery<AuditQuery>,
) -> AppResult<Json<Items<AuditEntry>>> {
    Ok(Items::new(
        audit::list_for(&state.db, "order", id, page_limit(q.limit)).await?,
    ))
}

#[utoipa::path(
    get, path = "/orders/{id}/items", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<ItemView>))
)]
async fn list_items(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<ItemView>>> {
    let items = order_items::list(&state.db, id).await?;
    Ok(Items::new(
        items.into_iter().map(item_view).collect::<AppResult<_>>()?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct AddItem {
    description: String,
    /// Decimal string, e.g. `"2.5"`.
    #[serde(deserialize_with = "decimal_str_or_number")]
    #[schema(value_type = String)]
    quantity: Decimal,
    /// Minor units in the order's currency. May be negative for discount lines.
    unit_price: i64,
    position: Option<i32>,
}

#[utoipa::path(
    post, path = "/orders/{id}/items", tag = "orders",
    params(("id" = i64, Path)),
    request_body = AddItem,
    responses((status = 201, body = ItemView))
)]
async fn add_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiJson(b): ApiJson<AddItem>,
) -> AppResult<(StatusCode, Json<ItemView>)> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let order = orders::lock(&mut *tx, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let currency = order_currency(&order)?;
    validate_line_item(
        &b.description,
        b.quantity,
        Money::new(b.unit_price, currency),
        currency,
    )
    .map_err(|e| AppError::validation(e.to_string()))?;
    let position = match b.position {
        Some(p) => p,
        None => order_items::next_position(&mut *tx, order_id).await?,
    };
    let item = order_items::insert(
        &mut *tx,
        order_id,
        position,
        b.description.trim(),
        b.quantity,
        b.unit_price,
        currency.code(),
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        order_id,
        "item_add",
        json!({ "item_id": item.id, "description": item.description, "quantity": item.quantity.to_string(), "unit_price": item.unit_price }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(item_view(item)?)))
}

#[derive(Deserialize, ToSchema)]
struct PatchItem {
    description: Option<String>,
    /// Decimal string, e.g. `"2.5"`.
    #[serde(default, deserialize_with = "optional_decimal")]
    #[schema(value_type = Option<String>)]
    quantity: Option<Decimal>,
    unit_price: Option<i64>,
    position: Option<i32>,
}

#[utoipa::path(
    patch, path = "/order-items/{id}", tag = "orders",
    params(("id" = i64, Path)),
    request_body = PatchItem,
    responses((status = 200, body = ItemView))
)]
async fn update_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchItem>,
) -> AppResult<Json<ItemView>> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let peek = order_items::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("order item"))?;
    let order = orders::lock(&mut *tx, peek.order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let current = order_items::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("order item"))?;
    let currency = order_currency(&order)?;

    let description = p.description.unwrap_or_else(|| current.description.clone());
    let quantity = p.quantity.unwrap_or(current.quantity);
    let unit_price = p.unit_price.unwrap_or(current.unit_price);
    validate_line_item(
        &description,
        quantity,
        Money::new(unit_price, currency),
        currency,
    )
    .map_err(|e| AppError::validation(e.to_string()))?;
    let updated = order_items::update(
        &mut *tx,
        id,
        p.position.unwrap_or(current.position),
        description.trim(),
        quantity,
        unit_price,
    )
    .await?
    .ok_or(AppError::NotFound("order item"))?;
    let changes = audit::diff(&[
        (
            "description",
            json!(current.description),
            json!(updated.description),
        ),
        (
            "quantity",
            json!(current.quantity.to_string()),
            json!(updated.quantity.to_string()),
        ),
        (
            "unit_price",
            json!(current.unit_price),
            json!(updated.unit_price),
        ),
        ("position", json!(current.position), json!(updated.position)),
    ]);
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        order.id,
        "item_update",
        json!({ "item_id": id, "changes": changes }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(item_view(updated)?))
}

#[utoipa::path(
    delete, path = "/order-items/{id}", tag = "orders",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted"))
)]
async fn delete_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let item = order_items::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("order item"))?;
    orders::lock(&mut *tx, item.order_id).await?;
    order_items::delete(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        item.order_id,
        "item_delete",
        json!({ "item_id": id, "description": item.description, "quantity": item.quantity.to_string(), "unit_price": item.unit_price }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
