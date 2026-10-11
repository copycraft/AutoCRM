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
use crate::repo::yard::{
    self, LayoutItem, LayoutRevision, VehicleMove, YardLocation, YardVehicle,
};
use crate::repo::{audit, orders, vehicles};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(locations, create_location))
        .routes(routes!(update_location))
        .routes(routes!(board))
        .routes(routes!(move_vehicle))
        .routes(routes!(vehicle_moves))
        .routes(routes!(layout))
        .routes(routes!(save_layout))
        .routes(routes!(update_layout_item))
        .routes(routes!(delete_layout_item))
        .routes(routes!(revisions))
        .routes(routes!(revert_revision))
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

// --- Layout (0049) ---

#[utoipa::path(
    get, path = "/yard/layout", tag = "yard",
    responses((status = 200, body = Items<LayoutItem>))
)]
async fn layout(State(state): State<AppState>, Auth(_): Auth) -> AppResult<Json<Items<LayoutItem>>> {
    Ok(Items::new(yard::layout_items(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct LayoutItemBody {
    /// Required on create, must not change on update.
    kind: Option<String>,
    #[serde(default, deserialize_with = "super::patch")]
    location_id: Option<Option<i64>>,
    label: Option<String>,
    number: Option<String>,
    x: Option<i32>,
    y: Option<i32>,
    width: Option<i32>,
    height: Option<i32>,
    rotation: Option<i32>,
    #[serde(default, deserialize_with = "super::patch")]
    parent_id: Option<Option<i64>>,
    color: Option<String>,
    position: Option<i32>,
}

fn validate_item(body: &LayoutItemBody, partial: bool, existing: Option<&LayoutItem>) -> AppResult<(String, Option<i64>, String, String, i32, i32, i32, i32, i32, Option<i64>, String, i32)> {
    let err = |m: &str| Err(AppError::validation(m));
    let kind_owned = body.kind.clone();
    let kind = kind_owned.as_deref().unwrap_or("");
    if !partial && !matches!(kind, "spot" | "building" | "workshop_bay") {
        return err("kind must be spot, building, or workshop_bay");
    }
    let kind = match kind {
        k @ ("spot" | "building" | "workshop_bay") => k.to_string(),
        _ => match existing {
            Some(e) => e.kind.clone(),
            None => return err("kind must be spot, building, or workshop_bay"),
        },
    };
    let location_id = body.location_id.flatten().or_else(|| existing.and_then(|e| e.location_id));
    let parent_id = body.parent_id.flatten().or_else(|| existing.and_then(|e| e.parent_id));
    if kind == "spot" && location_id.is_none() {
        return err("spot requires location_id");
    }
    if kind == "workshop_bay" && parent_id.is_none() {
        return err("workshop_bay requires parent_id");
    }
    let rotation = body.rotation.or(existing.map(|e| e.rotation)).unwrap_or(0);
    if !matches!(rotation, 0 | 90 | 180 | 270) {
        return err("rotation must be 0, 90, 180, or 270");
    }
    let width = body.width.or(existing.map(|e| e.width)).unwrap_or(1);
    let height = body.height.or(existing.map(|e| e.height)).unwrap_or(1);
    if width < 1 || height < 1 {
        return err("width and height must be at least 1");
    }
    let label = body.label.clone().or_else(|| existing.map(|e| e.label.clone())).unwrap_or_default();
    let number = body.number.clone().or_else(|| existing.map(|e| e.number.clone())).unwrap_or_default();
    let color = body.color.clone().or_else(|| existing.map(|e| e.color.clone())).unwrap_or_else(|| "#6b7280".into());
    let position = body.position.or(existing.map(|e| e.position)).unwrap_or(0);
    let x = body.x.or(existing.map(|e| e.x)).unwrap_or(0);
    let y = body.y.or(existing.map(|e| e.y)).unwrap_or(0);
    Ok((kind, location_id, label, number, x, y, width, height, rotation, parent_id, color, position))
}

#[utoipa::path(
    post, path = "/yard/layout", tag = "yard",
    request_body = LayoutItemBody,
    responses((status = 201, body = LayoutItem))
)]
async fn save_layout(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<LayoutItemBody>,
) -> AppResult<(StatusCode, Json<LayoutItem>)> {
    me.require(Capability::ManageConfiguration)?;
    let (kind, location_id, label, number, x, y, width, height, rotation, parent_id, color, position) =
        validate_item(&b, false, None)?;
    let created = yard::insert_layout_item(
        &state.db, &kind, location_id, &label, &number, x, y, width, height, rotation, parent_id, &color, position,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch, path = "/yard/layout/{id}", tag = "yard",
    params(("id" = i64, Path)),
    request_body = LayoutItemBody,
    responses((status = 200, body = LayoutItem))
)]
async fn update_layout_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LayoutItemBody>,
) -> AppResult<Json<LayoutItem>> {
    me.require(Capability::ManageConfiguration)?;
    let existing = yard::layout_item(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("layout item"))?;
    let (_kind, location_id, label, number, x, y, width, height, rotation, parent_id, color, position) =
        validate_item(&b, true, Some(&existing))?;
    let updated = yard::update_layout_item(
        &state.db, id, location_id, &label, &number, x, y, width, height, rotation, parent_id, &color, position,
    )
    .await?
    .ok_or(AppError::NotFound("layout item"))?;
    Ok(Json(updated))
}

#[utoipa::path(
    delete, path = "/yard/layout/{id}", tag = "yard",
    params(("id" = i64, Path)),
    responses((status = 204))
)]
async fn delete_layout_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageConfiguration)?;
    yard::delete_layout_item(&state.db, id)
        .await?
        .then_some(())
        .ok_or(AppError::NotFound("layout item"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct RevisionsQuery {
    #[serde(default = "default_limit")]
    limit: i64,
}
fn default_limit() -> i64 { 50 }

#[utoipa::path(
    get, path = "/yard/revisions", tag = "yard",
    params(RevisionsQuery),
    responses((status = 200, body = Items<LayoutRevision>))
)]
async fn revisions(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<RevisionsQuery>,
) -> AppResult<Json<Items<LayoutRevision>>> {
    Ok(Items::new(yard::revisions(&state.db, q.limit.clamp(1, 200)).await?))
}

#[derive(Deserialize, ToSchema)]
struct RevertBody {
    revision_id: i64,
    note: Option<String>,
}

#[utoipa::path(
    post, path = "/yard/revert", tag = "yard",
    request_body = RevertBody,
    responses((status = 201, body = LayoutRevision))
)]
async fn revert_revision(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<RevertBody>,
) -> AppResult<(StatusCode, Json<LayoutRevision>)> {
    me.require(Capability::ManageConfiguration)?;
    let target = yard::revision(&state.db, b.revision_id)
        .await?
        .ok_or(AppError::NotFound("revision"))?;
    let items: serde_json::Value = target.layout;
    let arr = items.as_array().ok_or_else(|| AppError::validation("revision layout is not an array"))?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM yard_layout_items").execute(&mut *tx).await?;
    for item in arr {
        let get = |k: &str| item.get(k);
        let kind = get("kind").and_then(|v| v.as_str()).ok_or_else(|| AppError::validation("item missing kind"))?;
        let location_id = get("location_id").and_then(|v| v.as_i64());
        let label = get("label").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let number = get("number").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let x = get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let width = get("width").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let height = get("height").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
        let rotation = get("rotation").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let parent_id = get("parent_id").and_then(|v| v.as_i64());
        let color = get("color").and_then(|v| v.as_str()).unwrap_or("#6b7280").to_string();
        let position = get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        yard::insert_layout_item(
            &mut *tx, kind, location_id, &label, &number, x, y, width, height, rotation, parent_id, &color, position,
        ).await?;
    }
    let note = super::optional(b.note);
    let note = note.unwrap_or_else(|| format!("Reverted to revision {}", b.revision_id));
    let id = yard::insert_revision(&mut *tx, Some(b.revision_id), items, &note, me.user_id).await?;
    tx.commit().await?;
    let rev = yard::revision(&state.db, id).await?.ok_or(AppError::NotFound("revision"))?;
    Ok((StatusCode::CREATED, Json(rev)))
}
