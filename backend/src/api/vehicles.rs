//! Vehicles (V2.1).
//!
//! Most vehicles are created implicitly: the four text fields on an order create or match
//! one (`service::orders::sync_vehicle`). These routes exist for the cases that cannot be
//! expressed that way — a job covering three identical Sprinters, correcting a plate typed
//! wrong years ago, and answering "has this van been here before".

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::orders;
use crate::repo::vehicles::{self, Vehicle, VehicleFields};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(search, create))
        .routes(routes!(detail, update))
        .routes(routes!(list_for_order, attach))
        .routes(routes!(detach))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SearchQuery {
    /// Plate (ignoring spaces, dashes and case), VIN, make, model or notes.
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/vehicles", tag = "vehicles",
    params(SearchQuery),
    responses((status = 200, body = Items<Vehicle>))
)]
async fn search(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> AppResult<Json<Items<Vehicle>>> {
    Ok(Items::new(
        vehicles::search(
            &state.db,
            q.q.as_deref(),
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct VehicleBody {
    vin: Option<String>,
    plate: Option<String>,
    make: Option<String>,
    model: Option<String>,
    year: Option<i32>,
    partner_id: Option<i64>,
    notes: Option<String>,
}

impl VehicleBody {
    fn fields(self) -> AppResult<VehicleFields> {
        let f = VehicleFields {
            vin: optional(self.vin).map(|v| v.to_uppercase()),
            plate: optional(self.plate).map(|p| p.to_uppercase()),
            make: optional(self.make),
            model: optional(self.model),
            year: self.year,
            partner_id: self.partner_id,
            notes: optional(self.notes),
        };
        if f.vin.is_none() && f.plate.is_none() {
            return Err(AppError::validation(
                "a vehicle needs a plate or a VIN: a make and model do not identify a van",
            ));
        }
        Ok(f)
    }
}

#[derive(Serialize, ToSchema)]
struct CreatedVehicle {
    #[serde(flatten)]
    vehicle: Vehicle,
    /// True when this plate or VIN already existed and the stored vehicle was reused
    /// rather than duplicated. The screen shows it as "this van has been here before",
    /// which for a warranty job is the most useful thing the system can say.
    existing: bool,
}

#[utoipa::path(
    post, path = "/vehicles", tag = "vehicles",
    request_body = VehicleBody,
    responses((status = 201, body = CreatedVehicle))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<VehicleBody>,
) -> AppResult<(StatusCode, Json<CreatedVehicle>)> {
    me.require(Capability::EditOrders)?;
    let fields = body.fields()?;
    let mut conn = state.db.acquire().await?;
    let existing =
        vehicles::find_existing(&mut *conn, fields.plate.as_deref(), fields.vin.as_deref()).await?;
    let already = existing.is_some();
    let vehicle = match existing {
        Some(v) => vehicles::enrich(&mut *conn, v.id, &fields)
            .await?
            .unwrap_or(v),
        None => vehicles::insert(&mut *conn, &fields).await?,
    };
    Ok((
        StatusCode::CREATED,
        Json(CreatedVehicle {
            vehicle,
            existing: already,
        }),
    ))
}

#[derive(Serialize, ToSchema)]
struct VehicleOrder {
    id: i64,
    number: String,
    title: String,
}

#[derive(Serialize, ToSchema)]
struct VehicleDetail {
    vehicle: Vehicle,
    /// Every job this van has been through, newest first.
    orders: Vec<VehicleOrder>,
}

#[utoipa::path(
    get, path = "/vehicles/{id}", tag = "vehicles",
    params(("id" = i64, Path)),
    responses((status = 200, body = VehicleDetail))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<VehicleDetail>> {
    let vehicle = vehicles::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("vehicle"))?;
    let orders = vehicles::orders_for_vehicle(&state.db, id)
        .await?
        .into_iter()
        .map(|(id, number, title)| VehicleOrder { id, number, title })
        .collect();
    Ok(Json(VehicleDetail { vehicle, orders }))
}

#[utoipa::path(
    patch, path = "/vehicles/{id}", tag = "vehicles",
    params(("id" = i64, Path)),
    request_body = VehicleBody,
    responses((status = 200, body = Vehicle))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<VehicleBody>,
) -> AppResult<Json<Vehicle>> {
    me.require(Capability::EditOrders)?;
    let fields = body.fields()?;
    Ok(Json(
        vehicles::update(&state.db, id, &fields)
            .await?
            .ok_or(AppError::NotFound("vehicle"))?,
    ))
}

#[utoipa::path(
    get, path = "/orders/{id}/vehicles", tag = "vehicles",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Vehicle>))
)]
async fn list_for_order(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(order_id): ApiPath<i64>,
) -> AppResult<Json<Items<Vehicle>>> {
    orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    Ok(Items::new(
        vehicles::list_for_order(&state.db, order_id).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct AttachBody {
    vehicle_id: i64,
}

/// One job can cover several identical vans, each with its own MEO photos and certificate.
#[utoipa::path(
    post, path = "/orders/{id}/vehicles", tag = "vehicles",
    params(("id" = i64, Path)),
    request_body = AttachBody,
    responses((status = 204))
)]
async fn attach(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiJson(body): ApiJson<AttachBody>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditOrders)?;
    orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    vehicles::find(&state.db, body.vehicle_id)
        .await?
        .ok_or(AppError::NotFound("vehicle"))?;
    vehicles::attach(&state.db, order_id, body.vehicle_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete, path = "/orders/{order_id}/vehicles/{vehicle_id}", tag = "vehicles",
    params(("order_id" = i64, Path), ("vehicle_id" = i64, Path)),
    responses((status = 204))
)]
async fn detach(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((order_id, vehicle_id)): ApiPath<(i64, i64)>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditOrders)?;
    if vehicles::detach(&state.db, order_id, vehicle_id).await? == 0 {
        return Err(AppError::NotFound("vehicle"));
    }
    Ok(StatusCode::NO_CONTENT)
}
