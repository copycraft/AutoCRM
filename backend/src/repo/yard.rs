//! The yard (0048): the places a vehicle can stand, and the moves between them. Where a
//! vehicle is now is its latest move (`vehicle_current_location`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct YardLocation {
    pub id: i64,
    pub name: String,
    /// `bay` | `parking` | `external`.
    pub kind: String,
    /// How many vehicles fit; null for no limit. The board warns, never refuses.
    pub capacity: Option<i32>,
    pub position: i32,
    pub archived_at: Option<DateTime<Utc>>,
}

pub async fn locations(
    db: impl PgExecutor<'_>,
    include_archived: bool,
) -> sqlx::Result<Vec<YardLocation>> {
    sqlx::query_as!(
        YardLocation,
        "SELECT id, name, kind, capacity, position, archived_at FROM yard_locations
         WHERE $1 OR archived_at IS NULL ORDER BY position, name",
        include_archived
    )
    .fetch_all(db)
    .await
}

pub async fn location(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<YardLocation>> {
    sqlx::query_as!(
        YardLocation,
        "SELECT id, name, kind, capacity, position, archived_at FROM yard_locations WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn insert_location(
    db: impl PgExecutor<'_>,
    name: &str,
    kind: &str,
    capacity: Option<i32>,
    position: i32,
) -> sqlx::Result<YardLocation> {
    sqlx::query_as!(
        YardLocation,
        "INSERT INTO yard_locations (name, kind, capacity, position) VALUES ($1, $2, $3, $4)
         RETURNING id, name, kind, capacity, position, archived_at",
        name,
        kind,
        capacity,
        position
    )
    .fetch_one(db)
    .await
}

pub async fn update_location(
    db: impl PgExecutor<'_>,
    id: i64,
    name: &str,
    kind: &str,
    capacity: Option<i32>,
    position: i32,
    archived: bool,
) -> sqlx::Result<Option<YardLocation>> {
    sqlx::query_as!(
        YardLocation,
        "UPDATE yard_locations
            SET name = $2, kind = $3, capacity = $4, position = $5,
                archived_at = CASE WHEN $6 THEN coalesce(archived_at, now()) ELSE NULL END
          WHERE id = $1
         RETURNING id, name, kind, capacity, position, archived_at",
        id,
        name,
        kind,
        capacity,
        position,
        archived
    )
    .fetch_optional(db)
    .await
}

/// A vehicle on the board: what it is, which job it is here for, and where it stands.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct YardVehicle {
    pub vehicle_id: i64,
    pub plate: Option<String>,
    pub vin: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    /// The open job it belongs to (the newest, when it has several).
    pub order_id: Option<i64>,
    pub order_number: Option<String>,
    pub order_title: Option<String>,
    pub partner_name: Option<String>,
    pub stage_key: Option<String>,
    pub stage_label: Option<String>,
    pub due_date: Option<chrono::NaiveDate>,
    /// Null: not placed anywhere yet.
    pub location_id: Option<i64>,
    pub moved_at: Option<DateTime<Utc>>,
    pub moved_by_name: Option<String>,
}

/// Every vehicle the board shows: those of open jobs, and any other still standing on a
/// place (a finished van waiting for pickup keeps its spot until it is moved off).
pub async fn board_vehicles(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<YardVehicle>> {
    sqlx::query_as!(
        YardVehicle,
        r#"WITH open_jobs AS (
               SELECT DISTINCT ON (ov.vehicle_id) ov.vehicle_id, o.id, o.number, o.title, o.due_date,
                      p.name AS partner_name, cs.stage_key, sd.label_hu
               FROM order_vehicles ov
               JOIN orders o ON o.id = ov.order_id
               JOIN partners p ON p.id = o.partner_id
               JOIN order_current_stage cs ON cs.order_id = o.id
               JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
               WHERE NOT sd.is_terminal
               ORDER BY ov.vehicle_id, o.created_at DESC, o.id DESC
           ), last_jobs AS (
               SELECT DISTINCT ON (ov.vehicle_id) ov.vehicle_id, o.id, o.number, o.title, o.due_date,
                      p.name AS partner_name, cs.stage_key, sd.label_hu
               FROM order_vehicles ov
               JOIN orders o ON o.id = ov.order_id
               JOIN partners p ON p.id = o.partner_id
               JOIN order_current_stage cs ON cs.order_id = o.id
               JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
               ORDER BY ov.vehicle_id, o.created_at DESC, o.id DESC
           )
           SELECT v.id AS vehicle_id, v.plate, v.vin, v.make, v.model,
                  coalesce(oj.id, lj.id) AS "order_id?",
                  coalesce(oj.number, lj.number) AS "order_number?",
                  coalesce(oj.title, lj.title) AS "order_title?",
                  coalesce(oj.partner_name, lj.partner_name) AS "partner_name?",
                  coalesce(oj.stage_key, lj.stage_key) AS "stage_key?",
                  coalesce(oj.label_hu, lj.label_hu) AS "stage_label?",
                  coalesce(oj.due_date, lj.due_date) AS "due_date?",
                  cl.location_id AS "location_id?",
                  cl.moved_at AS "moved_at?",
                  u.display_name AS "moved_by_name?"
           FROM vehicles v
           LEFT JOIN open_jobs oj ON oj.vehicle_id = v.id
           LEFT JOIN last_jobs lj ON lj.vehicle_id = v.id
           LEFT JOIN vehicle_current_location cl ON cl.vehicle_id = v.id
           LEFT JOIN users u ON u.id = cl.moved_by
           WHERE oj.vehicle_id IS NOT NULL OR cl.location_id IS NOT NULL
           ORDER BY coalesce(oj.due_date, lj.due_date) NULLS LAST, v.plate NULLS LAST, v.id"#
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VehicleMove {
    pub id: i64,
    pub vehicle_id: i64,
    pub location_id: Option<i64>,
    pub location_name: Option<String>,
    pub order_id: Option<i64>,
    pub note: Option<String>,
    pub moved_by: Option<i64>,
    pub moved_by_name: Option<String>,
    pub moved_at: DateTime<Utc>,
}

pub async fn insert_move(
    db: impl PgExecutor<'_>,
    vehicle_id: i64,
    location_id: Option<i64>,
    order_id: Option<i64>,
    note: Option<&str>,
    user_id: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO vehicle_moves (vehicle_id, location_id, order_id, note, moved_by)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
        vehicle_id,
        location_id,
        order_id,
        note,
        user_id
    )
    .fetch_one(db)
    .await
}

pub async fn moves_of(
    db: impl PgExecutor<'_>,
    vehicle_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<VehicleMove>> {
    sqlx::query_as!(
        VehicleMove,
        r#"SELECT m.id, m.vehicle_id, m.location_id, l.name AS "location_name?", m.order_id, m.note,
                  m.moved_by, u.display_name AS "moved_by_name?", m.moved_at
           FROM vehicle_moves m
           LEFT JOIN yard_locations l ON l.id = m.location_id
           LEFT JOIN users u ON u.id = m.moved_by
           WHERE m.vehicle_id = $1
           ORDER BY m.moved_at DESC, m.id DESC LIMIT $2"#,
        vehicle_id,
        limit
    )
    .fetch_all(db)
    .await
}

/// Where each of these vehicles stands now.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CurrentLocation {
    pub vehicle_id: i64,
    pub location_id: Option<i64>,
    pub location_name: Option<String>,
    pub moved_at: DateTime<Utc>,
    pub moved_by_name: Option<String>,
}

pub async fn current_of(
    db: impl PgExecutor<'_>,
    vehicle_ids: &[i64],
) -> sqlx::Result<Vec<CurrentLocation>> {
    sqlx::query_as!(
        CurrentLocation,
        r#"SELECT cl.vehicle_id AS "vehicle_id!", cl.location_id, l.name AS "location_name?",
                  cl.moved_at AS "moved_at!", u.display_name AS "moved_by_name?"
           FROM vehicle_current_location cl
           LEFT JOIN yard_locations l ON l.id = cl.location_id
           LEFT JOIN users u ON u.id = cl.moved_by
           WHERE cl.vehicle_id = ANY($1)"#,
        vehicle_ids
    )
    .fetch_all(db)
    .await
}
