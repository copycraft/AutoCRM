use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};

use crate::domain::order::format_order_number;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Order {
    pub id: i64,
    pub number: String,
    pub title: String,
    pub partner_id: i64,
    pub contact_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub project_type_id: Option<i64>,
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    pub valuation_date: NaiveDate,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_vin: Option<String>,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub assigned_to: Option<i64>,
    /// V2.2: the job this one repairs or repeats. Warranty and rework work is otherwise
    /// unlinkable to the job it fixes, and nobody reconstructs that afterwards.
    pub related_order_id: Option<i64>,
    /// 'warranty' | 'rework' | 'repeat'. Set together with related_order_id or not at all.
    pub relation: Option<String>,
    /// Intake slip (átvételi lap): odometer at takeover. Leaving `intake` requires it.
    pub mileage_in: Option<i32>,
    /// Intake slip: visible condition notes at takeover.
    pub intake_condition: Option<String>,
    /// Intake slip: fuel level at takeover (`E` | `1/4` | `1/2` | `3/4` | `F`).
    pub fuel_level: Option<String>,
    /// Intake slip: how many keys were handed over.
    pub key_count: Option<i32>,
    /// Intake slip: whether the valuables question was answered at all, and how.
    /// `None` means nobody asked; `Some(false)` means the vehicle was empty.
    pub valuables_declared: Option<bool>,
    /// Intake slip: what was left in the vehicle, when [`Self::valuables_declared`] is true.
    pub valuables: Option<String>,
    pub created_by: Option<i64>,
    pub minicrm_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct OrderFields {
    pub title: String,
    pub partner_id: i64,
    pub contact_id: Option<i64>,
    pub project_type_id: Option<i64>,
    pub currency: String,
    pub valuation_date: NaiveDate,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_plate: Option<String>,
    pub vehicle_vin: Option<String>,
    pub description: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub assigned_to: Option<i64>,
    pub related_order_id: Option<i64>,
    pub relation: Option<String>,
    pub mileage_in: Option<i32>,
    pub intake_condition: Option<String>,
    pub fuel_level: Option<String>,
    pub key_count: Option<i32>,
    pub valuables_declared: Option<bool>,
    pub valuables: Option<String>,
}

impl From<&Order> for OrderFields {
    fn from(o: &Order) -> Self {
        OrderFields {
            title: o.title.clone(),
            partner_id: o.partner_id,
            contact_id: o.contact_id,
            project_type_id: o.project_type_id,
            currency: o.currency.clone(),
            valuation_date: o.valuation_date,
            vehicle_make: o.vehicle_make.clone(),
            vehicle_model: o.vehicle_model.clone(),
            vehicle_plate: o.vehicle_plate.clone(),
            vehicle_vin: o.vehicle_vin.clone(),
            description: o.description.clone(),
            due_date: o.due_date,
            assigned_to: o.assigned_to,
            related_order_id: o.related_order_id,
            relation: o.relation.clone(),
            mileage_in: o.mileage_in,
            intake_condition: o.intake_condition.clone(),
            fuel_level: o.fuel_level.clone(),
            key_count: o.key_count,
            valuables_declared: o.valuables_declared,
            valuables: o.valuables.clone(),
        }
    }
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Order>> {
    sqlx::query_as!(
        Order,
        "SELECT id, number, title, partner_id, contact_id, lead_id, project_type_id, currency, valuation_date,
                vehicle_make, vehicle_model, vehicle_plate, vehicle_vin, description, due_date, assigned_to,
                related_order_id, relation, mileage_in, intake_condition,
                fuel_level, key_count, valuables_declared, valuables, created_by, minicrm_id, created_at, updated_at
         FROM orders WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

/// Row-locks the order for the rest of the transaction. Every change to an order's stage,
/// items or blockers takes this lock first, so concurrent edits serialize.
pub async fn lock(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Order>> {
    sqlx::query_as!(
        Order,
        "SELECT id, number, title, partner_id, contact_id, lead_id, project_type_id, currency, valuation_date,
                vehicle_make, vehicle_model, vehicle_plate, vehicle_vin, description, due_date, assigned_to,
                related_order_id, relation, mileage_in, intake_condition,
                fuel_level, key_count, valuables_declared, valuables, created_by, minicrm_id, created_at, updated_at
         FROM orders WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

/// Every order converted from this lead (V2.7). Plural since one enquiry for three
/// identical vans becomes three orders, each keeping its origin.
pub async fn find_by_lead(
    db: impl PgExecutor<'_>,
    lead_id: i64,
) -> sqlx::Result<Vec<(i64, String)>> {
    let rows = sqlx::query!(
        "SELECT id, number FROM orders WHERE lead_id = $1 ORDER BY id",
        lead_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.id, r.number)).collect())
}

/// Next `YYYY-NNNN` number. Takes a transaction-scoped advisory lock so two orders created
/// at the same moment can't draw the same number.
pub async fn next_number(conn: &mut PgConnection, year: i32) -> sqlx::Result<String> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('autocrm.order_number'))")
        .execute(&mut *conn)
        .await?;
    let max: Option<i32> = sqlx::query_scalar!(
        r#"SELECT max(split_part(number, '-', 2)::int) FROM orders WHERE number ~ ('^' || $1::text || '-[0-9]{4,9}$')"#,
        year.to_string()
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(format_order_number(year, max.unwrap_or(0) + 1))
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    number: &str,
    f: &OrderFields,
    lead_id: Option<i64>,
    created_by: i64,
) -> sqlx::Result<Order> {
    sqlx::query_as!(
        Order,
        "INSERT INTO orders (number, title, partner_id, contact_id, lead_id, project_type_id, currency, valuation_date,
                             vehicle_make, vehicle_model, vehicle_plate, vehicle_vin, description, due_date, assigned_to,
                             related_order_id, relation, mileage_in, intake_condition,
                             fuel_level, key_count, valuables_declared, valuables, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
         RETURNING id, number, title, partner_id, contact_id, lead_id, project_type_id, currency, valuation_date,
                   vehicle_make, vehicle_model, vehicle_plate, vehicle_vin, description, due_date, assigned_to,
                   related_order_id, relation, mileage_in, intake_condition,
                fuel_level, key_count, valuables_declared, valuables, created_by, minicrm_id, created_at, updated_at",
        number,
        f.title,
        f.partner_id,
        f.contact_id,
        lead_id,
        f.project_type_id,
        f.currency,
        f.valuation_date,
        f.vehicle_make,
        f.vehicle_model,
        f.vehicle_plate,
        f.vehicle_vin,
        f.description,
        f.due_date,
        f.assigned_to,
        f.related_order_id,
        f.relation,
        f.mileage_in,
        f.intake_condition,
        f.fuel_level,
        f.key_count,
        f.valuables_declared,
        f.valuables,
        created_by
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    f: &OrderFields,
) -> sqlx::Result<Option<Order>> {
    sqlx::query_as!(
        Order,
        "UPDATE orders
         SET title = $2, partner_id = $3, contact_id = $4, project_type_id = $5, currency = $6, valuation_date = $7,
             vehicle_make = $8, vehicle_model = $9, vehicle_plate = $10, vehicle_vin = $11, description = $12,
             due_date = $13, assigned_to = $14, related_order_id = $15, relation = $16,
             mileage_in = $17, intake_condition = $18,
             fuel_level = $19, key_count = $20, valuables_declared = $21, valuables = $22
         WHERE id = $1
         RETURNING id, number, title, partner_id, contact_id, lead_id, project_type_id, currency, valuation_date,
                   vehicle_make, vehicle_model, vehicle_plate, vehicle_vin, description, due_date, assigned_to,
                   related_order_id, relation, mileage_in, intake_condition,
                fuel_level, key_count, valuables_declared, valuables, created_by, minicrm_id, created_at, updated_at",
        id,
        f.title,
        f.partner_id,
        f.contact_id,
        f.project_type_id,
        f.currency,
        f.valuation_date,
        f.vehicle_make,
        f.vehicle_model,
        f.vehicle_plate,
        f.vehicle_vin,
        f.description,
        f.due_date,
        f.assigned_to,
        f.related_order_id,
        f.relation,
        f.mileage_in,
        f.intake_condition,
        f.fuel_level,
        f.key_count,
        f.valuables_declared,
        f.valuables
    )
    .fetch_optional(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct OrderSummary {
    pub id: i64,
    pub number: String,
    pub title: String,
    pub partner_id: i64,
    pub partner_name: String,
    pub project_type_id: Option<i64>,
    pub project_type_label: Option<String>,
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    pub total_minor: i64,
    pub vehicle_make: Option<String>,
    pub vehicle_model: Option<String>,
    pub vehicle_plate: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub assigned_to: Option<i64>,
    pub assigned_name: Option<String>,
    pub stage_key: String,
    pub stage_label: String,
    pub stage_is_terminal: bool,
    pub stage_entered_at: DateTime<Utc>,
    pub open_blockers: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct OrderFilter {
    /// ILIKE pattern (already escaped and wrapped in %).
    pub pattern: Option<String>,
    /// `%NORMALISEDPLATE%` or empty.
    pub plate_pattern: String,
    pub stage_key: Option<String>,
    pub partner_id: Option<i64>,
    pub project_type_id: Option<i64>,
    pub assigned_to: Option<i64>,
    pub open_only: bool,
}

/// Sort keys accepted by order search (`-` prefix for descending).
/// The value travels to SQL as a bind parameter matched against static CASE
/// branches, so the query stays fully compile-time checked.
/// `stage_entered_at` orders by when the order entered its *current* stage: the pickup
/// board and the dashboard ask for `stage=completed&sort=-stage_entered_at` to list the
/// most recently finished cars rather than the most recently created orders.
pub const ORDER_SORTS: &[&str] = &[
    "created_at",
    "due_date",
    "total",
    "number",
    "stage_entered_at",
];

pub const DEFAULT_SORT: &str = "-created_at";

/// Runtime-checked (not `query_as!`) so a new sort branch doesn't need the offline query
/// cache regenerated against a live database. Column names match `OrderSummary` fields.
const SEARCH_SQL: &str = r#"SELECT o.id, o.number, o.title, o.partner_id, p.name AS partner_name,
                  o.project_type_id, pt.label_hu AS project_type_label,
                  o.currency, ov.total_minor AS total_minor,
                  o.vehicle_make, o.vehicle_model, o.vehicle_plate, o.due_date,
                  o.assigned_to, u.display_name AS assigned_name,
                  cs.stage_key AS stage_key, sd.label_hu AS stage_label, sd.is_terminal AS stage_is_terminal,
                  cs.entered_at AS stage_entered_at,
                  (SELECT count(*) FROM blockers b WHERE b.order_id = o.id AND b.resolved_at IS NULL) AS open_blockers,
                  o.created_at, o.updated_at
           FROM orders o
           JOIN partners p ON p.id = o.partner_id
           JOIN order_current_stage cs ON cs.order_id = o.id
           JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
           JOIN order_values ov ON ov.order_id = o.id
           LEFT JOIN project_types pt ON pt.id = o.project_type_id
           LEFT JOIN users u ON u.id = o.assigned_to
           WHERE ($1::text IS NULL OR o.number ILIKE $1 OR o.title ILIKE $1 OR p.name ILIKE $1 OR o.vehicle_vin ILIKE $1
                  OR ($2::text <> '' AND upper(regexp_replace(o.vehicle_plate, '[^A-Za-z0-9]', '', 'g')) LIKE $2)
                  -- V2.1: the plate lives on the vehicle now. The order's own text columns
                  -- stay in the predicate as the migration fallback.
                  OR EXISTS (SELECT 1 FROM order_vehicles xv JOIN vehicles xveh ON xveh.id = xv.vehicle_id
                              WHERE xv.order_id = o.id
                                AND (($2::text <> '' AND xveh.plate_norm LIKE $2) OR xveh.vin ILIKE $1)))
             AND ($3::text IS NULL OR cs.stage_key = $3)
             AND ($4::bigint IS NULL OR o.partner_id = $4)
             AND ($5::bigint IS NULL OR o.project_type_id = $5)
             AND ($6::bigint IS NULL OR o.assigned_to = $6)
             AND (NOT $7 OR NOT sd.is_terminal)
           ORDER BY
               CASE WHEN $10 = 'created_at' THEN o.created_at END ASC,
               CASE WHEN $10 = '-created_at' THEN o.created_at END DESC,
               CASE WHEN $10 = 'due_date' THEN o.due_date END ASC NULLS LAST,
               CASE WHEN $10 = '-due_date' THEN o.due_date END DESC NULLS LAST,
                -- V3: totals sort in HUF terms. Raw minor units are not comparable
                -- across currencies (10 000 fillér ≈ €0.26 would rank with €100).
                CASE WHEN $10 = 'total' THEN ov.total_huf_minor END ASC NULLS LAST,
                CASE WHEN $10 = '-total' THEN ov.total_huf_minor END DESC NULLS LAST,
               CASE WHEN $10 = 'number' THEN o.number END ASC,
               CASE WHEN $10 = '-number' THEN o.number END DESC,
               CASE WHEN $10 = 'stage_entered_at' THEN cs.entered_at END ASC,
               CASE WHEN $10 = '-stage_entered_at' THEN cs.entered_at END DESC,
               o.id DESC
           LIMIT $8 OFFSET $9"#;

pub async fn search(
    db: impl PgExecutor<'_>,
    f: &OrderFilter,
    sort_key: &str,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<OrderSummary>> {
    sqlx::query_as::<_, OrderSummary>(SEARCH_SQL)
        .bind(&f.pattern)
        .bind(&f.plate_pattern)
        .bind(&f.stage_key)
        .bind(f.partner_id)
        .bind(f.project_type_id)
        .bind(f.assigned_to)
        .bind(f.open_only)
        .bind(limit)
        .bind(offset)
        .bind(sort_key)
        .fetch_all(db)
        .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct OrderValue {
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    pub total_minor: i64,
    pub valuation_date: NaiveDate,
    pub fx_day: Option<NaiveDate>,
    pub fx_rate: Option<Decimal>,
    pub total_huf_minor: Option<i64>,
}

pub async fn value(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Option<OrderValue>> {
    sqlx::query_as!(
        OrderValue,
        r#"SELECT currency AS "currency!", total_minor AS "total_minor!", valuation_date AS "valuation_date!",
                  fx_day, fx_rate, total_huf_minor
           FROM order_values WHERE order_id = $1"#,
        order_id
    )
    .fetch_optional(db)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::parse_sort;

    /// ORD-59 / ORD-L5: the pickup board and the dashboard list the most recently
    /// *completed* cars, which needs a sort on when the order entered its current stage.
    #[test]
    fn orders_can_be_sorted_by_when_they_entered_their_stage() {
        assert_eq!(
            parse_sort(Some("-stage_entered_at"), ORDER_SORTS, DEFAULT_SORT),
            Ok("-stage_entered_at".to_string())
        );
        assert!(SEARCH_SQL.contains("$10 = '-stage_entered_at' THEN cs.entered_at END DESC"));
        assert!(SEARCH_SQL.contains("$10 = 'stage_entered_at' THEN cs.entered_at END ASC"));
    }
}
