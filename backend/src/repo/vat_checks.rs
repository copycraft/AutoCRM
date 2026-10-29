//! The VAT-check audit log (0053). Insert and read only: the table refuses UPDATE/DELETE.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VatCheck {
    pub id: i64,
    pub subject_type: Option<String>,
    pub subject_id: Option<i64>,
    pub source: String,
    pub input_raw: String,
    pub country_code: String,
    pub vat_number: String,
    pub hu_domestic_tax_number: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    /// VIES's consultation number: proof we checked this number on this date.
    pub request_identifier: Option<String>,
    pub request_date: Option<DateTime<Utc>>,
    pub requester: Option<String>,
    pub attempts: i32,
    pub duration_ms: i32,
    pub hints: Vec<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

pub struct NewVatCheck<'a> {
    pub subject: Option<(&'a str, i64)>,
    pub input_raw: &'a str,
    pub country_code: &'a str,
    pub vat_number: &'a str,
    pub hu_domestic_tax_number: Option<&'a str>,
    pub status: &'a str,
    pub error_code: Option<&'a str>,
    pub name: Option<&'a str>,
    pub address: Option<&'a str>,
    pub request_identifier: Option<&'a str>,
    pub request_date: Option<DateTime<Utc>>,
    pub requester: Option<&'a str>,
    pub attempts: i32,
    pub duration_ms: i32,
    pub raw_response: &'a serde_json::Value,
    pub hints: &'a [String],
    pub created_by: &'a str,
}

pub async fn insert(db: impl PgExecutor<'_>, c: NewVatCheck<'_>) -> sqlx::Result<VatCheck> {
    sqlx::query_as!(
        VatCheck,
        r#"INSERT INTO vat_checks (subject_type, subject_id, source, input_raw, country_code, vat_number,
               hu_domestic_tax_number, status, error_code, name, address, request_identifier, request_date,
               requester, attempts, duration_ms, raw_response, hints, created_by)
           VALUES ($1, $2, 'VIES', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
           RETURNING id, subject_type, subject_id, source, input_raw, country_code AS "country_code!",
                     vat_number, hu_domestic_tax_number, status, error_code, name, address, request_identifier,
                     request_date, requester, attempts, duration_ms, hints, created_by, created_at"#,
        c.subject.map(|(t, _)| t),
        c.subject.map(|(_, id)| id),
        c.input_raw,
        c.country_code,
        c.vat_number,
        c.hu_domestic_tax_number,
        c.status,
        c.error_code,
        c.name,
        c.address,
        c.request_identifier,
        c.request_date,
        c.requester,
        c.attempts,
        c.duration_ms,
        c.raw_response,
        c.hints,
        c.created_by,
    )
    .fetch_one(db)
    .await
}

/// The newest final verdict (VALID/INVALID) on this number since `since`.
pub async fn latest_verdict(
    db: impl PgExecutor<'_>,
    country_code: &str,
    vat_number: &str,
    since: DateTime<Utc>,
) -> sqlx::Result<Option<VatCheck>> {
    sqlx::query_as!(
        VatCheck,
        r#"SELECT id, subject_type, subject_id, source, input_raw, country_code AS "country_code!",
                  vat_number, hu_domestic_tax_number, status, error_code, name, address, request_identifier,
                  request_date, requester, attempts, duration_ms, hints, created_by, created_at
           FROM vat_checks
           WHERE country_code = $1 AND vat_number = $2 AND source = 'VIES'
             AND status IN ('VALID', 'INVALID') AND created_at >= $3
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
        country_code,
        vat_number,
        since
    )
    .fetch_optional(db)
    .await
}

/// A subject's checks, newest first.
pub async fn list_for_subject(
    db: impl PgExecutor<'_>,
    subject_type: &str,
    subject_id: i64,
) -> sqlx::Result<Vec<VatCheck>> {
    sqlx::query_as!(
        VatCheck,
        r#"SELECT id, subject_type, subject_id, source, input_raw, country_code AS "country_code!",
                  vat_number, hu_domestic_tax_number, status, error_code, name, address, request_identifier,
                  request_date, requester, attempts, duration_ms, hints, created_by, created_at
           FROM vat_checks
           WHERE subject_type = $1 AND subject_id = $2
           ORDER BY created_at DESC, id DESC
           LIMIT 200"#,
        subject_type,
        subject_id
    )
    .fetch_all(db)
    .await
}

/// Records the latest verdict on the partner (the denormalised list/filter columns).
/// Returns false when the partner doesn't exist.
pub async fn set_partner_status(
    db: impl PgExecutor<'_>,
    partner_id: i64,
    check: &VatCheck,
    formatted: &str,
) -> sqlx::Result<bool> {
    // An UNAVAILABLE streak on the same number keeps its start (the 24-hour re-check
    // window, 0054); any real answer, or another number, ends it.
    let done = sqlx::query!(
        "UPDATE partners SET
             vat_unavailable_since = CASE
                 WHEN $3 <> 'UNAVAILABLE' THEN NULL
                 WHEN vat_status = 'UNAVAILABLE' AND vat_formatted = $2 THEN vat_unavailable_since
                 ELSE $4 END,
             vat_needs_manual_check = false,
             vat_formatted = $2, vat_status = $3, vat_checked_at = $4, vat_last_check_id = $5
         WHERE id = $1",
        partner_id,
        formatted,
        check.status,
        check.created_at,
        check.id
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// A partner a sweep job works on.
#[derive(Debug, Clone)]
pub struct SweepPartner {
    pub id: i64,
    /// The number to check: the last checked one, or `eu_tax_number` for the backfill.
    pub number: String,
    pub country: String,
    pub vat_unavailable_since: Option<DateTime<Utc>>,
}

/// Partners whose last answer was UNAVAILABLE and nobody has been asked to look at yet.
pub async fn unavailable_partners(
    db: impl PgExecutor<'_>,
    after_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<SweepPartner>> {
    sqlx::query_as!(
        SweepPartner,
        r#"SELECT id, vat_formatted AS "number!", country, vat_unavailable_since
           FROM partners
           WHERE vat_status = 'UNAVAILABLE' AND NOT vat_needs_manual_check
             AND vat_formatted IS NOT NULL AND archived_at IS NULL AND id > $1
           ORDER BY id LIMIT $2"#,
        after_id,
        limit
    )
    .fetch_all(db)
    .await
}

/// VIES kept failing for a day: a person has to check this one.
pub async fn flag_manual_check(db: impl PgExecutor<'_>, partner_id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE partners SET vat_needs_manual_check = true WHERE id = $1",
        partner_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// VALID partners last checked before `checked_before` with an order or enquiry touched in
/// the last 12 months.
pub async fn revalidation_due(
    db: impl PgExecutor<'_>,
    after_id: i64,
    checked_before: DateTime<Utc>,
    limit: i64,
) -> sqlx::Result<Vec<SweepPartner>> {
    sqlx::query_as!(
        SweepPartner,
        r#"SELECT p.id, p.vat_formatted AS "number!", p.country, p.vat_unavailable_since
           FROM partners p
           WHERE p.vat_status = 'VALID' AND p.vat_formatted IS NOT NULL AND p.archived_at IS NULL
             AND p.vat_checked_at < $2 AND p.id > $1
             AND (EXISTS (SELECT 1 FROM orders o WHERE o.partner_id = p.id
                                                   AND o.updated_at > now() - interval '12 months')
                  OR EXISTS (SELECT 1 FROM leads l WHERE l.partner_id = p.id
                                                     AND l.updated_at > now() - interval '12 months'))
           ORDER BY p.id LIMIT $3"#,
        after_id,
        checked_before,
        limit
    )
    .fetch_all(db)
    .await
}

/// Partners with an EU VAT number that were never checked (the MiniCRM import backfill).
pub async fn never_checked(
    db: impl PgExecutor<'_>,
    after_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<SweepPartner>> {
    sqlx::query_as!(
        SweepPartner,
        r#"SELECT id, eu_tax_number AS "number!", country, vat_unavailable_since
           FROM partners
           WHERE vat_status IS NULL AND eu_tax_number IS NOT NULL AND btrim(eu_tax_number) <> ''
             AND archived_at IS NULL AND id > $1
           ORDER BY id LIMIT $2"#,
        after_id,
        limit
    )
    .fetch_all(db)
    .await
}
