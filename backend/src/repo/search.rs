//! Global search: one input, three aggregates, one round trip.
//!
//! The header search box asks "which record is this about" — a plate shouted across the
//! yard, half a company name, an order number from an email, a phone number off a
//! display. Each aggregate searches its own identifying columns with the same ILIKE +
//! plate/phone-normalisation conventions as the list searches, capped per aggregate
//! so one chatty table cannot drown the others.
//! Ranking is recency within each group; cross-group ranking is the client's job.

use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::domain::order::normalize_plate;

/// One order hit: enough to recognise the job (number + plate + stage) and link to it.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderHit {
    pub id: i64,
    pub number: String,
    pub title: String,
    pub plate: Option<String>,
    pub stage_label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PartnerHit {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub city: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LeadHit {
    pub id: i64,
    pub title: String,
    pub contact_name: Option<String>,
    pub stage_label: String,
}

pub async fn orders(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<OrderHit>> {
    let pattern = crate::repo::like_pattern(q);
    let plate = normalize_plate(q);
    let plate = (!plate.is_empty()).then_some(format!("%{plate}%"));
    sqlx::query_as!(
        OrderHit,
        r#"SELECT o.id, o.number, o.title,
                  COALESCE(
                      (SELECT xveh.plate FROM order_vehicles xv
                        JOIN vehicles xveh ON xveh.id = xv.vehicle_id
                       WHERE xv.order_id = o.id ORDER BY xveh.plate LIMIT 1),
                      o.vehicle_plate
                  ) AS "plate?",
                  sd.label_hu AS "stage_label!"
           FROM orders o
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           JOIN partners p ON p.id = o.partner_id
           WHERE o.number ILIKE $1 OR o.title ILIKE $1 OR p.name ILIKE $1
              OR o.vehicle_vin ILIKE $1
              -- Legacy plate, normalised like the list search: "abc123" finds "ABC-123".
              OR ($2::text IS NOT NULL AND (
                     upper(regexp_replace(o.vehicle_plate, '[^A-Za-z0-9]', '', 'g')) LIKE $2
                  OR EXISTS (SELECT 1 FROM order_vehicles xv JOIN vehicles xveh ON xveh.id = xv.vehicle_id
                              WHERE xv.order_id = o.id
                                AND (xveh.plate_norm LIKE $2 OR xveh.vin ILIKE $1))))
           ORDER BY o.id DESC
           LIMIT $3"#,
        pattern,
        plate,
        limit
    )
    .fetch_all(db)
    .await
}

pub async fn partners(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<PartnerHit>> {
    let pattern = crate::repo::like_pattern(q);
    let phone = crate::repo::phone_pattern(q);
    sqlx::query_as!(
        PartnerHit,
        r#"SELECT id, name, kind::text AS "kind!", city
           FROM partners
           WHERE name ILIKE $1 OR city ILIKE $1 OR tax_number ILIKE $1
              -- Phone digits with Hungarian prefixes unified, mirroring
              -- domain::partner::normalize_phone.
              OR ($2::text IS NOT NULL AND regexp_replace(regexp_replace(regexp_replace(phone, '[^0-9]', '', 'g'), '^00', ''), '^06', '36') LIKE $2)
           ORDER BY id DESC
           LIMIT $3"#,
        pattern,
        phone,
        limit
    )
    .fetch_all(db)
    .await
}

pub async fn leads(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<LeadHit>> {
    let pattern = crate::repo::like_pattern(q);
    let phone = crate::repo::phone_pattern(q);
    sqlx::query_as!(
        LeadHit,
        r#"SELECT l.id, l.title, l.contact_name, sd.label_hu AS "stage_label!"
           FROM leads l
           JOIN lead_current_stage cs ON cs.lead_id = l.id
           JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
           LEFT JOIN partners p ON p.id = l.partner_id
           WHERE l.title ILIKE $1 OR l.contact_name ILIKE $1 OR l.contact_email ILIKE $1
              OR p.name ILIKE $1
              -- Contact phone, same normalisation as the lead list search.
              OR ($2::text IS NOT NULL AND regexp_replace(regexp_replace(regexp_replace(l.contact_phone, '[^0-9]', '', 'g'), '^00', ''), '^06', '36') LIKE $2)
           ORDER BY l.id DESC
           LIMIT $3"#,
        pattern,
        phone,
        limit
    )
    .fetch_all(db)
    .await
}
