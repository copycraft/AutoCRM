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
use crate::repo::{audit, employee_details, employee_documents, like_pattern};

const PHOTO_URL_TTL: Duration = Duration::from_secs(3600);
/// The picture is re-encoded to 512 px, so a phone original is far more than enough.
const MAX_PHOTO_BYTES: usize = 8 * 1024 * 1024;
const MAX_NAME: usize = 200;
const MAX_EMAIL: usize = 300;
const MAX_PHONE: usize = 50;
/// Scans of certificates and contracts.
const MAX_DOCUMENT_BYTES: usize = 25 * 1024 * 1024;

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
        .routes(routes!(get_details, put_details))
        .routes(routes!(list_documents, create_document))
        .routes(routes!(delete_document))
        .routes(routes!(expiring_documents))
        .routes(routes!(document_file_url))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(upload_document_file))
                .layer(DefaultBodyLimit::max(MAX_DOCUMENT_BYTES)),
        )
        // 0049
        .routes(routes!(set_user))
        .routes(routes!(list_checklist_items, create_checklist_item))
        .routes(routes!(update_checklist_item, archive_checklist_item))
        .routes(routes!(start_checklist))
}

#[utoipa::path(
    get, path = "/hr/employees/{id}/details", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = employee_details::EmployeeDetails))
)]
async fn get_details(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<employee_details::EmployeeDetails>> {
    me.require(Capability::AccessHr)?;
    let mut row = employee_details::get(&state.db, id)
        .await?
        .unwrap_or_else(|| employee_details::EmployeeDetails {
            employee_id: id,
            ..Default::default()
        });
    row.employee_id = id;
    Ok(Json(row))
}

#[utoipa::path(
    put, path = "/hr/employees/{id}/details", tag = "hr",
    params(("id" = i64, Path)),
    request_body = employee_details::EmployeeDetails,
    responses((status = 200, body = employee_details::EmployeeDetails))
)]
async fn put_details(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<employee_details::EmployeeDetails>,
) -> AppResult<Json<employee_details::EmployeeDetails>> {
    me.require(Capability::AccessHr)?;
    employees::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let mut tx = state.db.begin().await?;
    let saved = employee_details::upsert(&mut *tx, id, &b).await?;
    // Data drives the status: the employee's status follows the data only while it is
    // one of the four data-driven ones; HR's manual choices (absent, left, …) stay.
    if let Some(current) = employee_details::current_auto_key(&mut *tx, id).await? {
        if let Some(status_id) =
            employee_details::status_id_for_auto_key(&mut *tx, saved.auto_key()).await?
        {
            if current != saved.auto_key() {
                employees::set_status(&mut *tx, id, status_id).await?;
            }
        }
    }
    tx.commit().await?;
    Ok(Json(saved))
}

#[utoipa::path(
    get, path = "/hr/employees/{id}/documents", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<employee_documents::EmployeeDocument>))
)]
async fn list_documents(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<employee_documents::EmployeeDocument>>> {
    me.require(Capability::AccessHr)?;
    employees::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    Ok(Items::new(employee_documents::list(&state.db, id).await?))
}

#[utoipa::path(
    post, path = "/hr/employees/{id}/documents", tag = "hr",
    params(("id" = i64, Path)),
    request_body = employee_documents::NewDocument,
    responses((status = 201, body = employee_documents::EmployeeDocument))
)]
async fn create_document(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<employee_documents::NewDocument>,
) -> AppResult<(StatusCode, Json<employee_documents::EmployeeDocument>)> {
    me.require(Capability::AccessHr)?;
    employees::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(AppError::validation("title is required"));
    }
    if !employee_documents::kind_ok(&b.kind) {
        return Err(AppError::validation(
            "kind must be medical, contract, licence, training or other",
        ));
    }
    let mut clean = b;
    clean.title = title;
    // The file comes through its own upload; a client never names a storage key.
    clean.file_key = None;
    clean.file_name = None;
    clean.file_type = None;
    clean.file_size = None;
    let doc = employee_documents::insert(&state.db, id, &clean, me.user_id).await?;
    Ok((StatusCode::CREATED, Json(doc)))
}

#[utoipa::path(
    delete, path = "/hr/employees/{id}/documents/{doc_id}", tag = "hr",
    params(("id" = i64, Path), ("doc_id" = i64, Path)),
    responses((status = 204, description = "Removed"), (status = 404, description = "No such document"))
)]
async fn delete_document(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, doc_id)): ApiPath<(i64, i64)>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    if !employee_documents::soft_delete(&state.db, doc_id, id).await? {
        return Err(AppError::NotFound("document"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct DocumentFileQuery {
    /// The file's original name.
    filename: String,
}

/// The scan of a document: the raw file as the body (PDF, JPEG, PNG or WebP, at most
/// 25 MB), its name in `?filename=`. Replaces any earlier file.
#[utoipa::path(
    post, path = "/hr/employees/{id}/documents/{doc_id}/file", tag = "hr",
    params(("id" = i64, Path), ("doc_id" = i64, Path), DocumentFileQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses((status = 204, description = "Stored"), (status = 404, description = "No such document"))
)]
async fn upload_document_file(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, doc_id)): ApiPath<(i64, i64)>,
    ApiQuery(q): ApiQuery<DocumentFileQuery>,
    body: Bytes,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    if body.is_empty() {
        return Err(AppError::validation("the request body must be the file"));
    }
    let name: String = required("filename", &q.filename)?
        .chars()
        .take(200)
        .collect();
    let (content_type, ext) = super::incoming_invoices::sniff(&body)
        .filter(|(_, ext)| *ext != "xml")
        .ok_or_else(|| AppError::validation("only PDF, JPEG, PNG or WebP files"))?;
    let random: [u8; 12] = rand::random();
    let key = format!(
        "employee-documents/{id}/{doc_id}-{}.{ext}",
        hex::encode(random)
    );
    let size = body.len() as i64;
    state
        .storage
        .put_bytes(&key, body.to_vec(), content_type)
        .await
        .map_err(|e| AppError::internal(format!("storing employee document: {e}")))?;
    let mut tx = state.db.begin().await?;
    if !employee_documents::set_file(&mut *tx, doc_id, id, &key, &name, content_type, size).await? {
        return Err(AppError::NotFound("document"));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "document_file",
        json!({ "document_id": doc_id, "file_name": name }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct DocumentFileUrl {
    /// Presigned, expires after one hour; opens in the browser.
    url: String,
}

#[utoipa::path(
    get, path = "/hr/employees/{id}/documents/{doc_id}/file-url", tag = "hr",
    params(("id" = i64, Path), ("doc_id" = i64, Path)),
    responses((status = 200, body = DocumentFileUrl), (status = 404, description = "No file"))
)]
async fn document_file_url(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, doc_id)): ApiPath<(i64, i64)>,
) -> AppResult<Json<DocumentFileUrl>> {
    me.require(Capability::AccessHr)?;
    let (key, name) = employee_documents::file_of(&state.db, doc_id, id)
        .await?
        .ok_or(AppError::NotFound("document file"))?;
    let url = state
        .storage
        .presign_get(
            &key,
            PHOTO_URL_TTL,
            Some(crate::media::storage::content_disposition("inline", &name)),
        )
        .await?;
    Ok(Json(DocumentFileUrl { url }))
}

/// Documents that ran out or run out within 30 days, across all employees.
#[utoipa::path(
    get, path = "/hr/documents/expiring", tag = "hr",
    responses((status = 200, body = Items<employee_documents::ExpiringDocument>))
)]
async fn expiring_documents(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<employee_documents::ExpiringDocument>>> {
    me.require(Capability::AccessHr)?;
    let until = crate::service::business_today(state.config.business_tz)
        + chrono::TimeDelta::days(crate::service::reminders::DOCUMENT_EXPIRY_DAYS);
    Ok(Items::new(
        employee_documents::expiring(&state.db, until).await?,
    ))
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
    /// Paid annual leave per calendar year.
    pub annual_leave_days: i32,
    /// Where the employee stands (/hr/statuses).
    pub status_id: Option<i64>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The CRM account this person signs in with, if linked (0049).
    pub user_id: Option<i64>,
}

pub(super) async fn present(state: &AppState, row: EmployeeRow) -> Employee {
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
        annual_leave_days: row.annual_leave_days,
        status_id: row.status_id,
        archived_at: row.archived_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
        user_id: row.user_id,
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
    /// Only employees on this status (archived ones included when it is theirs).
    status: Option<i64>,
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
    let rows = employees::list(&state.db, pattern.as_deref(), q.include_archived, q.status).await?;
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
    /// Days of paid annual leave per year (0 to 366). Defaults to 20 on create.
    annual_leave_days: Option<i32>,
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
    let annual_leave_days = b
        .annual_leave_days
        .or_else(|| current.map(|e| e.annual_leave_days))
        .unwrap_or(20);
    if !(0..=366).contains(&annual_leave_days) {
        return Err(AppError::validation(
            "annual_leave_days must be between 0 and 366",
        ));
    }
    Ok(EmployeeInput {
        full_name,
        email,
        company_phone,
        personal_phone,
        annual_leave_days,
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
            annual_leave_days: None,
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
            annual_leave_days: 25,
            status_id: None,
            archived_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: None,
        };
        let mut b = body(None);
        b.personal_phone = Some(None); // explicit null clears
        let merged = merge(Some(&current), b).unwrap();
        assert_eq!(merged.full_name, "Kiss Péter");
        assert_eq!(merged.email.as_deref(), Some("peter@example.hu"));
        assert_eq!(merged.company_phone.as_deref(), Some("+36 30 111 2222"));
        assert_eq!(merged.personal_phone, None);
        // Absent keeps the allowance; an out-of-range one is refused.
        assert_eq!(merged.annual_leave_days, 25);
        let mut b = body(None);
        b.annual_leave_days = Some(400);
        assert!(merge(Some(&current), b).is_err());
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

// --- The CRM account, and joining/leaving checklists (0049) ---

use crate::repo::hr_checklists::{self, ChecklistItem};

#[derive(Deserialize, ToSchema)]
struct EmployeeUserBody {
    /// The user account; null unlinks.
    user_id: Option<i64>,
}

#[utoipa::path(
    put, path = "/hr/employees/{id}/user", tag = "hr",
    params(("id" = i64, Path)),
    request_body = EmployeeUserBody,
    responses((status = 200, body = Employee), (status = 409, description = "That account belongs to another employee (`duplicate`)"))
)]
async fn set_user(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<EmployeeUserBody>,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    if let Some(user_id) = b.user_id
        && crate::repo::users::find(&state.db, user_id)
            .await?
            .is_none()
    {
        return Err(AppError::validation("user_id is not a user"));
    }
    let mut tx = state.db.begin().await?;
    match employees::set_user(&mut *tx, id, b.user_id).await {
        Ok(true) => {}
        Ok(false) => return Err(AppError::NotFound("employee")),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Err(AppError::conflict(
                "duplicate",
                "that account is linked to another employee",
            ));
        }
        Err(e) => return Err(e.into()),
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "user_link",
        json!({ "user_id": b.user_id }),
    )
    .await?;
    let row = employees::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    tx.commit().await?;
    Ok(Json(present(&state, row).await))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ChecklistQuery {
    /// onboarding or offboarding; both when absent.
    kind: Option<String>,
}

#[utoipa::path(
    get, path = "/hr/checklist-items", tag = "hr",
    params(ChecklistQuery),
    responses((status = 200, body = Items<ChecklistItem>))
)]
async fn list_checklist_items(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<ChecklistQuery>,
) -> AppResult<Json<Items<ChecklistItem>>> {
    me.require(Capability::AccessHr)?;
    Ok(Items::new(
        hr_checklists::list(&state.db, q.kind.as_deref()).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct ChecklistItemBody {
    /// onboarding or offboarding (ignored on update).
    kind: Option<String>,
    title: String,
    /// Due this many days after the checklist is started (0–365).
    #[serde(default)]
    due_days: i32,
    /// Order within the list (update only).
    position: Option<i32>,
}

fn checked_item(b: &ChecklistItemBody) -> AppResult<String> {
    let title = required("title", &b.title)?;
    if title.chars().count() > 200 {
        return Err(AppError::validation("title is at most 200 characters"));
    }
    if !(0..=365).contains(&b.due_days) {
        return Err(AppError::validation("due_days must be 0-365"));
    }
    Ok(title)
}

#[utoipa::path(
    post, path = "/hr/checklist-items", tag = "hr",
    request_body = ChecklistItemBody,
    responses((status = 201, body = ChecklistItem))
)]
async fn create_checklist_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ChecklistItemBody>,
) -> AppResult<(StatusCode, Json<ChecklistItem>)> {
    me.require(Capability::AccessHr)?;
    let title = checked_item(&b)?;
    let kind = b.kind.as_deref().unwrap_or("");
    if !hr_checklists::KINDS.contains(&kind) {
        return Err(AppError::validation(
            "kind: expected onboarding or offboarding",
        ));
    }
    let item = hr_checklists::insert(&state.db, kind, &title, b.due_days).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(
    put, path = "/hr/checklist-items/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = ChecklistItemBody,
    responses((status = 200, body = ChecklistItem))
)]
async fn update_checklist_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ChecklistItemBody>,
) -> AppResult<Json<ChecklistItem>> {
    me.require(Capability::AccessHr)?;
    let title = checked_item(&b)?;
    let current = hr_checklists::list(&state.db, None)
        .await?
        .into_iter()
        .find(|i| i.id == id)
        .ok_or(AppError::NotFound("checklist item"))?;
    hr_checklists::update(
        &state.db,
        id,
        &title,
        b.due_days,
        b.position.unwrap_or(current.position),
    )
    .await?
    .map(Json)
    .ok_or(AppError::NotFound("checklist item"))
}

#[utoipa::path(
    delete, path = "/hr/checklist-items/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Removed from the list; tasks already made stay"))
)]
async fn archive_checklist_item(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    if !hr_checklists::archive(&state.db, id).await? {
        return Err(AppError::NotFound("checklist item"));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct StartChecklistBody {
    /// onboarding or offboarding.
    kind: String,
    /// First day (joining) or last day (leaving); tasks fall due from it. Defaults to today.
    start_date: Option<chrono::NaiveDate>,
    /// Who does the steps; defaults to the person starting the checklist.
    assigned_to: Option<i64>,
    /// Leaving only: switch off the linked CRM account and sign it out everywhere now.
    /// Needs the user-management right.
    #[serde(default)]
    deactivate_user: bool,
}

#[derive(Serialize, ToSchema)]
struct ChecklistStarted {
    tasks_created: usize,
    /// The linked account was switched off.
    user_deactivated: bool,
}

#[utoipa::path(
    post, path = "/hr/employees/{id}/checklists", tag = "hr",
    params(("id" = i64, Path)),
    request_body = StartChecklistBody,
    responses((status = 201, body = ChecklistStarted))
)]
async fn start_checklist(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<StartChecklistBody>,
) -> AppResult<(StatusCode, Json<ChecklistStarted>)> {
    me.require(Capability::AccessHr)?;
    if !hr_checklists::KINDS.contains(&b.kind.as_str()) {
        return Err(AppError::validation(
            "kind: expected onboarding or offboarding",
        ));
    }
    let deactivate = b.deactivate_user && b.kind == "offboarding";
    if deactivate {
        me.require(Capability::ManageUsers)?;
    }
    let assigned_to = b.assigned_to.unwrap_or(me.user_id);
    if crate::repo::users::find(&state.db, assigned_to)
        .await?
        .is_none()
    {
        return Err(AppError::validation("assigned_to is not a user"));
    }
    let start = b
        .start_date
        .unwrap_or_else(|| crate::service::business_today(state.config.business_tz));
    let mut tx = state.db.begin().await?;
    let employee = employees::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let created =
        hr_checklists::start(&mut tx, id, &b.kind, start, Some(assigned_to), me.user_id).await?;
    let mut user_deactivated = false;
    if deactivate && let Some(user_id) = employee.user_id {
        if user_id == me.user_id {
            return Err(AppError::validation(
                "you cannot switch off your own account",
            ));
        }
        let user = crate::repo::users::find(&mut *tx, user_id)
            .await?
            .ok_or(AppError::NotFound("user"))?;
        if user.is_active {
            if user.role == crate::domain::role::Role::Admin
                && crate::repo::users::count_active_admins(&mut *tx).await? <= 1
            {
                return Err(AppError::rule(
                    "last_admin",
                    "the last active admin cannot be switched off",
                ));
            }
            crate::repo::users::update(&mut *tx, user_id, None, None, Some(false), None, None).await?;
            crate::repo::sessions::revoke_all(&mut *tx, user_id, None).await?;
            user_deactivated = true;
        }
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "checklist_start",
        json!({ "kind": b.kind, "tasks": created, "start": start, "user_deactivated": user_deactivated }),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(ChecklistStarted {
            tasks_created: created,
            user_deactivated,
        }),
    ))
}
