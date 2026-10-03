//! HR leave and absence: who is away when, and how much annual leave is left. Every route
//! needs `AccessHr`, like the rest of the HR module.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional};
use crate::AppState;
use crate::domain::leave::{LeaveKind, working_days};
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::absences::{self, AbsenceRow};
use crate::repo::{audit, employees};
use crate::service::business_today;

/// A period longer than this is a typo, not leave.
const MAX_SPAN_DAYS: i64 = 366;
const MAX_NOTE: usize = 500;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_absences))
        .routes(routes!(create_absence))
        .routes(routes!(delete_absence))
        .routes(routes!(leave_summary))
}

#[derive(Serialize, ToSchema)]
pub struct Absence {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: String,
    pub kind: LeaveKind,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    /// Working days in the period: weekends and public holidays do not count.
    pub working_days: i32,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

fn present(row: AbsenceRow) -> AppResult<Absence> {
    let kind = LeaveKind::parse(&row.kind)
        .ok_or_else(|| AppError::internal(format!("unknown leave kind {:?}", row.kind)))?;
    Ok(Absence {
        id: row.id,
        employee_id: row.employee_id,
        employee_name: row.employee_name,
        kind,
        working_days: working_days(row.start_date, row.end_date, None),
        start_date: row.start_date,
        end_date: row.end_date,
        note: row.note,
        created_at: row.created_at,
    })
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AbsenceQuery {
    /// First day of the range (inclusive).
    from: NaiveDate,
    /// Last day of the range (inclusive). At most a year after `from`.
    to: NaiveDate,
    employee_id: Option<i64>,
}

/// Absences touching a date range: the team calendar.
#[utoipa::path(
    get, path = "/hr/absences", tag = "hr",
    params(AbsenceQuery),
    responses((status = 200, body = Items<Absence>))
)]
async fn list_absences(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<AbsenceQuery>,
) -> AppResult<Json<Items<Absence>>> {
    me.require(Capability::AccessHr)?;
    if q.to < q.from {
        return Err(AppError::validation("to must not be before from"));
    }
    if (q.to - q.from).num_days() > MAX_SPAN_DAYS {
        return Err(AppError::validation("the range is longer than a year"));
    }
    let rows = absences::list(&state.db, q.from, q.to, q.employee_id).await?;
    let items = rows.into_iter().map(present).collect::<AppResult<_>>()?;
    Ok(Items::new(items))
}

#[derive(Deserialize, ToSchema)]
struct AbsenceBody {
    kind: LeaveKind,
    start_date: NaiveDate,
    /// Last day away. Same as `start_date` for a single day.
    end_date: NaiveDate,
    note: Option<String>,
}

#[utoipa::path(
    post, path = "/hr/employees/{id}/absences", tag = "hr",
    params(("id" = i64, Path)),
    request_body = AbsenceBody,
    responses(
        (status = 201, body = Absence),
        (status = 409, description = "The employee already has leave in that period (code `overlap`)"),
    )
)]
async fn create_absence(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(employee_id): ApiPath<i64>,
    ApiJson(b): ApiJson<AbsenceBody>,
) -> AppResult<(StatusCode, Json<Absence>)> {
    me.require(Capability::AccessHr)?;
    if b.end_date < b.start_date {
        return Err(AppError::validation(
            "end_date must not be before start_date",
        ));
    }
    if (b.end_date - b.start_date).num_days() > MAX_SPAN_DAYS {
        return Err(AppError::validation("the period is longer than a year"));
    }
    if working_days(b.start_date, b.end_date, None) == 0 {
        return Err(AppError::validation(
            "the period contains no working days (weekend or public holiday only)",
        ));
    }
    let note = optional(b.note);
    if note.as_ref().is_some_and(|n| n.chars().count() > MAX_NOTE) {
        return Err(AppError::validation(format!(
            "note is too long (at most {MAX_NOTE} characters)"
        )));
    }

    let mut tx = state.db.begin().await?;
    // The lock serialises two people booking the same employee at once, so the overlap
    // check below cannot be raced.
    let employee = employees::lock(&mut *tx, employee_id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    if employee.archived_at.is_some() {
        return Err(AppError::validation("the employee has left"));
    }
    if absences::overlaps(&mut *tx, employee_id, b.start_date, b.end_date).await? {
        return Err(AppError::conflict(
            "overlap",
            "the employee already has leave in that period",
        ));
    }
    let id = absences::insert(
        &mut *tx,
        employee_id,
        b.kind.as_str(),
        b.start_date,
        b.end_date,
        note.as_deref(),
        me.user_id,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        employee_id,
        "absence_add",
        json!({ "absence_id": id, "kind": b.kind.as_str(), "from": b.start_date, "to": b.end_date }),
    )
    .await?;
    let row = absences::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("absence"))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(present(row)?)))
}

#[utoipa::path(
    delete, path = "/hr/absences/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Removed"))
)]
async fn delete_absence(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    let row = absences::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("absence"))?;
    absences::delete(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        row.employee_id,
        "absence_remove",
        json!({ "absence_id": id, "kind": row.kind, "from": row.start_date, "to": row.end_date }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct LeaveBalance {
    pub employee_id: i64,
    pub full_name: String,
    pub year: i32,
    /// Paid annual leave for the year.
    pub allowance_days: i32,
    /// Working days of annual leave taken (or booked) in the year.
    pub used_annual: i32,
    /// `allowance_days - used_annual`; negative when more is booked than the allowance.
    pub remaining: i32,
    pub sick_days: i32,
    pub unpaid_days: i32,
    pub other_days: i32,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SummaryQuery {
    /// Calendar year; the current one by default.
    year: Option<i32>,
}

/// Every current employee's leave balance for a year.
#[utoipa::path(
    get, path = "/hr/leave-summary", tag = "hr",
    params(SummaryQuery),
    responses((status = 200, body = Items<LeaveBalance>))
)]
async fn leave_summary(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<SummaryQuery>,
) -> AppResult<Json<Items<LeaveBalance>>> {
    me.require(Capability::AccessHr)?;
    let year = q
        .year
        .unwrap_or_else(|| business_today(state.config.business_tz).year());
    if !(2000..=2100).contains(&year) {
        return Err(AppError::validation("year is out of range"));
    }
    let first = NaiveDate::from_ymd_opt(year, 1, 1).expect("a real date");
    let last = NaiveDate::from_ymd_opt(year, 12, 31).expect("a real date");
    let staff = employees::list(&state.db, None, false).await?;
    let all = absences::list(&state.db, first, last, None).await?;

    let items = staff
        .into_iter()
        .map(|e| {
            let mut days = [0i32; 4]; // annual, sick, unpaid, other
            for a in all.iter().filter(|a| a.employee_id == e.id) {
                let n = working_days(a.start_date, a.end_date, Some(year));
                match LeaveKind::parse(&a.kind) {
                    Some(LeaveKind::Annual) => days[0] += n,
                    Some(LeaveKind::Sick) => days[1] += n,
                    Some(LeaveKind::Unpaid) => days[2] += n,
                    Some(LeaveKind::Other) | None => days[3] += n,
                }
            }
            LeaveBalance {
                employee_id: e.id,
                full_name: e.full_name,
                year,
                allowance_days: e.annual_leave_days,
                used_annual: days[0],
                remaining: e.annual_leave_days - days[0],
                sick_days: days[1],
                unpaid_days: days[2],
                other_days: days[3],
            }
        })
        .collect();
    Ok(Items::new(items))
}
