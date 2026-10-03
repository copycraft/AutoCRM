//! HR module: the staff directory. Every route needs `AccessHr` (admins, and users an admin
//! granted `hr_access`): the directory holds personal phone numbers.

use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::media::pipeline;
use crate::repo::employees::{self, EmployeeInput, EmployeeRow};
use crate::repo::{audit, like_pattern};

const PHOTO_URL_TTL: Duration = Duration::from_secs(3600);
/// The picture is re-encoded to 512 px, so a phone original is far more than enough.
const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;
const MAX_NAME: usize = 200;
const MAX_EMAIL: usize = 300;
const MAX_PHONE: usize = 50;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(detail, update))
        .routes(routes!(archive))
        .routes(routes!(unarchive))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(set_photo, remove_photo))
                .layer(DefaultBodyLimit::max(MAX_PHOTO_BYTES)),
        )
}

#[derive(Serialize, ToSchema)]
pub struct Employee {
    pub id: i64,
    pub full_name: String,
    pub email: Option<String>,
    /// The number the company pays for.
    pub company_phone: Option<String>,
    pub personal_phone: Option<String>,
    /// A short-lived link to the profile picture (about an hour); None without one.
    pub photo_url: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

async fn present(state: &AppState, row: EmployeeRow) -> Employee {
    let photo_url = match &row.photo_key {
        Some(key) => match state.storage.presign_get(key, PHOTO_URL_TTL, None).await {
            Ok(url) => Some(url),
            Err(e) => {
                tracing::warn!(employee_id = row.id, error = %e, "could not sign photo url");
                None
            }
        },
        None => None,
    };
    Employee {
        id: row.id,
        full_name: row.full_name,
        email: row.email,
        company_phone: row.company_phone,
        personal_phone: row.personal_phone,
        photo_url,
        archived_at: row.archived_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// Matches name, email and both phone numbers.
    q: Option<String>,
    /// Include employees who have left.
    #[serde(default)]
    include_archived: bool,
}

#[utoipa::path(
    get, path = "/hr/employees", tag = "hr",
    params(ListQuery),
    responses((status = 200, body = Items<Employee>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<Employee>>> {
    me.require(Capability::AccessHr)?;
    let pattern = q.q.as_deref().and_then(like_pattern);
    let rows = employees::list(&state.db, pattern.as_deref(), q.include_archived).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(present(&state, row).await);
    }
    Ok(Items::new(items))
}

/// Create requires `full_name`. On PATCH every field is optional; `null` clears.
#[derive(Deserialize, ToSchema)]
struct EmployeeBody {
    full_name: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    email: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    company_phone: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    personal_phone: Option<Option<String>>,
}

fn too_long(field: &str, value: &Option<String>, max: usize) -> AppResult<()> {
    match value {
        Some(v) if v.chars().count() > max => Err(AppError::validation(format!(
            "{field} is too long (at most {max} characters)"
        ))),
        _ => Ok(()),
    }
}

fn merge(current: Option<&EmployeeRow>, b: EmployeeBody) -> AppResult<EmployeeInput> {
    let full_name = b
        .full_name
        .or_else(|| current.map(|e| e.full_name.clone()))
        .unwrap_or_default();
    let full_name = required("full_name", &full_name)?;
    if full_name.chars().count() > MAX_NAME {
        return Err(AppError::validation(format!(
            "full_name is too long (at most {MAX_NAME} characters)"
        )));
    }
    let email = patch_text(&current.and_then(|e| e.email.clone()), b.email);
    too_long("email", &email, MAX_EMAIL)?;
    let email = match email {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("email is not a valid address"))?,
        ),
        None => None,
    };
    let company_phone = patch_text(
        &current.and_then(|e| e.company_phone.clone()),
        b.company_phone,
    );
    let personal_phone = patch_text(
        &current.and_then(|e| e.personal_phone.clone()),
        b.personal_phone,
    );
    too_long("company_phone", &company_phone, MAX_PHONE)?;
    too_long("personal_phone", &personal_phone, MAX_PHONE)?;
    Ok(EmployeeInput {
        full_name,
        email,
        company_phone,
        personal_phone,
    })
}

#[utoipa::path(
    post, path = "/hr/employees", tag = "hr",
    request_body = EmployeeBody,
    responses((status = 201, body = Employee))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<EmployeeBody>,
) -> AppResult<(StatusCode, Json<Employee>)> {
    me.require(Capability::AccessHr)?;
    let input = merge(None, b)?;
    let mut tx = state.db.begin().await?;
    let row = employees::insert(&mut *tx, &input, me.user_id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        row.id,
        "create",
        json!({ "full_name": row.full_name }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(present(&state, row).await)))
}

#[utoipa::path(
    get, path = "/hr/employees/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = Employee))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    let row = employees::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    Ok(Json(present(&state, row).await))
}

#[utoipa::path(
    patch, path = "/hr/employees/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = EmployeeBody,
    responses((status = 200, body = Employee))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<EmployeeBody>,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    let current = employees::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let input = merge(Some(&current), b)?;
    let updated = employees::update(&mut *tx, id, &input)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    // Field names only, not the values: the audit log is wider-readable than this module.
    let changed: Vec<&str> = [
        ("full_name", current.full_name != updated.full_name),
        ("email", current.email != updated.email),
        (
            "company_phone",
            current.company_phone != updated.company_phone,
        ),
        (
            "personal_phone",
            current.personal_phone != updated.personal_phone,
        ),
    ]
    .into_iter()
    .filter_map(|(name, changed)| changed.then_some(name))
    .collect();
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "update",
        json!({ "changed": changed }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(present(&state, updated).await))
}

async fn set_archived(
    state: &AppState,
    me: &crate::service::auth::AuthUser,
    id: i64,
    archived: bool,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    let row = employees::set_archived(&mut *tx, id, archived)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        if archived { "archive" } else { "unarchive" },
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(present(state, row).await))
}

#[utoipa::path(
    post, path = "/hr/employees/{id}/archive", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, description = "The employee has left; hidden from the default list", body = Employee))
)]
async fn archive(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Employee>> {
    set_archived(&state, &me, id, true).await
}

#[utoipa::path(
    post, path = "/hr/employees/{id}/unarchive", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = Employee))
)]
async fn unarchive(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Employee>> {
    set_archived(&state, &me, id, false).await
}

/// The profile picture: the raw image bytes (JPEG, PNG or WebP, up to 8 MB) as the body.
/// It is cropped to a centred square, scaled to 512 px and re-encoded as JPEG, which also
/// strips EXIF. Replaces any previous picture.
#[utoipa::path(
    put, path = "/hr/employees/{id}/photo", tag = "hr",
    params(("id" = i64, Path)),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 200, body = Employee))
)]
async fn set_photo(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    body: Bytes,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    if body.is_empty() {
        return Err(AppError::validation("the request body must be an image"));
    }
    employees::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let jpeg = tokio::task::spawn_blocking(move || pipeline::process_avatar(&body))
        .await
        .map_err(|e| AppError::internal(format!("avatar task: {e}")))?
        .map_err(|e| AppError::validation(e.to_string()))?;

    let key = format!("hr/employees/{id}/{:016x}.jpg", rand::random::<u64>());
    state
        .storage
        .put_bytes(&key, jpeg, "image/jpeg")
        .await
        .map_err(|e| AppError::internal(format!("storing photo: {e}")))?;

    let mut tx = state.db.begin().await?;
    let previous = employees::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?
        .photo_key;
    let row = employees::set_photo(&mut *tx, id, Some(&key))
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "photo_set",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    discard_object(&state, previous).await;
    Ok(Json(present(&state, row).await))
}

#[utoipa::path(
    delete, path = "/hr/employees/{id}/photo", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = Employee))
)]
async fn remove_photo(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    let previous = employees::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?
        .photo_key;
    let row = employees::set_photo(&mut *tx, id, None)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "photo_removed",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    discard_object(&state, previous).await;
    Ok(Json(present(&state, row).await))
}

/// Best effort: an orphaned object costs a few kilobytes, a failed request costs the user
/// their upload, so a delete that fails is logged and nothing more.
async fn discard_object(state: &AppState, key: Option<String>) {
    if let Some(key) = key
        && let Err(e) = state.storage.delete(&key).await
    {
        tracing::warn!(%key, error = %e, "could not delete replaced photo");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(name: Option<&str>) -> EmployeeBody {
        EmployeeBody {
            full_name: name.map(String::from),
            email: None,
            company_phone: None,
            personal_phone: None,
        }
    }

    #[test]
    fn create_needs_a_name_and_a_valid_email() {
        assert!(merge(None, body(None)).is_err());
        assert!(merge(None, body(Some("   "))).is_err());
        let ok = merge(None, body(Some("  Kiss Péter "))).unwrap();
        assert_eq!(ok.full_name, "Kiss Péter");

        let mut b = body(Some("Kiss Péter"));
        b.email = Some(Some("not-an-address".into()));
        assert!(merge(None, b).is_err());
        let mut b = body(Some("Kiss Péter"));
        b.email = Some(Some(" Peter@Example.HU ".into()));
        assert_eq!(
            merge(None, b).unwrap().email.as_deref(),
            Some("peter@example.hu")
        );
    }

    #[test]
    fn a_patch_keeps_absent_fields_and_clears_null_ones() {
        let current = EmployeeRow {
            id: 1,
            full_name: "Kiss Péter".into(),
            email: Some("peter@example.hu".into()),
            company_phone: Some("+36 30 111 2222".into()),
            personal_phone: Some("+36 20 333 4444".into()),
            photo_key: None,
            archived_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mut b = body(None);
        b.personal_phone = Some(None); // explicit null clears
        let merged = merge(Some(&current), b).unwrap();
        assert_eq!(merged.full_name, "Kiss Péter");
        assert_eq!(merged.email.as_deref(), Some("peter@example.hu"));
        assert_eq!(merged.company_phone.as_deref(), Some("+36 30 111 2222"));
        assert_eq!(merged.personal_phone, None);
    }

    #[test]
    fn overlong_values_are_refused() {
        let mut b = body(Some("Kiss Péter"));
        b.company_phone = Some(Some("1".repeat(51)));
        assert!(merge(None, b).is_err());
        assert!(merge(None, body(Some(&"é".repeat(201)))).is_err());
    }

    #[test]
    fn blank_optionals_become_null() {
        let mut b = body(Some("Kiss Péter"));
        b.company_phone = Some(Some("   ".into()));
        assert_eq!(merge(None, b).unwrap().company_phone, None);
    }
}
