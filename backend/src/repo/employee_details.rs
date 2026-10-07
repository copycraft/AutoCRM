//! The papers HR collects per employee: personal, financial and family data. Every field
//! is optional column by column, but what is missing decides the employee's
//! "…adatokra vár" status: those statuses set themselves until the data is complete.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct EmployeeDetails {
    pub employee_id: i64,
    pub birth_name: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub birth_place: Option<String>,
    pub mother_name: Option<String>,
    pub nationality: Option<String>,
    pub address: Option<String>,
    pub id_card_number: Option<String>,
    pub tax_id: Option<String>,
    pub taj_number: Option<String>,
    pub bank_account: Option<String>,
    pub marital_status: Option<String>,
    pub children_count: Option<i32>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Which data is still missing. Pure over the row, so the rule is unit-tested and the
/// same rule explains itself in tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing {
    pub personal: bool,
    pub financial: bool,
    pub family: bool,
}

fn blank(v: Option<&String>) -> bool {
    v.map(|s| s.trim().is_empty()).unwrap_or(true)
}

impl EmployeeDetails {
    pub fn missing(&self) -> Missing {
        Missing {
            personal: blank(self.birth_name.as_ref())
                || self.birth_date.is_none()
                || blank(self.birth_place.as_ref())
                || blank(self.mother_name.as_ref())
                || blank(self.address.as_ref()),
            financial: blank(self.tax_id.as_ref())
                || blank(self.taj_number.as_ref())
                || blank(self.bank_account.as_ref()),
            family: blank(self.marital_status.as_ref())
                || self.children_count.is_none()
                || blank(self.emergency_contact_name.as_ref()),
        }
    }

    /// The auto status the current data points at.
    pub fn auto_key(&self) -> &'static str {
        let m = self.missing();
        if m.personal {
            "missing_personal"
        } else if m.financial {
            "missing_financial"
        } else if m.family {
            "missing_family"
        } else {
            "complete"
        }
    }
}

pub async fn get(
    db: impl PgExecutor<'_>,
    employee_id: i64,
) -> sqlx::Result<Option<EmployeeDetails>> {
    sqlx::query_as(
        "SELECT employee_id, birth_name, birth_date, birth_place, mother_name, nationality,
                address, id_card_number, tax_id, taj_number, bank_account, marital_status,
                children_count, emergency_contact_name, emergency_contact_phone, updated_at
           FROM employee_details WHERE employee_id = $1",
    )
    .bind(employee_id)
    .fetch_optional(db)
    .await
}

/// Inserts or replaces the row; the request carries the full intended state.
pub async fn upsert(
    db: impl PgExecutor<'_>,
    employee_id: i64,
    d: &EmployeeDetails,
) -> sqlx::Result<EmployeeDetails> {
    sqlx::query_as(
        "INSERT INTO employee_details (employee_id, birth_name, birth_date, birth_place,
                mother_name, nationality, address, id_card_number, tax_id, taj_number,
                bank_account, marital_status, children_count, emergency_contact_name,
                emergency_contact_phone)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
         ON CONFLICT (employee_id) DO UPDATE SET
                birth_name = excluded.birth_name,
                birth_date = excluded.birth_date,
                birth_place = excluded.birth_place,
                mother_name = excluded.mother_name,
                nationality = excluded.nationality,
                address = excluded.address,
                id_card_number = excluded.id_card_number,
                tax_id = excluded.tax_id,
                taj_number = excluded.taj_number,
                bank_account = excluded.bank_account,
                marital_status = excluded.marital_status,
                children_count = excluded.children_count,
                emergency_contact_name = excluded.emergency_contact_name,
                emergency_contact_phone = excluded.emergency_contact_phone
         RETURNING employee_id, birth_name, birth_date, birth_place, mother_name, nationality,
                address, id_card_number, tax_id, taj_number, bank_account, marital_status,
                children_count, emergency_contact_name, emergency_contact_phone, updated_at",
    )
    .bind(employee_id)
    .bind(&d.birth_name)
    .bind(d.birth_date)
    .bind(&d.birth_place)
    .bind(&d.mother_name)
    .bind(&d.nationality)
    .bind(&d.address)
    .bind(&d.id_card_number)
    .bind(&d.tax_id)
    .bind(&d.taj_number)
    .bind(&d.bank_account)
    .bind(&d.marital_status)
    .bind(d.children_count)
    .bind(&d.emergency_contact_name)
    .bind(&d.emergency_contact_phone)
    .fetch_one(db)
    .await
}

/// The status id for an auto_key, archived ones excluded.
pub async fn status_id_for_auto_key(
    db: impl PgExecutor<'_>,
    key: &str,
) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar(
        "SELECT id FROM employee_statuses WHERE auto_key = $1 AND archived_at IS NULL",
    )
    .bind(key)
    .fetch_optional(db)
    .await
}

/// The auto_key of the employee's current status, when it is one of the data-driven ones.
pub async fn current_auto_key(
    db: impl PgExecutor<'_>,
    employee_id: i64,
) -> sqlx::Result<Option<String>> {
    let row: Option<Option<String>> = sqlx::query_scalar(
        "SELECT s.auto_key FROM employees e
           JOIN employee_statuses s ON s.id = e.status_id
          WHERE e.id = $1",
    )
    .bind(employee_id)
    .fetch_optional(db)
    .await?;
    Ok(row.flatten())
}
