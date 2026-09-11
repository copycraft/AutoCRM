use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::domain::partner::PartnerKind;

#[derive(Debug, Clone, Serialize)]
pub struct Partner {
    pub id: i64,
    pub kind: PartnerKind,
    pub name: String,
    pub tax_number: Option<String>,
    pub eu_tax_number: Option<String>,
    pub country: String,
    pub default_currency: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub address_line: Option<String>,
    pub notes: Option<String>,
    pub minicrm_id: Option<i64>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Validated, normalised partner fields (see api::partners).
#[derive(Debug, Clone)]
pub struct PartnerInput {
    pub kind: PartnerKind,
    pub name: String,
    pub tax_number: Option<String>,
    pub eu_tax_number: Option<String>,
    pub country: String,
    pub default_currency: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub address_line: Option<String>,
    pub notes: Option<String>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Partner>> {
    sqlx::query_as!(
        Partner,
        r#"SELECT id, kind AS "kind: PartnerKind", name, tax_number, eu_tax_number, country, default_currency,
                  email, phone, website, postal_code, city, address_line, notes, minicrm_id, archived_at, created_at, updated_at
           FROM partners WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn search(
    db: impl PgExecutor<'_>,
    pattern: Option<&str>,
    kind: Option<PartnerKind>,
    include_archived: bool,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<Partner>> {
    sqlx::query_as!(
        Partner,
        r#"SELECT id, kind AS "kind: PartnerKind", name, tax_number, eu_tax_number, country, default_currency,
                  email, phone, website, postal_code, city, address_line, notes, minicrm_id, archived_at, created_at, updated_at
           FROM partners
           WHERE ($1::text IS NULL OR name ILIKE $1 OR tax_number ILIKE $1 OR eu_tax_number ILIKE $1
                  OR email ILIKE $1 OR city ILIKE $1)
             AND ($2::partner_kind IS NULL OR kind = $2)
             AND ($3 OR archived_at IS NULL)
           ORDER BY name, id
           LIMIT $4 OFFSET $5"#,
        pattern,
        kind as Option<PartnerKind>,
        include_archived,
        limit,
        offset
    )
    .fetch_all(db)
    .await
}

pub async fn insert(db: impl PgExecutor<'_>, p: &PartnerInput) -> sqlx::Result<Partner> {
    sqlx::query_as!(
        Partner,
        r#"INSERT INTO partners (kind, name, tax_number, eu_tax_number, country, default_currency,
                                 email, phone, website, postal_code, city, address_line, notes)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
           RETURNING id, kind AS "kind: PartnerKind", name, tax_number, eu_tax_number, country, default_currency,
                     email, phone, website, postal_code, city, address_line, notes, minicrm_id, archived_at, created_at, updated_at"#,
        p.kind as PartnerKind,
        p.name,
        p.tax_number,
        p.eu_tax_number,
        p.country,
        p.default_currency,
        p.email,
        p.phone,
        p.website,
        p.postal_code,
        p.city,
        p.address_line,
        p.notes
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    p: &PartnerInput,
) -> sqlx::Result<Option<Partner>> {
    sqlx::query_as!(
        Partner,
        r#"UPDATE partners
           SET kind = $2, name = $3, tax_number = $4, eu_tax_number = $5, country = $6, default_currency = $7,
               email = $8, phone = $9, website = $10, postal_code = $11, city = $12, address_line = $13, notes = $14
           WHERE id = $1
           RETURNING id, kind AS "kind: PartnerKind", name, tax_number, eu_tax_number, country, default_currency,
                     email, phone, website, postal_code, city, address_line, notes, minicrm_id, archived_at, created_at, updated_at"#,
        id,
        p.kind as PartnerKind,
        p.name,
        p.tax_number,
        p.eu_tax_number,
        p.country,
        p.default_currency,
        p.email,
        p.phone,
        p.website,
        p.postal_code,
        p.city,
        p.address_line,
        p.notes
    )
    .fetch_optional(db)
    .await
}

pub async fn set_archived(db: impl PgExecutor<'_>, id: i64, archived: bool) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE partners SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) ELSE NULL END WHERE id = $1",
        id,
        archived
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}
