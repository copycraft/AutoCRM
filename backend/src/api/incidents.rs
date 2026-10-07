//! The internal incident log (0048). A damage judged "new" at kiadás (or found any other
//! way) becomes an incident: what happened, who answers for it, what it cost, how it was
//! settled. A rework job can be opened from it, linked both ways. Customers never see it.
//!
//! Logging is shop-floor work (`ChangeStages`); opening a rework job creates an order,
//! which is office work (`EditOrders`).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, page_limit, page_offset};
use crate::AppState;
use crate::domain::money::Currency;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::incidents::{self, Incident, IncidentUpdate, NewIncident};
use crate::repo::orders::OrderFields;
use crate::repo::{audit, inspections, orders};
use crate::service;
use crate::service::auth::AuthUser;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(detail, update))
        .routes(routes!(open_rework))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// `open` | `resolved`.
    status: Option<String>,
    /// Incidents of this order, or whose rework job it is.
    order_id: Option<i64>,
    inspection_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/incidents", tag = "incidents",
    params(ListQuery),
    responses((status = 200, body = Items<Incident>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<Incident>>> {
    if let Some(s) = q.status.as_deref()
        && s != "open"
        && s != "resolved"
    {
        return Err(AppError::validation("status must be open or resolved"));
    }
    Ok(Items::new(
        incidents::list(
            &state.db,
            q.status.as_deref(),
            q.order_id,
            q.inspection_id,
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[utoipa::path(
    get, path = "/incidents/{id}", tag = "incidents",
    params(("id" = i64, Path)),
    responses((status = 200, body = Incident))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Incident>> {
    Ok(Json(
        incidents::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("incident"))?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct IncidentBody {
    order_id: i64,
    /// The kiadás damage it is about. Its inspection must belong to the order.
    damage_id: Option<i64>,
    title: String,
    description: Option<String>,
    /// What it cost to put right, in minor units.
    cost_minor: Option<i64>,
    currency: Option<Currency>,
    /// Who caused it or answers for it, in words.
    responsible: Option<String>,
    /// Also open a rework job for it (needs `EditOrders`).
    #[serde(default)]
    open_rework: bool,
}

fn check_cost(cost: Option<i64>) -> AppResult<()> {
    if cost.is_some_and(|c| c < 0) {
        return Err(AppError::validation("cost_minor cannot be negative"));
    }
    Ok(())
}

/// The rework job: the same customer, vehicle and kind of work, linked to the job it fixes.
async fn create_rework(
    conn: &mut sqlx::PgConnection,
    state: &AppState,
    me: &AuthUser,
    incident_id: i64,
    order_id: i64,
    title: &str,
    description: Option<&str>,
) -> AppResult<i64> {
    let original = orders::find(&mut *conn, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let today = service::business_today(state.config.business_tz);
    let fields = OrderFields {
        title: format!("Javítás: {title}"),
        partner_id: original.partner_id,
        contact_id: original.contact_id,
        project_type_id: original.project_type_id,
        currency: original.currency.clone(),
        valuation_date: today,
        vehicle_make: original.vehicle_make.clone(),
        vehicle_model: original.vehicle_model.clone(),
        vehicle_plate: original.vehicle_plate.clone(),
        vehicle_vin: original.vehicle_vin.clone(),
        description: description.map(str::to_string),
        due_date: None,
        assigned_to: None,
        related_order_id: Some(original.id),
        relation: Some("rework".into()),
        mileage_in: None,
        intake_condition: None,
        fuel_level: None,
        key_count: None,
        valuables_declared: None,
        valuables: None,
    };
    let rework =
        service::orders::create_in_tx(&mut *conn, me.user_id, today, fields, Vec::new(), None)
            .await?;
    incidents::set_rework(&mut *conn, incident_id, rework.id).await?;
    audit::record(
        &mut *conn,
        Some(me.user_id),
        "order",
        order_id,
        "incident_rework",
        json!({ "incident_id": incident_id, "rework_order_id": rework.id, "number": rework.number }),
    )
    .await?;
    Ok(rework.id)
}

#[utoipa::path(
    post, path = "/incidents", tag = "incidents",
    request_body = IncidentBody,
    responses((status = 201, body = Incident))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<IncidentBody>,
) -> AppResult<(StatusCode, Json<Incident>)> {
    me.require(Capability::ChangeStages)?;
    if b.open_rework {
        me.require(Capability::EditOrders)?;
    }
    let order = orders::find(&state.db, b.order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let title = super::required("title", &b.title)?;
    check_cost(b.cost_minor)?;
    let inspection_id = match b.damage_id {
        Some(damage_id) => {
            let (inspection_id, damage_order, _) = inspections::damage_owner(&state.db, damage_id)
                .await?
                .ok_or(AppError::NotFound("damage"))?;
            if damage_order != order.id {
                return Err(AppError::validation("the damage belongs to another order"));
            }
            // The second click opens the first.
            if let Some(existing) = incidents::find_by_damage(&state.db, damage_id).await? {
                let found = incidents::find(&state.db, existing)
                    .await?
                    .ok_or(AppError::NotFound("incident"))?;
                return Ok((StatusCode::OK, Json(found)));
            }
            Some(inspection_id)
        }
        None => None,
    };
    let description = super::optional(b.description);
    let responsible = super::optional(b.responsible);
    let currency = b
        .currency
        .map(|c| c.code().to_string())
        .unwrap_or_else(|| order.currency.clone());
    let mut tx = state.db.begin().await?;
    let id = incidents::insert(
        &mut *tx,
        &NewIncident {
            order_id: order.id,
            inspection_id,
            damage_id: b.damage_id,
            title: &title,
            description: description.as_deref(),
            cost_minor: b.cost_minor,
            currency: &currency,
            responsible: responsible.as_deref(),
            created_by: me.user_id,
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        order.id,
        "incident",
        json!({ "incident_id": id, "title": title, "damage_id": b.damage_id }),
    )
    .await?;
    if b.open_rework {
        create_rework(
            &mut tx,
            &state,
            &me,
            id,
            order.id,
            &title,
            description.as_deref(),
        )
        .await?;
    }
    tx.commit().await?;
    let created = incidents::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incident"))?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[derive(Deserialize, ToSchema)]
struct IncidentPatch {
    title: Option<String>,
    #[serde(default, deserialize_with = "super::patch")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "super::patch")]
    cost_minor: Option<Option<i64>>,
    currency: Option<Currency>,
    #[serde(default, deserialize_with = "super::patch")]
    responsible: Option<Option<String>>,
    /// `open` | `resolved`.
    status: Option<String>,
    #[serde(default, deserialize_with = "super::patch")]
    resolution: Option<Option<String>>,
}

#[utoipa::path(
    patch, path = "/incidents/{id}", tag = "incidents",
    params(("id" = i64, Path)),
    request_body = IncidentPatch,
    responses((status = 200, body = Incident))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<IncidentPatch>,
) -> AppResult<Json<Incident>> {
    me.require(Capability::ChangeStages)?;
    let current = incidents::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incident"))?;
    let resolved = match b.status.as_deref() {
        None => current.status == "resolved",
        Some("open") => false,
        Some("resolved") => true,
        Some(_) => return Err(AppError::validation("status must be open or resolved")),
    };
    let cost_minor = b.cost_minor.unwrap_or(current.cost_minor);
    check_cost(cost_minor)?;
    let update = IncidentUpdate {
        title: match b.title {
            Some(t) => super::required("title", &t)?,
            None => current.title.clone(),
        },
        description: super::patch_text(&current.description, b.description),
        cost_minor,
        currency: b
            .currency
            .map(|c| c.code().to_string())
            .unwrap_or(current.currency.clone()),
        responsible: super::patch_text(&current.responsible, b.responsible),
        resolved,
        resolution: super::patch_text(&current.resolution, b.resolution),
    };
    let mut tx = state.db.begin().await?;
    incidents::update(&mut *tx, id, &update, me.user_id).await?;
    if resolved != (current.status == "resolved") {
        audit::record(
            &mut *tx,
            Some(me.user_id),
            "order",
            current.order_id,
            if resolved {
                "incident_resolved"
            } else {
                "incident_reopened"
            },
            json!({ "incident_id": id, "title": update.title }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(
        incidents::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("incident"))?,
    ))
}

/// Opens the rework job for an incident that has none yet.
#[utoipa::path(
    post, path = "/incidents/{id}/rework", tag = "incidents",
    params(("id" = i64, Path)),
    responses((status = 201, body = Incident))
)]
async fn open_rework(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<(StatusCode, Json<Incident>)> {
    me.require(Capability::EditOrders)?;
    let current = incidents::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incident"))?;
    if current.rework_order_id.is_some() {
        return Ok((StatusCode::OK, Json(current)));
    }
    let mut tx = state.db.begin().await?;
    create_rework(
        &mut tx,
        &state,
        &me,
        id,
        current.order_id,
        &current.title,
        current.description.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(
            incidents::find(&state.db, id)
                .await?
                .ok_or(AppError::NotFound("incident"))?,
        ),
    ))
}
