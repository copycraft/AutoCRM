//! Vehicle handover inspections (átadás-átvétel): the rental-company style
//! check-out / check-in damage record, order-linked.
//!
//! Photos live in `images` (category `inspection`) and are attached here with zone +
//! purpose + capture metadata; signatures are finger-drawn PNGs in `documents`.
//! A signed row is locked by the API layer — every mutation here assumes the
//! caller already checked `status = 'draft'`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Inspection {
    pub id: i64,
    pub order_id: i64,
    pub kind: String,
    pub status: String,
    pub vehicle_plate: String,
    pub vehicle_vin: Option<String>,
    pub inspector_name: String,
    pub driver_name: Option<String>,
    pub location: Option<String>,
    pub odometer: Option<i32>,
    pub fuel_level: Option<String>,
    pub battery_pct: Option<i32>,
    pub warning_lights: Option<String>,
    pub checkout_id: Option<i64>,
    pub customer_comment: Option<String>,
    pub signed_at: Option<DateTime<Utc>>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewInspection {
    pub order_id: i64,
    pub kind: String,
    pub vehicle_plate: String,
    pub vehicle_vin: Option<String>,
    pub inspector_name: String,
    pub driver_name: Option<String>,
    pub location: Option<String>,
    pub odometer: Option<i32>,
    pub fuel_level: Option<String>,
    pub battery_pct: Option<i32>,
    pub warning_lights: Option<String>,
    pub checkout_id: Option<i64>,
    pub created_by: i64,
}

pub async fn create(db: impl PgExecutor<'_>, n: &NewInspection) -> sqlx::Result<Inspection> {
    sqlx::query_as!(
        Inspection,
        r#"INSERT INTO inspections (order_id, kind, vehicle_plate, vehicle_vin, inspector_name,
                                    driver_name, location, odometer, fuel_level, battery_pct,
                                    warning_lights, checkout_id, created_by)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                      driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                      checkout_id, customer_comment, signed_at, created_by, created_at, updated_at"#,
        n.order_id,
        n.kind,
        n.vehicle_plate,
        n.vehicle_vin,
        n.inspector_name,
        n.driver_name,
        n.location,
        n.odometer,
        n.fuel_level,
        n.battery_pct,
        n.warning_lights,
        n.checkout_id,
        n.created_by
    )
    .fetch_one(db)
    .await
}

/// The inspection a client already created under this idempotency key, if any.
/// Runtime query (no offline-cache entry needed); the row itself comes from [`find`].
pub async fn find_by_client_key(
    db: &sqlx::PgPool,
    client_key: &str,
) -> sqlx::Result<Option<Inspection>> {
    let id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM inspections WHERE client_key = $1")
            .bind(client_key)
            .fetch_optional(db)
            .await?;
    match id {
        Some(id) => find(db, id).await,
        None => Ok(None),
    }
}

/// Idempotent create: inserts the inspection and records `client_key` in one
/// transaction. When the key is already taken (a retried create whose first
/// response was lost, or two racing retries), nothing new is written and the
/// existing row comes back with `created = false`.
///
/// Any other error (e.g. the one-draft-checkout index) is returned unchanged;
/// the caller translates it.
pub async fn create_with_client_key(
    db: &sqlx::PgPool,
    n: &NewInspection,
    client_key: &str,
) -> sqlx::Result<(Inspection, bool)> {
    if let Some(existing) = find_by_client_key(db, client_key).await? {
        return Ok((existing, false));
    }
    let attempt: sqlx::Result<Inspection> = async {
        let mut tx = db.begin().await?;
        let inspection = create(&mut *tx, n).await?;
        sqlx::query("UPDATE inspections SET client_key = $2 WHERE id = $1")
            .bind(inspection.id)
            .bind(client_key)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(inspection)
    }
    .await;
    match attempt {
        Ok(inspection) => Ok((inspection, true)),
        Err(e) => {
            // Lost a race against a concurrent retry with the same key: its row is
            // the answer. (For a check-out the loser may also trip the
            // one-draft-checkout index first; the key lookup covers both.)
            if let Some(existing) = find_by_client_key(db, client_key).await? {
                Ok((existing, false))
            } else {
                Err(e)
            }
        }
    }
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"SELECT id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                  driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                  checkout_id, customer_comment, signed_at, created_by, created_at, updated_at
            FROM inspections WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"SELECT id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                  driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                  checkout_id, customer_comment, signed_at, created_by, created_at, updated_at
            FROM inspections WHERE order_id = $1 ORDER BY created_at DESC, id DESC"#,
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn list_for_plate(
    db: impl PgExecutor<'_>,
    plate: &str,
) -> sqlx::Result<Vec<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"SELECT id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                  driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                  checkout_id, customer_comment, signed_at, created_by, created_at, updated_at
            FROM inspections WHERE vehicle_plate = $1 ORDER BY created_at DESC, id DESC"#,
        plate
    )
    .fetch_all(db)
    .await
}

/// The latest signed check-out of an order: what a new check-in compares against.
pub async fn latest_signed_checkout(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Option<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"SELECT id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                  driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                  checkout_id, customer_comment, signed_at, created_by, created_at, updated_at
            FROM inspections
            WHERE order_id = $1 AND kind = 'checkout' AND status = 'signed'
            ORDER BY signed_at DESC NULLS LAST, id DESC LIMIT 1"#,
        order_id
    )
    .fetch_optional(db)
    .await
}

pub async fn patch_draft(
    db: impl PgExecutor<'_>,
    id: i64,
    inspector_name: Option<&str>,
    driver_name: Option<&str>,
    location: Option<&str>,
    odometer: Option<i32>,
    fuel_level: Option<&str>,
    battery_pct: Option<i32>,
    warning_lights: Option<&str>,
    customer_comment: Option<&str>,
) -> sqlx::Result<Option<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"UPDATE inspections SET
               inspector_name = COALESCE($2, inspector_name),
               driver_name = COALESCE($3, driver_name),
               location = COALESCE($4, location),
               odometer = COALESCE($5, odometer),
               fuel_level = COALESCE($6, fuel_level),
               battery_pct = COALESCE($7, battery_pct),
               warning_lights = COALESCE($8, warning_lights),
               customer_comment = COALESCE($9, customer_comment)
            WHERE id = $1 AND status = 'draft'
            RETURNING id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                      driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                      checkout_id, customer_comment, signed_at, created_by, created_at, updated_at"#,
        id,
        inspector_name,
        driver_name,
        location,
        odometer,
        fuel_level,
        battery_pct,
        warning_lights,
        customer_comment
    )
    .fetch_optional(db)
    .await
}

pub async fn sign(
    db: impl PgExecutor<'_>,
    id: i64,
    customer_comment: Option<&str>,
) -> sqlx::Result<Option<Inspection>> {
    sqlx::query_as!(
        Inspection,
        r#"UPDATE inspections SET status = 'signed', signed_at = now(),
               customer_comment = COALESCE($2, customer_comment)
            WHERE id = $1 AND status = 'draft'
            RETURNING id, order_id, kind, status, vehicle_plate, vehicle_vin, inspector_name,
                      driver_name, location, odometer, fuel_level, battery_pct, warning_lights,
                      checkout_id, customer_comment, signed_at, created_by, created_at, updated_at"#,
        id,
        customer_comment
    )
    .fetch_optional(db)
    .await
}

/// Drafts can be discarded; signed rows live forever (annotations go to notes).
pub async fn remove_draft(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!("DELETE FROM inspections WHERE id = $1 AND status = 'draft'", id)
        .execute(db)
        .await?;
    Ok(r.rows_affected() > 0)
}

// ── Photos ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InspectionPhoto {
    pub id: i64,
    pub inspection_id: i64,
    pub image_id: i64,
    pub zone_key: String,
    pub purpose: String,
    pub damage_id: Option<i64>,
    pub taken_at: DateTime<Utc>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub created_at: DateTime<Utc>,
}

pub async fn attach_photo(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
    image_id: i64,
    zone_key: &str,
    purpose: &str,
    damage_id: Option<i64>,
    taken_at: DateTime<Utc>,
    lat: Option<f64>,
    lon: Option<f64>,
) -> sqlx::Result<InspectionPhoto> {
    sqlx::query_as!(
        InspectionPhoto,
        r#"INSERT INTO inspection_photos (inspection_id, image_id, zone_key, purpose, damage_id,
                                          taken_at, lat, lon)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, inspection_id, image_id, zone_key, purpose, damage_id,
                      taken_at, lat, lon, created_at"#,
        inspection_id,
        image_id,
        zone_key,
        purpose,
        damage_id,
        taken_at,
        lat,
        lon
    )
    .fetch_one(db)
    .await
}

pub async fn photos_for(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
) -> sqlx::Result<Vec<InspectionPhoto>> {
    sqlx::query_as!(
        InspectionPhoto,
        r#"SELECT id, inspection_id, image_id, zone_key, purpose, damage_id,
                  taken_at, lat, lon, created_at
            FROM inspection_photos WHERE inspection_id = $1 ORDER BY created_at, id"#,
        inspection_id
    )
    .fetch_all(db)
    .await
}

// ── Damages ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InspectionDamage {
    pub id: i64,
    pub inspection_id: i64,
    pub zone_key: String,
    pub damage_type: String,
    pub severity: String,
    pub note: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub view: String,
    pub created_at: DateTime<Utc>,
}

pub struct NewDamage {
    pub inspection_id: i64,
    pub zone_key: String,
    pub damage_type: String,
    pub severity: String,
    pub note: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub view: String,
}

pub async fn add_damage(db: impl PgExecutor<'_>, n: &NewDamage) -> sqlx::Result<InspectionDamage> {
    sqlx::query_as!(
        InspectionDamage,
        r#"INSERT INTO inspection_damages (inspection_id, zone_key, damage_type, severity, note, x, y, view)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, inspection_id, zone_key, damage_type, severity, note, x, y, view, created_at"#,
        n.inspection_id,
        n.zone_key,
        n.damage_type,
        n.severity,
        n.note,
        n.x,
        n.y,
        n.view
    )
    .fetch_one(db)
    .await
}

pub async fn damages_for(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
) -> sqlx::Result<Vec<InspectionDamage>> {
    sqlx::query_as!(
        InspectionDamage,
        r#"SELECT id, inspection_id, zone_key, damage_type, severity, note, x, y, view, created_at
            FROM inspection_damages WHERE inspection_id = $1 ORDER BY created_at, id"#,
        inspection_id
    )
    .fetch_all(db)
    .await
}

pub async fn remove_damage(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
    id: i64,
) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "DELETE FROM inspection_damages WHERE id = $1 AND inspection_id = $2",
        id,
        inspection_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() > 0)
}

// ── Check-in verdicts ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InspectionVerdict {
    pub id: i64,
    pub checkin_id: i64,
    pub checkin_damage_id: i64,
    pub checkout_damage_id: Option<i64>,
    pub verdict: String,
    pub note: Option<String>,
    pub reviewed_by: i64,
    pub reviewed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn set_verdict(
    db: impl PgExecutor<'_>,
    checkin_id: i64,
    checkin_damage_id: i64,
    checkout_damage_id: Option<i64>,
    verdict: &str,
    note: Option<&str>,
    reviewed_by: i64,
) -> sqlx::Result<InspectionVerdict> {
    sqlx::query_as!(
        InspectionVerdict,
        r#"INSERT INTO inspection_verdicts (checkin_id, checkin_damage_id, checkout_damage_id,
                                            verdict, note, reviewed_by)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (checkin_damage_id) DO UPDATE SET
                checkout_damage_id = EXCLUDED.checkout_damage_id,
                verdict = EXCLUDED.verdict,
                note = EXCLUDED.note,
                reviewed_by = EXCLUDED.reviewed_by,
                reviewed_at = now()
            RETURNING id, checkin_id, checkin_damage_id, checkout_damage_id, verdict,
                      note, reviewed_by, reviewed_at, created_at"#,
        checkin_id,
        checkin_damage_id,
        checkout_damage_id,
        verdict,
        note,
        reviewed_by
    )
    .fetch_one(db)
    .await
}

pub async fn verdicts_for(
    db: impl PgExecutor<'_>,
    checkin_id: i64,
) -> sqlx::Result<Vec<InspectionVerdict>> {
    sqlx::query_as!(
        InspectionVerdict,
        r#"SELECT id, checkin_id, checkin_damage_id, checkout_damage_id, verdict,
                  note, reviewed_by, reviewed_at, created_at
            FROM inspection_verdicts WHERE checkin_id = $1"#,
        checkin_id
    )
    .fetch_all(db)
    .await
}

// ── Signatures ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InspectionSignature {
    pub id: i64,
    pub inspection_id: i64,
    pub role: String,
    pub name: String,
    pub document_id: i64,
    pub signed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub async fn add_signature(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
    role: &str,
    name: &str,
    document_id: i64,
) -> sqlx::Result<InspectionSignature> {
    sqlx::query_as!(
        InspectionSignature,
        r#"INSERT INTO inspection_signatures (inspection_id, role, name, document_id)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (inspection_id, role) DO UPDATE SET
                name = EXCLUDED.name, document_id = EXCLUDED.document_id, signed_at = now()
            RETURNING id, inspection_id, role, name, document_id, signed_at, created_at"#,
        inspection_id,
        role,
        name,
        document_id
    )
    .fetch_one(db)
    .await
}

pub async fn signatures_for(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
) -> sqlx::Result<Vec<InspectionSignature>> {
    sqlx::query_as!(
        InspectionSignature,
        r#"SELECT id, inspection_id, role, name, document_id, signed_at, created_at
            FROM inspection_signatures WHERE inspection_id = $1 ORDER BY role"#,
        inspection_id
    )
    .fetch_all(db)
    .await
}

// ── Follow-up notes ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct InspectionNote {
    pub id: i64,
    pub inspection_id: i64,
    pub body: String,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
}

pub async fn add_note(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
    body: &str,
    created_by: i64,
) -> sqlx::Result<InspectionNote> {
    sqlx::query_as!(
        InspectionNote,
        r#"INSERT INTO inspection_notes (inspection_id, body, created_by)
            VALUES ($1, $2, $3)
            RETURNING id, inspection_id, body, created_by, created_at"#,
        inspection_id,
        body,
        created_by
    )
    .fetch_one(db)
    .await
}

pub async fn notes_for(
    db: impl PgExecutor<'_>,
    inspection_id: i64,
) -> sqlx::Result<Vec<InspectionNote>> {
    sqlx::query_as!(
        InspectionNote,
        r#"SELECT id, inspection_id, body, created_by, created_at
            FROM inspection_notes WHERE inspection_id = $1 ORDER BY created_at, id"#,
        inspection_id
    )
    .fetch_all(db)
    .await
}

// ── Zone templates ──

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ZoneTemplate {
    pub id: i64,
    pub set_key: String,
    pub zone_key: String,
    pub position: i32,
    pub instruction: String,
    pub optional: bool,
    pub required: bool,
}

pub async fn templates_for_sets(
    db: impl PgExecutor<'_>,
    sets: &[String],
) -> sqlx::Result<Vec<ZoneTemplate>> {
    sqlx::query_as!(
        ZoneTemplate,
        r#"SELECT id, set_key, zone_key, position, instruction, optional, required
            FROM inspection_zone_templates WHERE set_key = ANY($1)
            ORDER BY position, id"#,
        sets
    )
    .fetch_all(db)
    .await
}

pub struct NewZone {
    pub zone_key: String,
    pub position: i32,
    pub instruction: String,
    pub optional: bool,
    pub required: bool,
}

/// Admin replace of a whole set: delete + insert inside the caller's transaction.
pub async fn replace_template_set(
    db: &mut sqlx::PgConnection,
    set_key: &str,
    zones: &[NewZone],
) -> sqlx::Result<Vec<ZoneTemplate>> {
    sqlx::query!(
        "DELETE FROM inspection_zone_templates WHERE set_key = $1",
        set_key
    )
    .execute(&mut *db)
    .await?;
    let mut out = Vec::with_capacity(zones.len());
    for z in zones {
        let row = sqlx::query_as!(
            ZoneTemplate,
            r#"INSERT INTO inspection_zone_templates (set_key, zone_key, position, instruction, optional, required)
                VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING id, set_key, zone_key, position, instruction, optional, required"#,
            set_key,
            z.zone_key,
            z.position,
            z.instruction,
            z.optional,
            z.required
        )
        .fetch_one(&mut *db)
        .await?;
        out.push(row);
    }
    Ok(out)
}
