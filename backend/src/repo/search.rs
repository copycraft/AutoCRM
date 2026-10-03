//! Global search: one box, several kinds of record, one round trip.
//!
//! The header search box asks "which record is this about": a plate shouted across the
//! yard, half a company name, an order number from an email, a phone number off a display.
//!
//! Every word typed must match somewhere in a record (see `domain::search_terms`), literally
//! or in the accent- and punctuation-insensitive folded form, so "kovacs sprinter" finds the
//! Kovács Kft. order for a Sprinter. Each group is capped so one chatty table cannot drown
//! the others, and ranked by how well the first field matches (exact, then prefix, then
//! anywhere), newest first within a rank.
//!
//! Runtime-checked queries: the match clause is shared text and the groups differ only in
//! their columns.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::domain::search_terms::SearchTerms;

/// One order hit: enough to recognise the job (number + plate + stage) and link to it.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct OrderHit {
    pub id: i64,
    pub number: String,
    pub title: String,
    pub plate: Option<String>,
    pub stage_label: String,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct PartnerHit {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub city: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct LeadHit {
    pub id: i64,
    pub title: String,
    pub contact_name: Option<String>,
    pub stage_label: String,
}

/// A person at a partner company.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ContactHit {
    pub id: i64,
    pub partner_id: i64,
    pub name: String,
    pub partner_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct EmailHit {
    pub id: i64,
    pub subject: String,
    pub to_address: String,
    pub status: String,
    pub queued_at: DateTime<Utc>,
}

/// A member of staff. Only ever returned to users with HR access.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct EmployeeHit {
    pub id: i64,
    pub full_name: String,
    pub email: Option<String>,
    pub company_phone: Option<String>,
    pub archived: bool,
}

/// Every word must match the row's text. `$1` literal patterns, `$2` folded patterns
/// (aligned), against `s.hay` and its fold `s.hay_norm`.
const MATCH: &str = "NOT EXISTS (
        SELECT 1 FROM unnest($1::text[], $2::text[]) AS t(raw, norm)
        WHERE NOT (s.hay ILIKE t.raw OR (t.norm IS NOT NULL AND s.hay_norm ~ t.norm)))";

/// A phone column as the folded text should see it: digits, `00` dropped, `06` read as `36`.
/// Mirrors `domain::partner::normalize_phone`, so "06 30 ..." finds "+36 30 ...".
fn phone_digits(column: &str) -> String {
    format!(
        "regexp_replace(regexp_replace(regexp_replace({column}, '[^0-9]', '', 'g'), '^00', ''), '^06', '36')"
    )
}

/// How well the first field matches the whole input: 0 exact, 1 starts with, 2 anywhere.
fn rank(field: &str) -> String {
    format!(
        "CASE WHEN search_fold({field}) = $4 THEN 0
              WHEN search_fold({field}) LIKE $4 || '%' THEN 1 ELSE 2 END"
    )
}

/// Runs one group's query with the shared binds.
async fn run<T>(
    db: impl PgExecutor<'_>,
    sql: &str,
    terms: &SearchTerms,
    limit: i64,
) -> sqlx::Result<Vec<T>>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
{
    sqlx::query_as(sql)
        .bind(&terms.raw)
        .bind(&terms.norm)
        .bind(limit)
        .bind(&terms.phrase)
        .fetch_all(db)
        .await
}

pub async fn orders(db: impl PgExecutor<'_>, q: &str, limit: i64) -> sqlx::Result<Vec<OrderHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.number, s.title, s.plate, s.stage_label
         FROM (
             SELECT o.id, o.number, o.title,
                    COALESCE(
                        (SELECT xveh.plate FROM order_vehicles xv
                          JOIN vehicles xveh ON xveh.id = xv.vehicle_id
                         WHERE xv.order_id = o.id ORDER BY xveh.plate LIMIT 1),
                        o.vehicle_plate
                    ) AS plate,
                    sd.label_hu AS stage_label,
                    concat_ws(' ', o.number, o.title, p.name, o.vehicle_vin, o.vehicle_plate,
                              o.vehicle_make, o.vehicle_model,
                              (SELECT string_agg(concat_ws(' ', xveh.plate, xveh.vin, xveh.make, xveh.model), ' ')
                                 FROM order_vehicles xv JOIN vehicles xveh ON xveh.id = xv.vehicle_id
                                WHERE xv.order_id = o.id)) AS hay
             FROM orders o
             JOIN order_current_stage cs ON cs.order_id = o.id
             JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
             JOIN partners p ON p.id = o.partner_id
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY LEAST({rank_number}, {rank_plate}), s.id DESC
         LIMIT $3",
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank_number = rank("s.number"),
        rank_plate = rank("coalesce(s.plate, '')"),
    );
    run(db, &sql, &terms, limit).await
}

pub async fn partners(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<PartnerHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.name, s.kind, s.city
         FROM (
             SELECT id, name, kind::text AS kind, city,
                    concat_ws(' ', name, city, tax_number, email, phone, {phone}) AS hay
             FROM partners
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY {rank}, s.id DESC
         LIMIT $3",
        phone = phone_digits("phone"),
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank = rank("s.name"),
    );
    run(db, &sql, &terms, limit).await
}

pub async fn leads(db: impl PgExecutor<'_>, q: &str, limit: i64) -> sqlx::Result<Vec<LeadHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.title, s.contact_name, s.stage_label
         FROM (
             SELECT l.id, l.title, l.contact_name, sd.label_hu AS stage_label,
                    concat_ws(' ', l.title, l.contact_name, l.contact_email, l.contact_phone,
                              {phone}, p.name, l.description) AS hay
             FROM leads l
             JOIN lead_current_stage cs ON cs.lead_id = l.id
             JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
             LEFT JOIN partners p ON p.id = l.partner_id
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY LEAST({rank_title}, {rank_name}), s.id DESC
         LIMIT $3",
        phone = phone_digits("l.contact_phone"),
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank_title = rank("s.title"),
        rank_name = rank("coalesce(s.contact_name, '')"),
    );
    run(db, &sql, &terms, limit).await
}

pub async fn contacts(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<ContactHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.partner_id, s.name, s.partner_name, s.email, s.phone
         FROM (
             SELECT c.id, c.partner_id, c.name, p.name AS partner_name, c.email, c.phone,
                    concat_ws(' ', c.name, c.email, c.phone, {phone}, c.position, p.name) AS hay
             FROM contacts c JOIN partners p ON p.id = c.partner_id
             WHERE c.archived_at IS NULL
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY {rank}, s.id DESC
         LIMIT $3",
        phone = phone_digits("c.phone"),
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank = rank("s.name"),
    );
    run(db, &sql, &terms, limit).await
}

pub async fn emails(db: impl PgExecutor<'_>, q: &str, limit: i64) -> sqlx::Result<Vec<EmailHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.subject, s.to_address, s.status, s.queued_at
         FROM (
             SELECT id, subject, to_address, status::text AS status, queued_at,
                    concat_ws(' ', subject, to_address) AS hay
             FROM email_messages
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY {rank}, s.id DESC
         LIMIT $3",
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank = rank("s.subject"),
    );
    run(db, &sql, &terms, limit).await
}

/// The staff directory. The caller must already have checked `AccessHr`.
pub async fn employees(
    db: impl PgExecutor<'_>,
    q: &str,
    limit: i64,
) -> sqlx::Result<Vec<EmployeeHit>> {
    let Some(terms) = SearchTerms::parse(q) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT s.id, s.full_name, s.email, s.company_phone, s.archived
         FROM (
             SELECT id, full_name, email, company_phone, archived_at IS NOT NULL AS archived,
                    concat_ws(' ', full_name, email, company_phone, personal_phone,
                              {company}, {personal}) AS hay
             FROM employees
         ) s, LATERAL (SELECT search_fold(s.hay) AS hay_norm) n
         WHERE {match_clause}
         ORDER BY s.archived, {rank}, s.id DESC
         LIMIT $3",
        company = phone_digits("company_phone"),
        personal = phone_digits("personal_phone"),
        match_clause = MATCH.replace("s.hay_norm", "n.hay_norm"),
        rank = rank("s.full_name"),
    );
    run(db, &sql, &terms, limit).await
}
