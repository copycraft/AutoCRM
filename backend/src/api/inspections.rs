//! Vehicle handover inspections (átadás-átvétel): the rental-company style
//! check-out / check-in damage record.
//!
//! Order-linked only: an inspection belongs to the order whose car is handed over,
//! and the web surfaces it inside the Átvételi lap. Creation happens on the phone
//! (guided walkaround); the web reads history, comparisons and zone templates.
//!
//! Photos travel through the normal ticket → PUT → complete flow with category
//! `inspection` and are attached here with zone + purpose + capture metadata.
//! Signatures are finger-drawn PNGs uploaded as `other` documents.
//!
//! Capability split: reads are open, every mutation needs `ChangeStages` (the shop
//! floor inspects: admin, office and designer), and zone templates need
//! `ManageConfiguration`. A signed inspection is locked — later remarks go to
//! notes, which is also how the phone syncs post-sign annotations.

use std::collections::BTreeMap;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::lookups::{
    DAMAGE_TYPE_KEYS as DAMAGE_TYPES, SEVERITY_KEYS as SEVERITIES, TYRE_CONDITION_KEYS,
    TYRE_POSITION_KEYS, VERDICT_KEYS as VERDICTS, WALKAROUND_KEYS as KINDS,
};
use crate::domain::media::DocumentKind;
use crate::domain::media::ImageCategory;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::documents::{self, Owner};
use crate::repo::images::{self, Image};
use crate::repo::inspections::{
    self, Inspection, InspectionDamage, InspectionNote, InspectionPhoto as InspectionPhotoRow,
    InspectionSignature, InspectionTyre, InspectionVerdict, InspectionVideo as InspectionVideoRow,
    ZoneTemplate,
};
use crate::repo::{config, orders};

const URL_TTL: Duration = Duration::from_secs(3600);

const PURPOSES: &[&str] = &["overview", "closeup", "dashboard", "signature"];
// DAMAGE_TYPES / SEVERITIES / VERDICTS / KINDS come from `domain::lookups` (imported
// above), next to the labels `GET /config/lookups` publishes: one definition, no drift.
const MAX_ZONES: usize = 80;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(detail, patch, remove))
        .routes(routes!(attach_photo))
        .routes(routes!(add_damage, remove_damage))
        .routes(routes!(add_signature))
        .routes(routes!(sign))
        .routes(routes!(add_note))
        .routes(routes!(comparison, set_verdict))
        .routes(routes!(templates, replace_templates, delete_templates))
        .routes(routes!(put_tyres))
        .routes(routes!(attach_video))
}

fn in_list(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{field} must be one of {}, not '{value}'",
            allowed.join(", ")
        )))
    }
}

async fn draft_or_locked(db: &sqlx::PgPool, id: i64) -> AppResult<Inspection> {
    let inspection = inspections::find(db, id)
        .await?
        .ok_or(AppError::NotFound("inspection"))?;
    if inspection.status != "draft" {
        return Err(AppError::rule(
            "locked",
            "a signed inspection cannot be changed; add a follow-up note instead",
        ));
    }
    Ok(inspection)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    order_id: Option<i64>,
    /// Full vehicle history across orders, by plate.
    plate: Option<String>,
}

#[utoipa::path(
    get, path = "/inspections", tag = "inspections",
    params(ListQuery),
    responses((status = 200, body = Items<Inspection>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<Inspection>>> {
    if let Some(order_id) = q.order_id {
        orders::find(&state.db, order_id)
            .await?
            .ok_or(AppError::NotFound("order"))?;
        return Ok(Items::new(
            inspections::list_for_order(&state.db, order_id).await?,
        ));
    }
    if let Some(plate) = q.plate {
        let plate = plate.trim();
        if plate.is_empty() {
            return Err(AppError::validation("plate is required"));
        }
        return Ok(Items::new(
            inspections::list_for_plate(&state.db, plate).await?,
        ));
    }
    Err(AppError::validation("order_id or plate is required"))
}

#[derive(Deserialize, ToSchema)]
struct CreateBody {
    order_id: i64,
    /// `checkout` | `checkin`.
    kind: String,
    vehicle_plate: Option<String>,
    vehicle_vin: Option<String>,
    inspector_name: String,
    driver_name: Option<String>,
    location: Option<String>,
    odometer: Option<i32>,
    fuel_level: Option<String>,
    battery_pct: Option<i32>,
    warning_lights: Option<String>,
    /// Átvételi lap, read at the car. Only a `checkout` (átvétel) uses these: it writes them,
    /// with `odometer`, `fuel_level` and a plate or VIN the order lacks, onto the order, so
    /// the walkaround is the intake slip and leaving `intake` needs no second form.
    #[serde(default)]
    key_count: Option<i32>,
    #[serde(default)]
    intake_condition: Option<String>,
    /// Whether the valuables question was asked; `valuables` says what, and needs `true`.
    #[serde(default)]
    valuables_declared: Option<bool>,
    #[serde(default)]
    valuables: Option<String>,
    /// Optional idempotency key (1..100 chars), e.g. the phone's local draft UUID.
    /// A repeat create with the same key returns the inspection it already created
    /// (200) instead of a second row or `checkout_open`. Reusing a key for a
    /// different order or kind answers 409 `duplicate`.
    #[serde(default)]
    client_key: Option<String>,
}

/// Normalises the optional idempotency key: blank means "no key".
fn client_key(raw: Option<&str>) -> AppResult<Option<String>> {
    match raw.map(str::trim).filter(|k| !k.is_empty()) {
        None => Ok(None),
        Some(k) if k.chars().count() > 100 => {
            Err(AppError::validation("client_key is at most 100 characters"))
        }
        Some(k) => Ok(Some(k.to_string())),
    }
}

/// A key replay is only the same request when it names the same order and kind;
/// anything else is a client reusing a key, which must not hand back an
/// unrelated inspection.
fn replay_matches(existing: &Inspection, order_id: i64, kind: &str) -> AppResult<()> {
    if existing.order_id == order_id && existing.kind == kind {
        Ok(())
    } else {
        Err(AppError::conflict(
            "duplicate",
            "client_key was already used for a different inspection",
        ))
    }
}

#[utoipa::path(
    post, path = "/inspections", tag = "inspections",
    request_body(content = CreateBody),
    responses(
        (status = 201, description = "Created", body = Inspection),
        (status = 200, description = "Already created under this `client_key`", body = Inspection)
    )
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(mut b): ApiJson<CreateBody>,
) -> AppResult<(StatusCode, Json<Inspection>)> {
    me.require(Capability::ChangeStages)?;
    if !KINDS.contains(&b.kind.as_str()) {
        return Err(AppError::validation("kind must be checkout or checkin"));
    }
    let order = orders::find(&state.db, b.order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let key = client_key(b.client_key.as_deref())?;
    // Replay first: the row exists, so the rules that guard creating it (e.g.
    // `checkout_open` against itself) no longer apply.
    if let Some(key) = &key {
        if let Some(existing) = inspections::find_by_client_key(&state.db, key).await? {
            replay_matches(&existing, b.order_id, &b.kind)?;
            return Ok((StatusCode::OK, Json(existing)));
        }
    }
    let inspector_name = super::required("inspector_name", &b.inspector_name)?;
    if let Some(o) = b.odometer {
        if o < 0 {
            return Err(AppError::validation("odometer cannot be negative"));
        }
    }
    if let Some(p) = b.battery_pct {
        if !(0..=100).contains(&p) {
            return Err(AppError::validation("battery_pct must be 0..100"));
        }
    }
    // An átvétel is also the átvételi lap: read and checked now, written to the order once
    // the inspection exists.
    let slip = if b.kind == "checkout" {
        let (valuables_declared, valuables) =
            super::orders::valuables(b.valuables_declared, b.valuables.take())?;
        let upper = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_uppercase)
        };
        Some(orders::IntakeSlip {
            mileage_in: b.odometer,
            fuel_level: super::orders::fuel_level(b.fuel_level.clone())?,
            key_count: super::orders::key_count(b.key_count)?,
            intake_condition: super::optional(b.intake_condition.take()),
            valuables_declared,
            valuables,
            vehicle_plate: upper(&b.vehicle_plate),
            vehicle_vin: upper(&b.vehicle_vin),
        })
    } else {
        None
    };
    // A check-in is always compared: it links the latest signed check-out, and
    // there is no check-in without one.
    let checkout_id = if b.kind == "checkin" {
        Some(
            inspections::latest_signed_checkout(&state.db, b.order_id)
                .await?
                .map(|c| c.id)
                .ok_or_else(|| {
                    AppError::rule(
                        "checkout_required",
                        "a signed check-out is required before check-in",
                    )
                })?,
        )
    } else {
        None
    };
    let new = inspections::NewInspection {
        order_id: b.order_id,
        kind: std::mem::take(&mut b.kind),
        vehicle_plate: b
            .vehicle_plate
            .filter(|p| !p.trim().is_empty())
            .or_else(|| order.vehicle_plate.clone())
            .ok_or_else(|| AppError::validation("vehicle_plate is required"))?,
        vehicle_vin: b
            .vehicle_vin
            .filter(|v| !v.trim().is_empty())
            .or_else(|| order.vehicle_vin.clone()),
        inspector_name,
        driver_name: super::optional(b.driver_name),
        location: super::optional(b.location),
        odometer: b.odometer,
        fuel_level: super::optional(b.fuel_level),
        battery_pct: b.battery_pct,
        warning_lights: super::optional(b.warning_lights),
        checkout_id,
        created_by: me.user_id,
    };
    let created = match &key {
        Some(key) => inspections::create_with_client_key(&state.db, &new, key).await,
        None => inspections::create(&state.db, &new)
            .await
            .map(|i| (i, true)),
    };
    let (inspection, is_new) = created.map_err(|e| {
        // The one-draft-checkout-per-order unique index surfaces as a DB error;
        // translate it into the rule the phone explains.
        let msg = e.to_string();
        if msg.contains("inspections_one_draft_checkout") {
            AppError::rule(
                "checkout_open",
                "this order already has an open check-out; sign or discard it first",
            )
        } else {
            AppError::Database(e)
        }
    })?;
    if !is_new {
        // Lost a race to a concurrent retry with the same key.
        replay_matches(&inspection, new.order_id, &new.kind)?;
        return Ok((StatusCode::OK, Json(inspection)));
    }
    if let Some(slip) = slip {
        let mut conn = state.db.acquire().await?;
        crate::service::orders::apply_walkaround_intake(&mut conn, me.user_id, &order, &slip)
            .await?;
    }
    Ok((StatusCode::CREATED, Json(inspection)))
}

#[derive(Serialize, ToSchema)]
struct InspectionPhoto {
    id: i64,
    inspection_id: i64,
    image_id: i64,
    zone_key: String,
    purpose: String,
    damage_id: Option<i64>,
    taken_at: DateTime<Utc>,
    lat: Option<f64>,
    lon: Option<f64>,
    created_at: DateTime<Utc>,
    /// Presigned, expires after one hour; null until generated.
    thumb_url: Option<String>,
    /// Presigned, expires after one hour.
    display_url: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct InspectionDetail {
    inspection: Inspection,
    photos: Vec<InspectionPhoto>,
    damages: Vec<InspectionDamage>,
    verdicts: Vec<InspectionVerdict>,
    signatures: Vec<InspectionSignature>,
    notes: Vec<InspectionNote>,
    /// The heading of every zone in the list this inspection is walked with, by zone key,
    /// so a client can name a zone without keeping its own table. A zone the office has
    /// since removed from the list has no entry: show its key.
    zone_titles: BTreeMap<String, String>,
    /// Each tyre's tread and condition (0048).
    tyres: Vec<InspectionTyre>,
    /// Walkaround clips (0048).
    videos: Vec<InspectionVideoView>,
}

#[derive(Serialize, ToSchema)]
struct InspectionVideoView {
    #[serde(flatten)]
    video: InspectionVideoRow,
    filename: String,
    byte_size: i64,
    content_type: String,
    /// Presigned, served inline for the video player; expires after one hour.
    url: Option<String>,
}

async fn video_views(
    state: &AppState,
    rows: Vec<InspectionVideoRow>,
) -> AppResult<Vec<InspectionVideoView>> {
    let mut out = Vec::with_capacity(rows.len());
    for video in rows {
        let Some(doc) = documents::find(&state.db, video.document_id)
            .await?
            .filter(|d| d.deleted_at.is_none())
        else {
            continue;
        };
        let url = state
            .storage
            .presign_get(
                &doc.storage_key,
                URL_TTL,
                Some(crate::media::storage::content_disposition(
                    "inline",
                    &doc.filename,
                )),
            )
            .await?;
        out.push(InspectionVideoView {
            video,
            filename: doc.filename,
            byte_size: doc.byte_size,
            content_type: doc.content_type,
            url: Some(url),
        });
    }
    Ok(out)
}

async fn photo_views(
    state: &AppState,
    photos: Vec<InspectionPhotoRow>,
) -> AppResult<Vec<InspectionPhoto>> {
    let mut views = Vec::with_capacity(photos.len());
    for photo in photos {
        let image: Option<Image> = images::find(&state.db, photo.image_id).await?;
        let (thumb_url, display_url) = match image {
            Some(image) if image.deleted_at.is_none() => {
                let thumb = match &image.thumb_key {
                    Some(k) => Some(state.storage.presign_get(k, URL_TTL, None).await?),
                    None => None,
                };
                let display = match &image.display_key {
                    Some(k) => Some(state.storage.presign_get(k, URL_TTL, None).await?),
                    None => None,
                };
                (thumb, display)
            }
            _ => (None, None),
        };
        views.push(InspectionPhoto {
            id: photo.id,
            inspection_id: photo.inspection_id,
            image_id: photo.image_id,
            zone_key: photo.zone_key,
            purpose: photo.purpose,
            damage_id: photo.damage_id,
            taken_at: photo.taken_at,
            lat: photo.lat,
            lon: photo.lon,
            created_at: photo.created_at,
            thumb_url,
            display_url,
        });
    }
    Ok(views)
}

async fn detail_for(state: &AppState, inspection: &Inspection) -> AppResult<InspectionDetail> {
    let photos = inspections::photos_for(&state.db, inspection.id).await?;
    let project_type_id = orders::find(&state.db, inspection.order_id)
        .await?
        .and_then(|o| o.project_type_id);
    let zone_titles = {
        let mut conn = state.db.acquire().await?;
        inspections::templates_for(&mut conn, project_type_id, &inspection.kind).await?
    }
    .into_iter()
    .map(|z| (z.zone_key, z.title))
    .collect();
    Ok(InspectionDetail {
        zone_titles,
        inspection: inspection.clone(),
        photos: photo_views(state, photos).await?,
        damages: inspections::damages_for(&state.db, inspection.id).await?,
        verdicts: if inspection.kind == "checkin" {
            inspections::verdicts_for(&state.db, inspection.id).await?
        } else {
            Vec::new()
        },
        signatures: inspections::signatures_for(&state.db, inspection.id).await?,
        notes: inspections::notes_for(&state.db, inspection.id).await?,
        tyres: inspections::tyres_for(&state.db, inspection.id).await?,
        videos: video_views(
            state,
            inspections::videos_for(&state.db, inspection.id).await?,
        )
        .await?,
    })
}

#[utoipa::path(
    get, path = "/inspections/{id}", tag = "inspections",
    params(("id" = i64, Path)),
    responses((status = 200, body = InspectionDetail))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<InspectionDetail>> {
    let inspection = inspections::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("inspection"))?;
    Ok(Json(detail_for(&state, &inspection).await?))
}

#[derive(Deserialize, ToSchema)]
struct PatchBody {
    inspector_name: Option<String>,
    driver_name: Option<Option<String>>,
    location: Option<Option<String>>,
    odometer: Option<Option<i32>>,
    fuel_level: Option<Option<String>>,
    battery_pct: Option<Option<i32>>,
    warning_lights: Option<Option<String>>,
    customer_comment: Option<Option<String>>,
}

#[utoipa::path(
    patch, path = "/inspections/{id}", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = PatchBody),
    responses((status = 200, body = Inspection))
)]
async fn patch(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<PatchBody>,
) -> AppResult<Json<Inspection>> {
    me.require(Capability::ChangeStages)?;
    let current = draft_or_locked(&state.db, id).await?;
    if let Some(Some(o)) = b.odometer {
        if o < 0 {
            return Err(AppError::validation("odometer cannot be negative"));
        }
    }
    if let Some(Some(p)) = b.battery_pct {
        if !(0..=100).contains(&p) {
            return Err(AppError::validation("battery_pct must be 0..100"));
        }
    }
    inspections::patch_draft(
        &state.db,
        id,
        b.inspector_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
        match &b.driver_name {
            None => current.driver_name.as_deref(),
            Some(v) => v.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        },
        match &b.location {
            None => current.location.as_deref(),
            Some(v) => v.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        },
        b.odometer.unwrap_or(current.odometer),
        match &b.fuel_level {
            None => current.fuel_level.as_deref(),
            Some(v) => v.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        },
        b.battery_pct.unwrap_or(current.battery_pct),
        match &b.warning_lights {
            None => current.warning_lights.as_deref(),
            Some(v) => v.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        },
        match &b.customer_comment {
            None => current.customer_comment.as_deref(),
            Some(v) => v.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        },
    )
    .await?
    .map(Json)
    .ok_or(AppError::NotFound("inspection"))
}

#[utoipa::path(
    delete, path = "/inspections/{id}", tag = "inspections",
    params(("id" = i64, Path)),
    responses((status = 204))
)]
async fn remove(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::ChangeStages)?;
    if !inspections::remove_draft(&state.db, id).await? {
        // Either missing or already signed: both read as "cannot discard this".
        let exists = inspections::find(&state.db, id).await?.is_some();
        if exists {
            return Err(AppError::rule(
                "locked",
                "a signed inspection cannot be discarded",
            ));
        }
        return Err(AppError::NotFound("inspection"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct AttachPhotoBody {
    image_id: i64,
    zone_key: String,
    /// `overview` | `closeup` | `dashboard` | `signature`.
    purpose: String,
    damage_id: Option<i64>,
    taken_at: DateTime<Utc>,
    lat: Option<f64>,
    lon: Option<f64>,
}

#[utoipa::path(
    post, path = "/inspections/{id}/photos", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = AttachPhotoBody),
    responses((status = 201, body = InspectionPhotoRow))
)]
async fn attach_photo(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<AttachPhotoBody>,
) -> AppResult<(StatusCode, Json<InspectionPhotoRow>)> {
    me.require(Capability::ChangeStages)?;
    let inspection = draft_or_locked(&state.db, id).await?;
    in_list(&b.purpose, PURPOSES, "purpose")?;
    let zone_key = super::required("zone_key", &b.zone_key)?;
    let image = images::find(&state.db, b.image_id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    // Evidence hygiene: only live-captured inspection photos of the same order
    // attach, and each image attaches once.
    if image.order_id != inspection.order_id {
        return Err(AppError::validation("image belongs to another order"));
    }
    if image.category != ImageCategory::Inspection {
        return Err(AppError::validation("image is not an inspection photo"));
    }
    if let Some(damage_id) = b.damage_id {
        let damages = inspections::damages_for(&state.db, id).await?;
        if !damages.iter().any(|d| d.id == damage_id) {
            return Err(AppError::validation(
                "damage does not belong to this inspection",
            ));
        }
    }
    let photo = inspections::attach_photo(
        &state.db,
        id,
        b.image_id,
        &zone_key,
        &b.purpose,
        b.damage_id,
        b.taken_at,
        b.lat,
        b.lon,
    )
    .await
    .map_err(|e| {
        if e.to_string().contains("inspection_photos_image_id_key") {
            AppError::rule(
                "already_attached",
                "image is already attached to an inspection",
            )
        } else {
            AppError::Database(e)
        }
    })?;
    Ok((StatusCode::CREATED, Json(photo)))
}

#[derive(Deserialize, ToSchema)]
struct DamageBody {
    zone_key: String,
    damage_type: String,
    severity: String,
    note: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    view: Option<String>,
}

#[utoipa::path(
    post, path = "/inspections/{id}/damages", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = DamageBody),
    responses((status = 201, body = InspectionDamage))
)]
async fn add_damage(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<DamageBody>,
) -> AppResult<(StatusCode, Json<InspectionDamage>)> {
    me.require(Capability::ChangeStages)?;
    draft_or_locked(&state.db, id).await?;
    in_list(&b.damage_type, DAMAGE_TYPES, "damage_type")?;
    in_list(&b.severity, SEVERITIES, "severity")?;
    let view = b.view.unwrap_or_else(|| "top".to_string());
    if view != "top" && view != "side" {
        return Err(AppError::validation("view must be top or side"));
    }
    for (name, v) in [("x", b.x), ("y", b.y)] {
        if let Some(v) = v {
            if !(0.0..=1.0).contains(&v) {
                return Err(AppError::validation(format!("{name} must be 0..1")));
            }
        }
    }
    let damage = inspections::add_damage(
        &state.db,
        &inspections::NewDamage {
            inspection_id: id,
            zone_key: super::required("zone_key", &b.zone_key)?,
            damage_type: b.damage_type,
            severity: b.severity,
            note: super::optional(b.note),
            x: b.x,
            y: b.y,
            view,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(damage)))
}

#[utoipa::path(
    delete, path = "/inspections/{id}/damages/{damage_id}", tag = "inspections",
    params(("id" = i64, Path), ("damage_id" = i64, Path)),
    responses((status = 204))
)]
async fn remove_damage(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, damage_id)): ApiPath<(i64, i64)>,
) -> AppResult<StatusCode> {
    me.require(Capability::ChangeStages)?;
    draft_or_locked(&state.db, id).await?;
    if !inspections::remove_damage(&state.db, id, damage_id).await? {
        return Err(AppError::NotFound("damage"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct SignatureBody {
    /// `inspector` | `customer`.
    role: String,
    name: String,
    /// Finger-drawn PNG uploaded as an `other` document of the same order.
    document_id: i64,
}

#[utoipa::path(
    post, path = "/inspections/{id}/signatures", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = SignatureBody),
    responses((status = 201, body = InspectionSignature))
)]
async fn add_signature(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<SignatureBody>,
) -> AppResult<(StatusCode, Json<InspectionSignature>)> {
    me.require(Capability::ChangeStages)?;
    let inspection = draft_or_locked(&state.db, id).await?;
    if b.role != "inspector" && b.role != "customer" {
        return Err(AppError::validation("role must be inspector or customer"));
    }
    let name = super::required("name", &b.name)?;
    let doc = documents::find(&state.db, b.document_id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    if !Owner::Order(inspection.order_id).owns(&doc) {
        return Err(AppError::validation("document belongs to another order"));
    }
    let sig = inspections::add_signature(&state.db, id, &b.role, &name, b.document_id).await?;
    Ok((StatusCode::CREATED, Json(sig)))
}

#[derive(Deserialize, ToSchema)]
struct SignBody {
    customer_comment: Option<String>,
}

#[utoipa::path(
    post, path = "/inspections/{id}/sign", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = SignBody),
    responses((status = 200, body = Inspection))
)]
async fn sign(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<SignBody>,
) -> AppResult<Json<Inspection>> {
    me.require(Capability::ChangeStages)?;
    let inspection = draft_or_locked(&state.db, id).await?;
    let sigs = inspections::signatures_for(&state.db, id).await?;
    let has = |role: &str| sigs.iter().any(|s| s.role == role);
    if !has("inspector") || !has("customer") {
        return Err(AppError::rule(
            "signatures_required",
            "both inspector and customer signatures are required",
        ));
    }
    // A check-in signs only with every damage reviewed: an unreviewed flag is
    // exactly the unattributed damage this system exists to prevent.
    if inspection.kind == "checkin" {
        let damages = inspections::damages_for(&state.db, id).await?;
        let verdicts = inspections::verdicts_for(&state.db, id).await?;
        let reviewed: std::collections::HashSet<i64> =
            verdicts.iter().map(|v| v.checkin_damage_id).collect();
        let pending = damages.iter().filter(|d| !reviewed.contains(&d.id)).count();
        if pending > 0 {
            return Err(AppError::rule(
                "verdicts_pending",
                format!("{pending} damage item(s) still need a review verdict"),
            ));
        }
    }
    inspections::sign(
        &state.db,
        id,
        super::optional(b.customer_comment).as_deref(),
    )
    .await?
    .map(Json)
    .ok_or(AppError::NotFound("inspection"))
}

#[derive(Deserialize, ToSchema)]
struct NoteBody {
    body: String,
}

#[utoipa::path(
    post, path = "/inspections/{id}/notes", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = NoteBody),
    responses((status = 201, body = InspectionNote))
)]
async fn add_note(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<NoteBody>,
) -> AppResult<(StatusCode, Json<InspectionNote>)> {
    me.require(Capability::ChangeStages)?;
    inspections::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("inspection"))?;
    let body = super::required("body", &b.body)?;
    if body.chars().count() > 2000 {
        return Err(AppError::validation("body is at most 2000 characters"));
    }
    let note = inspections::add_note(&state.db, id, &body, me.user_id).await?;
    Ok((StatusCode::CREATED, Json(note)))
}

#[derive(Serialize, ToSchema)]
struct Comparison {
    checkin: InspectionDetail,
    checkout: InspectionDetail,
    /// Suggested verdict per check-in damage: same zone + same type as a
    /// check-out damage reads as pre-existing, anything else as new. The
    /// inspector still confirms or dismisses each one.
    suggestions: Vec<Suggestion>,
}

#[derive(Serialize, ToSchema)]
struct Suggestion {
    checkin_damage_id: i64,
    checkout_damage_id: Option<i64>,
    suggested: String,
}

#[utoipa::path(
    get, path = "/inspections/{id}/comparison", tag = "inspections",
    params(("id" = i64, Path)),
    responses((status = 200, body = Comparison))
)]
async fn comparison(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Comparison>> {
    let checkin = inspections::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("inspection"))?;
    if checkin.kind != "checkin" {
        return Err(AppError::validation("only a check-in has a comparison"));
    }
    let checkout_id = checkin.checkout_id.ok_or(AppError::validation(
        "check-in is not linked to a check-out",
    ))?;
    let checkout = inspections::find(&state.db, checkout_id)
        .await?
        .ok_or(AppError::NotFound("inspection"))?;
    let checkin_detail = detail_for(&state, &checkin).await?;
    let checkout_detail = detail_for(&state, &checkout).await?;
    let mut suggestions = Vec::with_capacity(checkin_detail.damages.len());
    for damage in &checkin_detail.damages {
        let matched = checkout_detail
            .damages
            .iter()
            .find(|d| d.zone_key == damage.zone_key && d.damage_type == damage.damage_type);
        suggestions.push(Suggestion {
            checkin_damage_id: damage.id,
            checkout_damage_id: matched.map(|d| d.id),
            suggested: if matched.is_some() {
                "preexisting".to_string()
            } else {
                "new".to_string()
            },
        });
    }
    Ok(Json(Comparison {
        checkin: checkin_detail,
        checkout: checkout_detail,
        suggestions,
    }))
}

#[derive(Deserialize, ToSchema)]
struct VerdictBody {
    checkin_damage_id: i64,
    checkout_damage_id: Option<i64>,
    /// `preexisting` | `new` | `dismissed`.
    verdict: String,
    note: Option<String>,
}

#[utoipa::path(
    post, path = "/inspections/{id}/verdicts", tag = "inspections",
    params(("id" = i64, Path)),
    request_body(content = VerdictBody),
    responses((status = 200, body = InspectionVerdict))
)]
async fn set_verdict(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<VerdictBody>,
) -> AppResult<Json<InspectionVerdict>> {
    me.require(Capability::ChangeStages)?;
    let checkin = draft_or_locked(&state.db, id).await?;
    if checkin.kind != "checkin" {
        return Err(AppError::validation("only a check-in takes verdicts"));
    }
    in_list(&b.verdict, VERDICTS, "verdict")?;
    let damages = inspections::damages_for(&state.db, id).await?;
    if !damages.iter().any(|d| d.id == b.checkin_damage_id) {
        return Err(AppError::validation(
            "damage does not belong to this check-in",
        ));
    }
    if let Some(linked) = b.checkout_damage_id {
        let checkout_id = checkin.checkout_id.ok_or(AppError::validation(
            "check-in is not linked to a check-out",
        ))?;
        let checkout_damages = inspections::damages_for(&state.db, checkout_id).await?;
        if !checkout_damages.iter().any(|d| d.id == linked) {
            return Err(AppError::validation(
                "linked damage does not belong to the check-out",
            ));
        }
    }
    Ok(Json(
        inspections::set_verdict(
            &state.db,
            id,
            b.checkin_damage_id,
            b.checkout_damage_id,
            &b.verdict,
            super::optional(b.note).as_deref(),
            me.user_id,
        )
        .await?,
    ))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct TemplatesQuery {
    /// The order's project type (the kind of vehicle). Omitted: the general list.
    project_type_id: Option<i64>,
    /// `checkout` (the first walkaround, the vehicle arriving) or `checkin` (the second,
    /// leaving). Omitted: `checkout`.
    kind: Option<String>,
}

fn template_kind(kind: Option<String>) -> AppResult<String> {
    let kind = kind.unwrap_or_else(|| "checkout".to_string());
    in_list(&kind, KINDS, "kind")?;
    Ok(kind)
}

/// The zone list the phone walks: the project type's own list for this walkaround, else
/// the general one. Items carry `project_type_id`, so a client can tell an own list (set)
/// from the general list it was served instead (null).
#[utoipa::path(
    get, path = "/inspections/templates", tag = "inspections",
    params(TemplatesQuery),
    responses((status = 200, body = Items<ZoneTemplate>))
)]
async fn templates(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<TemplatesQuery>,
) -> AppResult<Json<Items<ZoneTemplate>>> {
    let kind = template_kind(q.kind)?;
    let mut conn = state.db.acquire().await?;
    Ok(Items::new(
        inspections::templates_for(&mut conn, q.project_type_id, &kind).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct ZoneBody {
    zone_key: String,
    position: i32,
    /// The short heading the phone shows for the zone.
    title: String,
    instruction: String,
    optional: bool,
    required: bool,
}

#[derive(Deserialize, ToSchema)]
struct ReplaceTemplatesBody {
    /// The project type the list is for; null replaces the general list.
    project_type_id: Option<i64>,
    /// `checkout` or `checkin`.
    kind: String,
    zones: Vec<ZoneBody>,
}

/// Replaces one whole list, in order. Admin only.
#[utoipa::path(
    put, path = "/inspections/templates", tag = "inspections",
    request_body(content = ReplaceTemplatesBody),
    responses((status = 200, body = Items<ZoneTemplate>))
)]
async fn replace_templates(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ReplaceTemplatesBody>,
) -> AppResult<Json<Items<ZoneTemplate>>> {
    me.require(Capability::ManageConfiguration)?;
    in_list(&b.kind, KINDS, "kind")?;
    if b.zones.is_empty() || b.zones.len() > MAX_ZONES {
        return Err(AppError::validation(format!(
            "zones must hold 1..{MAX_ZONES} entries"
        )));
    }
    let mut zones = Vec::with_capacity(b.zones.len());
    for z in b.zones {
        // One fact, two fields: the phone skips an `optional` zone and insists on the rest.
        if z.required == z.optional {
            return Err(AppError::validation(format!(
                "zone {}: required and optional must disagree: a zone is one or the other",
                z.zone_key.trim()
            )));
        }
        zones.push(inspections::NewZone {
            zone_key: super::required("zone_key", &z.zone_key)?,
            position: z.position,
            title: super::required("title", &z.title)?,
            instruction: super::required("instruction", &z.instruction)?,
            optional: z.optional,
            required: z.required,
        });
    }
    let mut tx = state.db.begin().await?;
    if let Some(id) = b.project_type_id {
        if !config::project_types(&mut *tx)
            .await?
            .iter()
            .any(|t| t.id == id)
        {
            return Err(AppError::NotFound("project type"));
        }
    }
    let out =
        inspections::replace_template_list(&mut tx, b.project_type_id, &b.kind, &zones).await?;
    tx.commit().await?;
    Ok(Items::new(out))
}

/// Removes a project type's own list, so it is served the general one again. The general
/// list cannot be removed: every walkaround needs a list to fall back to. Admin only.
#[utoipa::path(
    delete, path = "/inspections/templates", tag = "inspections",
    params(TemplatesQuery),
    responses((status = 204, description = "Removed"))
)]
async fn delete_templates(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<TemplatesQuery>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageConfiguration)?;
    let kind = template_kind(q.kind)?;
    if q.project_type_id.is_none() {
        return Err(AppError::validation(
            "the general list cannot be removed: replace it instead",
        ));
    }
    let mut tx = state.db.begin().await?;
    if !inspections::delete_template_list(&mut tx, q.project_type_id, &kind).await? {
        return Err(AppError::NotFound("zone list"));
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inspection(order_id: i64, kind: &str) -> Inspection {
        let now = Utc::now();
        Inspection {
            id: 1,
            order_id,
            kind: kind.into(),
            status: "draft".into(),
            vehicle_plate: "ABC-123".into(),
            vehicle_vin: None,
            inspector_name: "Sanyi".into(),
            driver_name: None,
            location: None,
            odometer: None,
            fuel_level: None,
            battery_pct: None,
            warning_lights: None,
            checkout_id: None,
            customer_comment: None,
            signed_at: None,
            created_by: 1,
            created_at: now,
            updated_at: now,
        }
    }

    // INSP-L10: the key is optional; blank means none, and it is bounded like the column.
    #[test]
    fn client_key_is_optional_trimmed_and_bounded() {
        assert_eq!(client_key(None).unwrap(), None);
        assert_eq!(client_key(Some("  ")).unwrap(), None);
        assert_eq!(client_key(Some(" u-1 ")).unwrap().as_deref(), Some("u-1"));
        assert!(client_key(Some(&"k".repeat(100))).is_ok());
        assert!(matches!(
            client_key(Some(&"k".repeat(101))),
            Err(AppError::Validation(_))
        ));
    }

    // INSP-L10: a retried create with the same key is the same request only for the
    // same order and kind; otherwise it must not return someone else's inspection.
    #[test]
    fn a_key_replay_must_name_the_same_order_and_kind() {
        let existing = inspection(7, "checkout");
        assert!(replay_matches(&existing, 7, "checkout").is_ok());
        assert!(matches!(
            replay_matches(&existing, 8, "checkout"),
            Err(AppError::Conflict {
                code: "duplicate",
                ..
            })
        ));
        assert!(matches!(
            replay_matches(&existing, 7, "checkin"),
            Err(AppError::Conflict {
                code: "duplicate",
                ..
            })
        ));
    }
}

// ── Tyres and videos (0048) ─────────────────────────────────────────────────

#[derive(Deserialize, ToSchema)]
struct TyresBody {
    /// The whole tyre record; replaces what was there. A position appears once.
    tyres: Vec<InspectionTyre>,
}

#[utoipa::path(
    put, path = "/inspections/{id}/tyres", tag = "inspections",
    params(("id" = i64, Path)),
    request_body = TyresBody,
    responses((status = 200, body = Vec<InspectionTyre>))
)]
async fn put_tyres(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<TyresBody>,
) -> AppResult<Json<Vec<InspectionTyre>>> {
    me.require(Capability::ChangeStages)?;
    draft_or_locked(&state.db, id).await?;
    let mut seen = std::collections::HashSet::new();
    let mut tyres = Vec::with_capacity(b.tyres.len());
    for mut t in b.tyres {
        in_list(&t.position, TYRE_POSITION_KEYS, "position")?;
        in_list(&t.condition, TYRE_CONDITION_KEYS, "condition")?;
        if !seen.insert(t.position.clone()) {
            return Err(AppError::validation(format!(
                "tyre position '{}' appears twice",
                t.position
            )));
        }
        if let Some(mm) = t.tread_mm
            && (mm.is_sign_negative() || mm > rust_decimal::Decimal::from(30))
        {
            return Err(AppError::validation("tread_mm must be between 0 and 30"));
        }
        t.tread_mm = t.tread_mm.map(|mm| mm.round_dp(1));
        t.note = super::optional(t.note);
        if t.note.as_deref().is_some_and(|n| n.chars().count() > 500) {
            return Err(AppError::validation(
                "a tyre note is at most 500 characters",
            ));
        }
        tyres.push(t);
    }
    let mut tx = state.db.begin().await?;
    inspections::replace_tyres(&mut tx, id, &tyres).await?;
    tx.commit().await?;
    Ok(Json(inspections::tyres_for(&state.db, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct VideoBody {
    /// A `video` document of the same order, uploaded through the normal flow.
    document_id: i64,
    zone_key: Option<String>,
    duration_ms: Option<i32>,
    taken_at: DateTime<Utc>,
}

#[utoipa::path(
    post, path = "/inspections/{id}/videos", tag = "inspections",
    params(("id" = i64, Path)),
    request_body = VideoBody,
    responses((status = 201, body = InspectionVideoRow))
)]
async fn attach_video(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<VideoBody>,
) -> AppResult<(StatusCode, Json<InspectionVideoRow>)> {
    me.require(Capability::ChangeStages)?;
    let inspection = draft_or_locked(&state.db, id).await?;
    let doc = documents::find(&state.db, b.document_id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    if !Owner::Order(inspection.order_id).owns(&doc) {
        return Err(AppError::validation("document belongs to another order"));
    }
    if doc.kind != DocumentKind::Video {
        return Err(AppError::validation("document is not a video"));
    }
    if b.duration_ms.is_some_and(|d| d < 0) {
        return Err(AppError::validation("duration_ms cannot be negative"));
    }
    let zone_key = super::optional(b.zone_key);
    if zone_key.as_deref().is_some_and(|z| z.chars().count() > 64) {
        return Err(AppError::validation("zone_key is at most 64 characters"));
    }
    let video = inspections::attach_video(
        &state.db,
        id,
        b.document_id,
        zone_key.as_deref(),
        b.duration_ms,
        b.taken_at,
    )
    .await?;
    if video.inspection_id != id {
        return Err(AppError::rule(
            "already_attached",
            "the video is already attached to another inspection",
        ));
    }
    Ok((StatusCode::CREATED, Json(video)))
}
