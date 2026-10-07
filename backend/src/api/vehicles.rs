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
use super::{
    Items, apply_patch, optional, page_limit, page_offset, patch as patch_field, patch_text,
};
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
        .routes(routes!(decode_vin))
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

// PATCH semantics (docs/API.md): an omitted field keeps its value, `null` clears it.
#[derive(Deserialize, ToSchema)]
struct VehicleBody {
    #[serde(default, deserialize_with = "patch_field")]
    vin: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    plate: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    make: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    model: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    year: Option<Option<i32>>,
    #[serde(default, deserialize_with = "patch_field")]
    partner_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
}

impl VehicleBody {
    /// Create: every field as sent.
    fn fields(self) -> AppResult<VehicleFields> {
        identified(VehicleFields {
            vin: optional(self.vin.flatten()).map(|v| v.to_uppercase()),
            plate: optional(self.plate.flatten()).map(|p| p.to_uppercase()),
            make: optional(self.make.flatten()),
            model: optional(self.model.flatten()),
            year: self.year.flatten(),
            partner_id: self.partner_id.flatten(),
            notes: optional(self.notes.flatten()),
        })
    }

    /// PATCH: omitted fields keep the stored value, `null` clears.
    fn merged(self, current: &Vehicle) -> AppResult<VehicleFields> {
        identified(VehicleFields {
            vin: patch_text(&current.vin, self.vin).map(|v| v.to_uppercase()),
            plate: patch_text(&current.plate, self.plate).map(|p| p.to_uppercase()),
            make: patch_text(&current.make, self.make),
            model: patch_text(&current.model, self.model),
            year: apply_patch(&current.year, &self.year),
            partner_id: apply_patch(&current.partner_id, &self.partner_id),
            notes: patch_text(&current.notes, self.notes),
        })
    }
}

fn identified(f: VehicleFields) -> AppResult<VehicleFields> {
    if f.vin.is_none() && f.plate.is_none() {
        return Err(AppError::validation(
            "a vehicle needs a plate or a VIN: a make and model do not identify a van",
        ));
    }
    Ok(f)
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
    let current = vehicles::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("vehicle"))?;
    let fields = body.merged(&current)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn stored() -> Vehicle {
        Vehicle {
            id: 3,
            vin: Some("WDB9066331S123456".into()),
            plate: Some("ABC-123".into()),
            plate_norm: Some("ABC123".into()),
            make: Some("Mercedes".into()),
            model: Some("Sprinter".into()),
            year: Some(2021),
            partner_id: Some(9),
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn body(json: &str) -> VehicleBody {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn vehicle_patch_keeps_omitted_fields() {
        // Before the fix PATCH went through the create path, which reads a notes-only
        // body as "no plate, no VIN" and refuses it (or, with a plate, wipes the rest).
        assert!(body(r#"{"notes":"hátsó ajtó sérült"}"#).fields().is_err());

        let f = body(r#"{"notes":"hátsó ajtó sérült"}"#)
            .merged(&stored())
            .unwrap();
        assert_eq!(f.plate.as_deref(), Some("ABC-123"));
        assert_eq!(f.vin.as_deref(), Some("WDB9066331S123456"));
        assert_eq!(f.make.as_deref(), Some("Mercedes"));
        assert_eq!(f.model.as_deref(), Some("Sprinter"));
        assert_eq!(f.year, Some(2021));
        assert_eq!(f.partner_id, Some(9));
        assert_eq!(f.notes.as_deref(), Some("hátsó ajtó sérült"));
    }

    #[test]
    fn vehicle_patch_null_clears_but_never_both_identifiers() {
        let f = body(r#"{"make":null,"year":null,"partner_id":null}"#)
            .merged(&stored())
            .unwrap();
        assert_eq!(f.make, None);
        assert_eq!(f.year, None);
        assert_eq!(f.partner_id, None);
        assert_eq!(f.plate.as_deref(), Some("ABC-123"));
        assert!(
            body(r#"{"plate":null,"vin":null}"#)
                .merged(&stored())
                .is_err()
        );
    }

    #[test]
    fn vehicle_patch_uppercases_a_new_plate() {
        let f = body(r#"{"plate":"xyz-987"}"#).merged(&stored()).unwrap();
        assert_eq!(f.plate.as_deref(), Some("XYZ-987"));
    }
}

#[utoipa::path(
    get, path = "/vehicles/decode/{vin}", tag = "vehicles",
    params(("vin" = String, Path, description = "17 characters; spaces and dashes are ignored")),
    responses((status = 200, body = crate::domain::vin::VinInfo, description = "What the VIN itself says (maker, region, model year), plus make and model from NHTSA when VIN_DECODER_ONLINE is on"))
)]
async fn decode_vin(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(vin): ApiPath<String>,
) -> AppResult<Json<crate::domain::vin::VinInfo>> {
    use chrono::Datelike;
    let next_year = crate::service::business_today(state.config.business_tz).year() + 1;
    let mut info = crate::domain::vin::decode(&vin, next_year)
        .ok_or_else(|| AppError::validation("vin: 17 letters and digits, without I, O and Q"))?;
    if state.config.vin_online {
        match crate::integrations::vpic::lookup(&info.vin).await {
            Ok(found) => {
                if found.make.is_some() || found.model.is_some() {
                    info.make = found.make;
                    info.model = found.model;
                    info.model_year = found.model_year.or(info.model_year);
                    info.source = "nhtsa".into();
                }
            }
            Err(e) => tracing::warn!(error = %e, "online VIN lookup failed; offline answer only"),
        }
    }
    Ok(Json(info))
}
