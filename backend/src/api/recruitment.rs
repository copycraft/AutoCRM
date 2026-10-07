//! HR recruitment: job listings with a generic public application form. HR (anyone with
//! `AccessHr`) writes the listing, publishes it and shares its link. Applicants need no
//! login: the form posts their details and resume, and that creates their profile. HR then
//! reads profiles, keeps notes and deletes the ones that do not get the job.

use std::time::Duration;

use axum::Json;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth};
use super::{Items, optional, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::media::storage::content_disposition;
use crate::repo::audit;
use crate::repo::recruitment::{self, ApplicationInput, ApplicationRow, PostingInput, PostingRow};
use crate::service;

const RESUME_URL_TTL: Duration = Duration::from_secs(900);
/// A resume is a document, not a portfolio.
const MAX_RESUME_BYTES: usize = 10 * 1024 * 1024;
/// The resume plus the form's text fields and the multipart framing.
const MAX_FORM_BYTES: usize = MAX_RESUME_BYTES + 256 * 1024;
const MAX_TITLE: usize = 200;
const MAX_LOCATION: usize = 200;
const MAX_DESCRIPTION: usize = 10_000;
const MAX_NAME: usize = 200;
const MAX_EMAIL: usize = 300;
const MAX_PHONE: usize = 50;
const MAX_CITY: usize = 200;
const MAX_MESSAGE: usize = 5_000;
const MAX_NOTES: usize = 10_000;
const MAX_FILENAME: usize = 150;
const MIN_AGE: i32 = 14;
const MAX_AGE: i32 = 100;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_jobs, create_job))
        .routes(routes!(job_detail, update_job, delete_job))
        .routes(routes!(publish_job))
        .routes(routes!(close_job))
        .routes(routes!(job_applications))
        .routes(routes!(update_application, delete_application))
        .routes(routes!(public_job))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(apply))
                .layer(DefaultBodyLimit::max(MAX_FORM_BYTES)),
        )
}

// ---------------------------------------------------------------------------------------
// Listings (HR)
// ---------------------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct JobPosting {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    /// `draft` (link exists, form refuses applications), `published` (form open) or
    /// `closed` (form says the position is filled).
    pub status: String,
    /// The link to share: the public application form of this listing.
    pub public_url: String,
    pub application_count: i64,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn public_url(state: &AppState, slug: &str) -> String {
    format!(
        "{}/hu/jobs/{slug}",
        state.config.public_base_url.trim_end_matches('/')
    )
}

fn present_job(state: &AppState, row: PostingRow) -> JobPosting {
    JobPosting {
        public_url: public_url(state, &row.slug),
        id: row.id,
        title: row.title,
        description: row.description,
        location: row.location,
        status: row.status,
        application_count: row.application_count,
        published_at: row.published_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

#[derive(Deserialize, ToSchema)]
struct JobBody {
    title: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    description: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    location: Option<Option<String>>,
}

fn capped(field: &str, value: Option<String>, max: usize) -> AppResult<Option<String>> {
    let value = optional(value);
    match &value {
        Some(v) if v.chars().count() > max => Err(AppError::validation(format!(
            "{field} is too long (at most {max} characters)"
        ))),
        _ => Ok(value),
    }
}

fn merge_job(current: Option<&PostingRow>, b: JobBody) -> AppResult<PostingInput> {
    let title = b
        .title
        .or_else(|| current.map(|p| p.title.clone()))
        .unwrap_or_default();
    let title = required("title", &title)?;
    if title.chars().count() > MAX_TITLE {
        return Err(AppError::validation(format!(
            "title is too long (at most {MAX_TITLE} characters)"
        )));
    }
    let description = capped(
        "description",
        patch_text(&current.and_then(|p| p.description.clone()), b.description),
        MAX_DESCRIPTION,
    )?;
    let location = capped(
        "location",
        patch_text(&current.and_then(|p| p.location.clone()), b.location),
        MAX_LOCATION,
    )?;
    Ok(PostingInput {
        title,
        description,
        location,
    })
}

#[utoipa::path(
    get, path = "/hr/jobs", tag = "hr",
    responses((status = 200, body = Items<JobPosting>))
)]
async fn list_jobs(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<JobPosting>>> {
    me.require(Capability::AccessHr)?;
    let rows = recruitment::list_postings(&state.db).await?;
    Ok(Items::new(
        rows.into_iter().map(|r| present_job(&state, r)).collect(),
    ))
}

/// Creates a draft and returns it with its link. Nothing is public until it is published.
#[utoipa::path(
    post, path = "/hr/jobs", tag = "hr",
    request_body = JobBody,
    responses((status = 201, body = JobPosting))
)]
async fn create_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<JobBody>,
) -> AppResult<(StatusCode, Json<JobPosting>)> {
    me.require(Capability::AccessHr)?;
    let input = merge_job(None, b)?;
    let slug = format!("{:016x}", rand::random::<u64>());
    let mut tx = state.db.begin().await?;
    let id = recruitment::insert_posting(&mut *tx, &input, &slug, me.user_id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_posting",
        id,
        "create",
        json!({ "title": input.title }),
    )
    .await?;
    let row = recruitment::find_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(present_job(&state, row))))
}

#[utoipa::path(
    get, path = "/hr/jobs/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = JobPosting))
)]
async fn job_detail(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<JobPosting>> {
    me.require(Capability::AccessHr)?;
    let row = recruitment::find_posting(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    Ok(Json(present_job(&state, row)))
}

#[utoipa::path(
    patch, path = "/hr/jobs/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = JobBody,
    responses((status = 200, body = JobPosting))
)]
async fn update_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<JobBody>,
) -> AppResult<Json<JobPosting>> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    recruitment::lock_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let current = recruitment::find_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let input = merge_job(Some(&current), b)?;
    recruitment::update_posting(&mut *tx, id, &input).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_posting",
        id,
        "update",
        json!({}),
    )
    .await?;
    let row = recruitment::find_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    tx.commit().await?;
    Ok(Json(present_job(&state, row)))
}

/// A listing nobody applied to can go. One with applicants cannot: close it instead, so a
/// profile is only ever deleted on purpose.
#[utoipa::path(
    delete, path = "/hr/jobs/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted"))
)]
async fn delete_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    recruitment::lock_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    if !recruitment::resume_keys(&mut *tx, id).await?.is_empty() {
        return Err(AppError::validation(
            "a job with applications cannot be deleted; close it instead",
        ));
    }
    recruitment::delete_posting(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_posting",
        id,
        "delete",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn set_status(
    state: &AppState,
    me: &service::auth::AuthUser,
    id: i64,
    status: &str,
) -> AppResult<Json<JobPosting>> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    recruitment::lock_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    recruitment::set_posting_status(&mut *tx, id, status).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_posting",
        id,
        status_action(status),
        json!({}),
    )
    .await?;
    let row = recruitment::find_posting(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    tx.commit().await?;
    Ok(Json(present_job(state, row)))
}

fn status_action(status: &str) -> &'static str {
    if status == "published" {
        "publish"
    } else {
        "close"
    }
}

/// Opens the form: the link starts accepting applications (also reopens a closed listing).
#[utoipa::path(
    post, path = "/hr/jobs/{id}/publish", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = JobPosting))
)]
async fn publish_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<JobPosting>> {
    set_status(&state, &me, id, "published").await
}

/// Stops accepting applications; the link then says the position is closed.
#[utoipa::path(
    post, path = "/hr/jobs/{id}/close", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = JobPosting))
)]
async fn close_job(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<JobPosting>> {
    set_status(&state, &me, id, "closed").await
}

// ---------------------------------------------------------------------------------------
// Applicant profiles (HR)
// ---------------------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct Application {
    pub id: i64,
    pub job_id: i64,
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub age: i32,
    pub city: Option<String>,
    pub message: Option<String>,
    pub resume_filename: String,
    /// A short-lived download link to the resume (about 15 minutes); None if it cannot be
    /// signed.
    pub resume_url: Option<String>,
    /// HR's own notes, e.g. from the phone interview.
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

async fn present_application(state: &AppState, row: ApplicationRow) -> Application {
    let resume_url = match state
        .storage
        .presign_get(
            &row.resume_key,
            RESUME_URL_TTL,
            Some(content_disposition("attachment", &row.resume_filename)),
        )
        .await
    {
        Ok(url) => Some(url),
        Err(e) => {
            tracing::warn!(application_id = row.id, error = %e, "could not sign resume url");
            None
        }
    };
    Application {
        id: row.id,
        job_id: row.posting_id,
        full_name: row.full_name,
        email: row.email,
        phone: row.phone,
        age: row.age,
        city: row.city,
        message: row.message,
        resume_filename: row.resume_filename,
        resume_url,
        notes: row.notes,
        created_at: row.created_at,
    }
}

#[utoipa::path(
    get, path = "/hr/jobs/{id}/applications", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Application>))
)]
async fn job_applications(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<Application>>> {
    me.require(Capability::AccessHr)?;
    recruitment::find_posting(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("job"))?;
    let rows = recruitment::list_applications(&state.db, id).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(present_application(&state, row).await);
    }
    Ok(Items::new(items))
}

#[derive(Deserialize, ToSchema)]
struct ApplicationPatch {
    /// `null` or blank clears the notes.
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
}

#[utoipa::path(
    patch, path = "/hr/applications/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = ApplicationPatch,
    responses((status = 200, body = Application))
)]
async fn update_application(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ApplicationPatch>,
) -> AppResult<Json<Application>> {
    me.require(Capability::AccessHr)?;
    let current = recruitment::find_application(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("application"))?;
    let notes = capped("notes", patch_text(&current.notes, b.notes), MAX_NOTES)?;
    let row = recruitment::set_notes(&state.db, id, notes.as_deref())
        .await?
        .ok_or(AppError::NotFound("application"))?;
    Ok(Json(present_application(&state, row).await))
}

/// Deletes the profile and its resume file: the applicant is not getting the job. The
/// audit log keeps only that it happened, not who they were.
#[utoipa::path(
    delete, path = "/hr/applications/{id}", tag = "hr",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted"))
)]
async fn delete_application(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    let mut tx = state.db.begin().await?;
    let key = recruitment::delete_application(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("application"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "job_application",
        id,
        "delete",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    // Best effort: an orphaned file is a few hundred kilobytes; a failed request would
    // leave HR with a profile they meant to remove.
    if let Err(e) = state.storage.delete(&key).await {
        tracing::warn!(%key, error = %e, "could not delete resume of a deleted application");
    }
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------------------
// The public form (no login)
// ---------------------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
pub struct PublicJob {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    /// `published` or `closed`. A draft is not found.
    pub status: String,
}

/// What the application page shows for a listing's link. Drafts do not exist publicly.
#[utoipa::path(
    get, path = "/public/jobs/{slug}", tag = "recruitment",
    params(("slug" = String, Path)),
    security(()),
    responses(
        (status = 200, body = PublicJob),
        (status = 404, description = "No such listing, or not yet published"),
    )
)]
async fn public_job(
    State(state): State<AppState>,
    ApiPath(slug): ApiPath<String>,
) -> AppResult<Json<PublicJob>> {
    let row = recruitment::find_posting_by_slug(&state.db, &slug)
        .await?
        .filter(|p| p.status != "draft")
        .ok_or(AppError::NotFound("job"))?;
    Ok(Json(PublicJob {
        title: row.title,
        description: row.description,
        location: row.location,
        status: row.status,
    }))
}

/// The application form as multipart/form-data. Documented as a schema, parsed by hand.
#[derive(ToSchema)]
#[allow(dead_code)]
struct ApplyForm {
    full_name: String,
    email: String,
    phone: String,
    age: i32,
    city: Option<String>,
    message: Option<String>,
    /// A PDF, DOC, DOCX, ODT or RTF file, up to 10 MB.
    #[schema(format = Binary)]
    resume: Vec<u8>,
    /// Honeypot: real visitors leave it empty; a filled one is silently dropped.
    company: Option<String>,
}

/// The text fields as the visitor typed them, before validation.
#[derive(Default)]
struct RawForm {
    full_name: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    age: Option<String>,
    city: Option<String>,
    message: Option<String>,
    company: Option<String>,
    resume: Option<(String, Vec<u8>)>,
}

fn bad_form(e: impl std::fmt::Display) -> AppError {
    AppError::validation(format!("could not read the form: {e}"))
}

async fn read_form(mut multipart: Multipart) -> AppResult<RawForm> {
    let mut form = RawForm::default();
    while let Some(field) = multipart.next_field().await.map_err(bad_form)? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "resume" {
            let filename = field.file_name().unwrap_or("resume").to_string();
            let bytes = field.bytes().await.map_err(bad_form)?;
            form.resume = Some((filename, bytes.to_vec()));
            continue;
        }
        let text = field.text().await.map_err(bad_form)?;
        match name.as_str() {
            "full_name" => form.full_name = Some(text),
            "email" => form.email = Some(text),
            "phone" => form.phone = Some(text),
            "age" => form.age = Some(text),
            "city" => form.city = Some(text),
            "message" => form.message = Some(text),
            "company" => form.company = Some(text),
            _ => {}
        }
    }
    Ok(form)
}

/// The resume's file type, from its first bytes: the filename and the browser's
/// content type are whatever the visitor's machine claims. Returns the extension and a
/// content type to serve it with.
fn sniff_resume(filename: &str, bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if bytes.starts_with(b"%PDF-") {
        return Some(("pdf", "application/pdf"));
    }
    if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Some(("doc", "application/msword"));
    }
    if bytes.starts_with(b"{\\rtf") {
        return Some(("rtf", "application/rtf"));
    }
    // DOCX and ODT are zip files: the signature alone cannot tell them from any other zip,
    // so the extension has to agree.
    if bytes.starts_with(b"PK\x03\x04") {
        return match ext.as_str() {
            "docx" => Some((
                "docx",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )),
            "odt" => Some(("odt", "application/vnd.oasis.opendocument.text")),
            _ => None,
        };
    }
    None
}

/// The filename as shown to HR: no path, no control characters, bounded.
fn clean_filename(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base.chars().filter(|c| !c.is_control()).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return "resume".into();
    }
    if cleaned.chars().count() <= MAX_FILENAME {
        return cleaned.to_string();
    }
    // Keep the extension when cutting a long name.
    let (stem, ext) = match cleaned.rsplit_once('.') {
        Some((s, e)) if e.chars().count() <= 8 => (s, format!(".{e}")),
        _ => (cleaned, String::new()),
    };
    let keep = MAX_FILENAME.saturating_sub(ext.chars().count());
    format!("{}{ext}", stem.chars().take(keep).collect::<String>())
}

/// The visitor's details, validated. The resume is checked separately.
fn validate_form(f: &RawForm) -> AppResult<ApplicationInput> {
    let full_name = capped("full_name", f.full_name.clone(), MAX_NAME)?
        .ok_or_else(|| AppError::validation("full_name is required"))?;
    let email = capped("email", f.email.clone(), MAX_EMAIL)?
        .ok_or_else(|| AppError::validation("email is required"))?;
    let email = normalize_address(&email)
        .ok_or_else(|| AppError::validation("email is not a valid address"))?;
    let phone = capped("phone", f.phone.clone(), MAX_PHONE)?
        .ok_or_else(|| AppError::validation("phone is required"))?;
    let age: i32 = f
        .age
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| AppError::validation("age is required"))?
        .parse()
        .map_err(|_| AppError::validation("age must be a number"))?;
    if !(MIN_AGE..=MAX_AGE).contains(&age) {
        return Err(AppError::validation(format!(
            "age must be between {MIN_AGE} and {MAX_AGE}"
        )));
    }
    Ok(ApplicationInput {
        full_name,
        email,
        phone,
        age,
        city: capped("city", f.city.clone(), MAX_CITY)?,
        message: capped("message", f.message.clone(), MAX_MESSAGE)?,
        resume_key: String::new(),
        resume_filename: String::new(),
        resume_content_type: String::new(),
    })
}

/// Apply to a listing: `multipart/form-data` with the details and the resume file. The
/// resume is stored and a profile is created for HR in one go; there is no login. Only a
/// published listing takes applications.
#[utoipa::path(
    post, path = "/public/jobs/{slug}/applications", tag = "recruitment",
    params(("slug" = String, Path)),
    security(()),
    request_body(content = ApplyForm, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Application received"),
        (status = 400, description = "Invalid form or unusable resume"),
        (status = 404, description = "No such listing, or not yet published"),
    )
)]
async fn apply(
    State(state): State<AppState>,
    ApiPath(slug): ApiPath<String>,
    multipart: Multipart,
) -> AppResult<StatusCode> {
    let posting = recruitment::find_posting_by_slug(&state.db, &slug)
        .await?
        .filter(|p| p.status != "draft")
        .ok_or(AppError::NotFound("job"))?;
    let form = read_form(multipart).await?;
    if optional(form.company.clone()).is_some() {
        return Ok(StatusCode::CREATED);
    }
    if posting.status != "published" {
        return Err(AppError::validation("this position is no longer open"));
    }
    let mut input = validate_form(&form)?;

    let (filename, bytes) = form
        .resume
        .ok_or_else(|| AppError::validation("resume is required"))?;
    if bytes.is_empty() {
        return Err(AppError::validation("resume is empty"));
    }
    if bytes.len() > MAX_RESUME_BYTES {
        return Err(AppError::validation("resume is too large (at most 10 MB)"));
    }
    let (ext, content_type) = sniff_resume(&filename, &bytes)
        .ok_or_else(|| AppError::validation("resume must be a PDF, DOC, DOCX, ODT or RTF file"))?;

    // One profile per person per listing: a double-click or a second try is not a second
    // candidate.
    if recruitment::list_applications(&state.db, posting.id)
        .await?
        .iter()
        .any(|a| a.email == input.email)
    {
        return Err(AppError::validation(
            "you have already applied to this position",
        ));
    }

    let key = format!(
        "hr/applications/{}/{:016x}.{ext}",
        posting.id,
        rand::random::<u64>()
    );
    state
        .storage
        .put_bytes(&key, bytes, content_type)
        .await
        .map_err(|e| AppError::internal(format!("storing resume: {e}")))?;
    input.resume_key = key.clone();
    input.resume_filename = clean_filename(&filename);
    input.resume_content_type = content_type.to_string();

    let row = match recruitment::insert_application(&state.db, posting.id, &input).await {
        Ok(row) => row,
        Err(e) => {
            // No profile points at the file, so nothing else would ever remove it.
            if let Err(d) = state.storage.delete(&key).await {
                tracing::warn!(%key, error = %d, "could not remove a resume after a failed insert");
            }
            return Err(e.into());
        }
    };
    // The profile is committed; a failed alert is logged, never the applicant's problem.
    if let Err(e) = service::notifications::application_arrived(
        &state.db,
        posting.id,
        &posting.title,
        &row.full_name,
    )
    .await
    {
        tracing::error!(application_id = row.id, error = %e, "could not raise the application notification");
    }
    Ok(StatusCode::CREATED)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> RawForm {
        RawForm {
            full_name: Some("  Kiss Péter ".into()),
            email: Some(" Peter@Example.HU ".into()),
            phone: Some("+36 30 123 4567".into()),
            age: Some(" 27 ".into()),
            ..RawForm::default()
        }
    }

    #[test]
    fn a_complete_form_is_cleaned() {
        let ok = validate_form(&form()).unwrap();
        assert_eq!(ok.full_name, "Kiss Péter");
        assert_eq!(ok.email, "peter@example.hu");
        assert_eq!(ok.age, 27);
        assert_eq!(ok.city, None);
    }

    #[test]
    fn name_email_phone_and_age_are_required() {
        for blank in ["full_name", "email", "phone", "age"] {
            let mut f = form();
            match blank {
                "full_name" => f.full_name = Some("  ".into()),
                "email" => f.email = None,
                "phone" => f.phone = Some("".into()),
                _ => f.age = None,
            }
            assert!(validate_form(&f).is_err(), "{blank} should be required");
        }
    }

    #[test]
    fn age_must_be_a_sensible_number() {
        for bad in ["abc", "13", "101", "-5", "27.5"] {
            let mut f = form();
            f.age = Some(bad.into());
            assert!(validate_form(&f).is_err(), "{bad} should be refused");
        }
        let mut f = form();
        f.age = Some("14".into());
        assert!(validate_form(&f).is_ok());
    }

    #[test]
    fn an_invalid_email_is_refused() {
        let mut f = form();
        f.email = Some("not-an-address".into());
        assert!(validate_form(&f).is_err());
    }

    #[test]
    fn overlong_text_is_refused() {
        let mut f = form();
        f.message = Some("x".repeat(MAX_MESSAGE + 1));
        assert!(validate_form(&f).is_err());
    }

    #[test]
    fn resumes_are_recognised_by_content_not_by_name() {
        assert_eq!(sniff_resume("cv.pdf", b"%PDF-1.7 ...").unwrap().0, "pdf");
        // A renamed executable is not a PDF.
        assert!(sniff_resume("cv.pdf", b"MZ\x90\x00").is_none());
        let ole = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1, 0, 0];
        assert_eq!(sniff_resume("cv.doc", &ole).unwrap().0, "doc");
        assert_eq!(sniff_resume("cv.docx", b"PK\x03\x04..").unwrap().0, "docx");
        assert_eq!(sniff_resume("CV.ODT", b"PK\x03\x04..").unwrap().0, "odt");
        // A zip is only accepted when it claims to be a document.
        assert!(sniff_resume("holiday.zip", b"PK\x03\x04..").is_none());
        assert_eq!(sniff_resume("cv.rtf", b"{\\rtf1\\ansi").unwrap().0, "rtf");
        assert!(sniff_resume("cv.pdf", b"").is_none());
    }

    #[test]
    fn filenames_lose_paths_and_stay_bounded() {
        assert_eq!(
            clean_filename("C:\\Users\\x\\Önéletrajz.pdf"),
            "Önéletrajz.pdf"
        );
        assert_eq!(clean_filename("../../etc/passwd"), "passwd");
        assert_eq!(clean_filename("  "), "resume");
        let long = format!("{}.pdf", "a".repeat(400));
        let cut = clean_filename(&long);
        assert_eq!(cut.chars().count(), MAX_FILENAME);
        assert!(cut.ends_with(".pdf"));
    }

    #[test]
    fn a_job_needs_a_title_and_a_patch_keeps_the_rest() {
        let body = |t: Option<&str>| JobBody {
            title: t.map(String::from),
            description: None,
            location: None,
        };
        assert!(merge_job(None, body(None)).is_err());
        assert!(merge_job(None, body(Some("  "))).is_err());
        assert_eq!(
            merge_job(None, body(Some(" Lakatos "))).unwrap().title,
            "Lakatos"
        );

        let current = PostingRow {
            id: 1,
            title: "Lakatos".into(),
            description: Some("Műszakban".into()),
            location: Some("Budapest".into()),
            slug: "abc".into(),
            status: "draft".into(),
            published_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            application_count: 0,
        };
        let mut b = body(None);
        b.location = Some(None); // explicit null clears
        let merged = merge_job(Some(&current), b).unwrap();
        assert_eq!(merged.title, "Lakatos");
        assert_eq!(merged.description.as_deref(), Some("Műszakban"));
        assert_eq!(merged.location, None);
    }
}
