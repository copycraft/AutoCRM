//! The yard board (0048): where each vehicle stands, moved by dragging on the web or by a
//! picker on the phone. Places are configuration, edited by admins; moving a vehicle is
//! shop-floor work (`ChangeStages`).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::lookups::YARD_KIND_KEYS;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::yard::{self, VehicleMove, YardLocation, YardVehicle};
use crate::repo::{audit, orders, vehicles};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(locations, create_location))
        .routes(routes!(update_location))
        .routes(routes!(board))
        .routes(routes!(move_vehicle))
        .routes(routes!(vehicle_moves))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct LocationsQuery {
    #[serde(default)]
    include_archived: bool,
}

#[utoipa::path(
    get, path = "/yard/locations", tag = "yard",
    params(LocationsQuery),
    responses((status = 200, body = Items<YardLocation>))
)]
async fn locations(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<LocationsQuery>,
) -> AppResult<Json<Items<YardLocation>>> {
    Ok(Items::new(
        yard::locations(&state.db, q.include_archived).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct LocationBody {
    name: Option<String>,
    /// `bay` | `parking` | `external` (`yard_kinds` in `GET /config/lookups`).
    kind: Option<String>,
    /// How many vehicles fit. Send null for no limit.
    #[serde(default, deserialize_with = "super::patch")]
    capacity: Option<Option<i32>>,
    position: Option<i32>,
    archived: Option<bool>,
}

fn check_location(kind: &str, capacity: Option<i32>) -> AppResult<()> {
    if !YARD_KIND_KEYS.contains(&kind) {
        return Err(AppError::validation(format!(
            "kind must be one of {}",
            YARD_KIND_KEYS.join(", ")
        )));
    }
    if capacity.is_some_and(|c| !(1..=100).contains(&c)) {
        return Err(AppError::validation("capacity must be between 1 and 100"));
    }
    Ok(())
}

fn duplicate_name(e: sqlx::Error) -> AppError {
    if e.to_string().contains("yard_locations_name_idx") {
        AppError::conflict("duplicate", "a place with this name already exists")
    } else {
        AppError::Database(e)
    }
}

#[utoipa::path(
    post, path = "/yard/locations", tag = "yard",
    request_body = LocationBody,
    responses((status = 201, body = YardLocation))
)]
async fn create_location(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<LocationBody>,
) -> AppResult<(StatusCode, Json<YardLocation>)> {
    me.require(Capability::ManageConfiguration)?;
    let name = super::required("name", b.name.as_deref().unwrap_or(""))?;
    let kind = b.kind.unwrap_or_else(|| "bay".into());
    let capacity = b.capacity.flatten();
    check_location(&kind, capacity)?;
    let position = match b.position {
        Some(p) => p,
        None => {
            yard::locations(&state.db, true)
                .await?
                .iter()
                .map(|l| l.position)
                .max()
                .unwrap_or(0)
                + 10
        }
    };
    let created = yard::insert_location(&state.db, &name, &kind, capacity, position)
        .await
        .map_err(duplicate_name)?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch, path = "/yard/locations/{id}", tag = "yard",
    params(("id" = i64, Path)),
    request_body = LocationBody,
    responses((status = 200, body = YardLocation))
)]
async fn update_location(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LocationBody>,
) -> AppResult<Json<YardLocation>> {
    me.require(Capability::ManageConfiguration)?;
    let current = yard::location(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("yard location"))?;
    let name = match b.name {
        Some(n) => super::required("name", &n)?,
        None => current.name,
    };
    let kind = b.kind.unwrap_or(current.kind);
    let capacity = b.capacity.unwrap_or(current.capacity);
    check_location(&kind, capacity)?;
    let updated = yard::update_location(
        &state.db,
        id,
        &name,
        &kind,
        capacity,
        b.position.unwrap_or(current.position),
        b.archived.unwrap_or(current.archived_at.is_some()),
    )
    .await
    .map_err(duplicate_name)?
    .ok_or(AppError::NotFound("yard location"))?;
    Ok(Json(updated))
}

#[derive(Serialize, ToSchema)]
struct Board {
    /// Live places in board order.
    locations: Vec<YardLocation>,
    /// Vehicles of open jobs, and any vehicle still standing on a place. `location_id`
    /// null: not placed yet.
    vehicles: Vec<YardVehicle>,
}

#[utoipa::path(
    get, path = "/yard/board", tag = "yard",
    responses((status = 200, body = Board))
)]
async fn board(State(state): State<AppState>, Auth(_): Auth) -> AppResult<Json<Board>> {
    Ok(Json(Board {
        locations: yard::locations(&state.db, false).await?,
        vehicles: yard::board_vehicles(&state.db).await?,
    }))
}

#[derive(Deserialize, ToSchema)]
struct MoveBody {
    vehicle_id: i64,
    /// Where to. Null: the vehicle left the site.
    location_id: Option<i64>,
    /// The job it is moved for; it then shows in that job's history.
    order_id: Option<i64>,
    note: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct MoveResult {
    id: i64,
    /// The place is full with this vehicle in it (not refused: a warning to show).
    over_capacity: bool,
}

#[utoipa::path(
    post, path = "/yard/moves", tag = "yard",
    request_body = MoveBody,
    responses((status = 201, body = MoveResult))
)]
async fn move_vehicle(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<MoveBody>,
) -> AppResult<(StatusCode, Json<MoveResult>)> {
    me.require(Capability::ChangeStages)?;
    vehicles::find(&state.db, b.vehicle_id)
        .await?
        .ok_or(AppError::NotFound("vehicle"))?;
    let mut over_capacity = false;
    if let Some(location_id) = b.location_id {
        let location = yard::location(&state.db, location_id)
            .await?
            .filter(|l| l.archived_at.is_none())
            .ok_or(AppError::NotFound("yard location"))?;
        if let Some(capacity) = location.capacity {
            let here = yard::board_vehicles(&state.db)
                .await?
                .iter()
                .filter(|v| v.location_id == Some(location_id) && v.vehicle_id != b.vehicle_id)
                .count();
            over_capacity = here as i64 + 1 > i64::from(capacity);
        }
    }
    if let Some(order_id) = b.order_id {
        orders::find(&state.db, order_id)
            .await?
            .ok_or(AppError::NotFound("order"))?;
    }
    let note = super::optional(b.note);
    if note.as_deref().is_some_and(|n| n.chars().count() > 500) {
        return Err(AppError::validation("note is at most 500 characters"));
    }
    let mut tx = state.db.begin().await?;
    let id = yard::insert_move(
        &mut *tx,
        b.vehicle_id,
        b.location_id,
        b.order_id,
        note.as_deref(),
        me.user_id,
    )
    .await?;
    if let Some(order_id) = b.order_id {
        audit::record(
            &mut *tx,
            Some(me.user_id),
            "order",
            order_id,
            "vehicle_moved",
            json!({ "vehicle_id": b.vehicle_id, "location_id": b.location_id }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(MoveResult { id, over_capacity })))
}

#[utoipa::path(
    get, path = "/vehicles/{id}/moves", tag = "yard",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<VehicleMove>))
)]
async fn vehicle_moves(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<VehicleMove>>> {
    vehicles::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("vehicle"))?;
    Ok(Items::new(yard::moves_of(&state.db, id, 100).await?))
}
