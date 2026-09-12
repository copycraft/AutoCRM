//! The vehicle as an entity (V2.1).
//!
//! Matching is by normalised plate first, VIN second — the plate is what staff type and
//! what a customer quotes on the phone; the VIN is what is right when the plate has
//! changed. Both are nullable because thirty years of records contain one or the other.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor};
use utoipa::ToSchema;

use crate::domain::order::normalize_plate;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Vehicle {
    pub id: i64,
    pub vin: Option<String>,
    pub plate: Option<String>,
    /// Plate with spaces, dashes and case removed. Generated column; search matches on it.
    pub plate_norm: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i32>,
    pub partner_id: Option<i64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct VehicleFields {
    pub vin: Option<String>,
    pub plate: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i32>,
    pub partner_id: Option<i64>,
    pub notes: Option<String>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Vehicle>> {
    sqlx::query_as!(
        Vehicle,
        "SELECT id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at
           FROM vehicles WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

/// The vehicle this plate or VIN already refers to, if any. Plate wins: it is what people
/// search by, and a VIN typo should not merge two vans.
pub async fn find_existing(
    db: impl PgExecutor<'_>,
    plate: Option<&str>,
    vin: Option<&str>,
) -> sqlx::Result<Option<Vehicle>> {
    let plate_norm = plate.map(normalize_plate).filter(|p| !p.is_empty());
    let vin = vin
        .map(|v| v.trim().to_uppercase())
        .filter(|v| !v.is_empty());
    sqlx::query_as!(
        Vehicle,
        "SELECT id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at
           FROM vehicles
          WHERE ($1::text IS NOT NULL AND plate_norm = $1)
             OR ($2::text IS NOT NULL AND upper(vin) = $2)
          ORDER BY (plate_norm = $1) DESC, id
          LIMIT 1",
        plate_norm,
        vin
    )
    .fetch_optional(db)
    .await
}

pub async fn insert(db: impl PgExecutor<'_>, f: &VehicleFields) -> sqlx::Result<Vehicle> {
    sqlx::query_as!(
        Vehicle,
        "INSERT INTO vehicles (vin, plate, make, model, year, partner_id, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at",
        f.vin,
        f.plate,
        f.make,
        f.model,
        f.year,
        f.partner_id,
        f.notes
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    f: &VehicleFields,
) -> sqlx::Result<Option<Vehicle>> {
    sqlx::query_as!(
        Vehicle,
        "UPDATE vehicles SET vin = $2, plate = $3, make = $4, model = $5, year = $6,
                             partner_id = $7, notes = $8
          WHERE id = $1
         RETURNING id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at",
        id,
        f.vin,
        f.plate,
        f.make,
        f.model,
        f.year,
        f.partner_id,
        f.notes
    )
    .fetch_optional(db)
    .await
}

/// Fills in columns the stored vehicle is missing without overwriting anything it has.
/// Used when an order supplies a VIN for a van previously known only by its plate; never
/// the other way round, because the older record is the one staff have already corrected.
pub async fn enrich(
    db: impl PgExecutor<'_>,
    id: i64,
    f: &VehicleFields,
) -> sqlx::Result<Option<Vehicle>> {
    sqlx::query_as!(
        Vehicle,
        "UPDATE vehicles
            SET vin = coalesce(vin, $2), plate = coalesce(plate, $3), make = coalesce(make, $4),
                model = coalesce(model, $5), year = coalesce(year, $6),
                partner_id = coalesce(partner_id, $7)
          WHERE id = $1
         RETURNING id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at",
        id,
        f.vin,
        f.plate,
        f.make,
        f.model,
        f.year,
        f.partner_id
    )
    .fetch_optional(db)
    .await
}

/// Finds or creates the vehicle these fields describe. `None` when there is neither a plate
/// nor a VIN — a make and model alone do not identify a van.
pub async fn upsert(conn: &mut PgConnection, f: &VehicleFields) -> sqlx::Result<Option<Vehicle>> {
    let has_plate = f.plate.as_deref().is_some_and(|p| !p.trim().is_empty());
    let has_vin = f.vin.as_deref().is_some_and(|v| !v.trim().is_empty());
    if !has_plate && !has_vin {
        return Ok(None);
    }
    if let Some(existing) = find_existing(&mut *conn, f.plate.as_deref(), f.vin.as_deref()).await? {
        return enrich(&mut *conn, existing.id, f).await;
    }
    insert(&mut *conn, f).await.map(Some)
}

pub async fn attach(db: impl PgExecutor<'_>, order_id: i64, vehicle_id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO order_vehicles (order_id, vehicle_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
        order_id,
        vehicle_id
    )
    .execute(db)
    .await
    .map(|_| ())
}

pub async fn detach(db: impl PgExecutor<'_>, order_id: i64, vehicle_id: i64) -> sqlx::Result<u64> {
    let done = sqlx::query!(
        "DELETE FROM order_vehicles WHERE order_id = $1 AND vehicle_id = $2",
        order_id,
        vehicle_id
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

pub async fn list_for_order(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<Vehicle>> {
    sqlx::query_as!(
        Vehicle,
        "SELECT v.id, v.vin, v.plate, v.plate_norm, v.make, v.model, v.year, v.partner_id, v.notes,
                v.created_at, v.updated_at
           FROM vehicles v JOIN order_vehicles ov ON ov.vehicle_id = v.id
          WHERE ov.order_id = $1
          ORDER BY v.id",
        order_id
    )
    .fetch_all(db)
    .await
}

/// Orders this vehicle has been through, newest first. The answer to "has this van been
/// here before", which is the whole point of the entity.
pub async fn orders_for_vehicle(
    db: impl PgExecutor<'_>,
    vehicle_id: i64,
) -> sqlx::Result<Vec<(i64, String, String)>> {
    let rows = sqlx::query!(
        "SELECT o.id, o.number, o.title
           FROM orders o JOIN order_vehicles ov ON ov.order_id = o.id
          WHERE ov.vehicle_id = $1
          ORDER BY o.created_at DESC, o.id DESC",
        vehicle_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.id, r.number, r.title))
        .collect())
}

pub async fn search(
    db: impl PgExecutor<'_>,
    q: Option<&str>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<Vehicle>> {
    let pattern = q.and_then(crate::repo::like_pattern);
    let plate = q.map(normalize_plate).filter(|p| !p.is_empty());
    sqlx::query_as!(
        Vehicle,
        "SELECT id, vin, plate, plate_norm, make, model, year, partner_id, notes, created_at, updated_at
           FROM vehicles
          WHERE $1::text IS NULL
             OR make ILIKE $1 OR model ILIKE $1 OR vin ILIKE $1 OR notes ILIKE $1
             OR ($2::text IS NOT NULL AND plate_norm LIKE '%' || $2 || '%')
          ORDER BY id DESC
          LIMIT $3 OFFSET $4",
        pattern,
        plate,
        limit,
        offset
    )
    .fetch_all(db)
    .await
}
