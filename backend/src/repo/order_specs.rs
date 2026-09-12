//! The build specification on an order: heating or cooling, never both.
//!
//! Which form an order uses comes from its project type (`project_types.spec_form`) and is
//! copied onto the row when the spec is written. Looking it up at read time would mean that
//! renaming a project type in settings silently reinterprets specs already recorded.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OrderSpec {
    pub order_id: i64,
    /// `heating` | `cooling`.
    pub form: String,

    // Shared: both a heated and a cooled body hold a temperature and are insulated.
    #[schema(value_type = Option<String>)]
    pub target_temp_c: Option<Decimal>,
    pub insulation_mm: Option<i32>,

    // Cooling.
    pub cooling_unit_make: Option<String>,
    pub cooling_unit_model: Option<String>,
    /// ATP classification (FNA, FRC, …). Free text until the real list is confirmed.
    pub atp_class: Option<String>,
    pub compartments: Option<i32>,
    /// `automatic` | `manual` | `hot_gas`.
    pub defrost: Option<String>,
    pub electric_standby: Option<bool>,

    // Heating.
    pub heater_make: Option<String>,
    pub heater_model: Option<String>,
    #[schema(value_type = Option<String>)]
    pub heat_output_kw: Option<Decimal>,
    /// `diesel` | `electric` | `lpg` | `engine_coolant`.
    pub fuel: Option<String>,
    pub thermostat: Option<bool>,

    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The writable half. The database refuses a row whose fields belong to the other variant,
/// so a caller that forgets to clear them gets an error rather than a meaningless row.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SpecFields {
    pub form: String,
    #[schema(value_type = Option<String>)]
    pub target_temp_c: Option<Decimal>,
    pub insulation_mm: Option<i32>,
    pub cooling_unit_make: Option<String>,
    pub cooling_unit_model: Option<String>,
    pub atp_class: Option<String>,
    pub compartments: Option<i32>,
    pub defrost: Option<String>,
    pub electric_standby: Option<bool>,
    pub heater_make: Option<String>,
    pub heater_model: Option<String>,
    #[schema(value_type = Option<String>)]
    pub heat_output_kw: Option<Decimal>,
    pub fuel: Option<String>,
    pub thermostat: Option<bool>,
    pub notes: Option<String>,
}

impl SpecFields {
    /// Blanks the fields of the other variant. The CHECK constraints would refuse them
    /// anyway; doing it here means switching a project type from cooling to heating on a
    /// half-filled form is a clean save rather than a 422.
    pub fn for_form(mut self) -> Self {
        if self.form == "heating" {
            self.cooling_unit_make = None;
            self.cooling_unit_model = None;
            self.atp_class = None;
            self.compartments = None;
            self.defrost = None;
            self.electric_standby = None;
        } else {
            self.heater_make = None;
            self.heater_model = None;
            self.heat_output_kw = None;
            self.fuel = None;
            self.thermostat = None;
        }
        self
    }
}

pub async fn find(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Option<OrderSpec>> {
    sqlx::query_as!(
        OrderSpec,
        r#"SELECT order_id, form, target_temp_c, insulation_mm, cooling_unit_make, cooling_unit_model,
                  atp_class, compartments, defrost, electric_standby, heater_make, heater_model,
                  heat_output_kw, fuel, thermostat, notes, created_at, updated_at
             FROM order_specs WHERE order_id = $1"#,
        order_id
    )
    .fetch_optional(db)
    .await
}

pub async fn upsert(
    db: impl PgExecutor<'_>,
    order_id: i64,
    f: &SpecFields,
) -> sqlx::Result<OrderSpec> {
    sqlx::query_as!(
        OrderSpec,
        r#"INSERT INTO order_specs (order_id, form, target_temp_c, insulation_mm, cooling_unit_make,
                                    cooling_unit_model, atp_class, compartments, defrost, electric_standby,
                                    heater_make, heater_model, heat_output_kw, fuel, thermostat, notes)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           ON CONFLICT (order_id) DO UPDATE
           SET form = EXCLUDED.form, target_temp_c = EXCLUDED.target_temp_c,
               insulation_mm = EXCLUDED.insulation_mm, cooling_unit_make = EXCLUDED.cooling_unit_make,
               cooling_unit_model = EXCLUDED.cooling_unit_model, atp_class = EXCLUDED.atp_class,
               compartments = EXCLUDED.compartments, defrost = EXCLUDED.defrost,
               electric_standby = EXCLUDED.electric_standby, heater_make = EXCLUDED.heater_make,
               heater_model = EXCLUDED.heater_model, heat_output_kw = EXCLUDED.heat_output_kw,
               fuel = EXCLUDED.fuel, thermostat = EXCLUDED.thermostat, notes = EXCLUDED.notes
           RETURNING order_id, form, target_temp_c, insulation_mm, cooling_unit_make, cooling_unit_model,
                     atp_class, compartments, defrost, electric_standby, heater_make, heater_model,
                     heat_output_kw, fuel, thermostat, notes, created_at, updated_at"#,
        order_id,
        f.form,
        f.target_temp_c,
        f.insulation_mm,
        f.cooling_unit_make,
        f.cooling_unit_model,
        f.atp_class,
        f.compartments,
        f.defrost,
        f.electric_standby,
        f.heater_make,
        f.heater_model,
        f.heat_output_kw,
        f.fuel,
        f.thermostat,
        f.notes
    )
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query!("DELETE FROM order_specs WHERE order_id = $1", order_id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// The spec form a project type asks for, if any.
pub async fn form_for_project_type(
    db: impl PgExecutor<'_>,
    project_type_id: i64,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!(
        "SELECT spec_form FROM project_types WHERE id = $1",
        project_type_id
    )
    .fetch_optional(db)
    .await
    .map(Option::flatten)
}
