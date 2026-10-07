//! Email log, manual compose, templates and the suppression list.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset, required};
use crate::AppState;
use crate::domain::email::{EmailStatus, normalize_address};
use crate::domain::role::Capability;
use crate::domain::template::{VARIABLES, unknown_variables};
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::repo::emails::{self, EmailFilter, EmailMessage, EmailSummary, Suppression};
use crate::repo::templates::{self, EmailTemplate};
use crate::service::email::{self as email_service, ComposeRequest, Preview};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, send))
        .routes(routes!(preview))
        .routes(routes!(detail))
        .routes(routes!(cancel))
        .routes(routes!(retry))
        .routes(routes!(list_templates, create_template))
        .routes(routes!(variables))
        .routes(routes!(update_template))
        .routes(routes!(copy_template))
        // 0049
        .routes(routes!(preview_template))
        .routes(routes!(list_suppressions, add_suppression))
        .routes(routes!(remove_suppression))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    order_id: Option<i64>,
    lead_id: Option<i64>,
    /// Includes mail about the partner's orders and leads.
    partner_id: Option<i64>,
    status: Option<EmailStatus>,
    /// Only failed and needs-review mail.
    #[serde(default)]
    attention: bool,
    /// Free text over subject and recipient.
    q: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/emails", tag = "email",
    params(ListQuery),
    responses((status = 200, body = Items<EmailSummary>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<EmailSummary>>> {
    let filter = EmailFilter {
        order_id: q.order_id,
        lead_id: q.lead_id,
        partner_id: q.partner_id,
        status: q.status,
        needs_attention: q.attention,
        q: q.q,
    };
    Ok(Items::new(
        emails::list(
            &state.db,
            &filter,
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[utoipa::path(
    get, path = "/emails/{id}", tag = "email",
    params(("id" = i64, Path)),
    responses((status = 200, body = EmailMessage))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<EmailMessage>> {
    Ok(Json(
        emails::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("email"))?,
    ))
}

#[utoipa::path(
    post, path = "/emails/preview", tag = "email",
    request_body = ComposeRequest,
    responses((status = 200, body = Preview))
)]
async fn preview(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(req): ApiJson<ComposeRequest>,
) -> AppResult<Json<Preview>> {
    me.require(Capability::SendEmail)?;
    Ok(Json(email_service::preview(&state, &me, &req).await?))
}

#[utoipa::path(
    post, path = "/emails", tag = "email",
    request_body(content = ComposeRequest, description = "Subject/body override the template. Unresolved variables are rejected."),
    responses((status = 202, description = "Queued", body = EmailMessage))
)]
async fn send(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(req): ApiJson<ComposeRequest>,
) -> AppResult<(StatusCode, Json<EmailMessage>)> {
    me.require(Capability::SendEmail)?;
    let id = email_service::send_manual(&state, &me, &req).await?;
    let email = emails::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("email"))?;
    Ok((StatusCode::ACCEPTED, Json(email)))
}

#[utoipa::path(
    post, path = "/emails/{id}/cancel", tag = "email",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Cancelled"))
)]
async fn cancel(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::SendEmail)?;
    let email = emails::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("email"))?;
    if email.sent_by != Some(me.user_id) && !me.can(Capability::OperateSystem) {
        return Err(AppError::Forbidden);
    }
    if !emails::cancel(&state.db, id, "cancelled by user", Some(me.user_id)).await? {
        return Err(AppError::conflict(
            "not_cancellable",
            "only queued, failed or needs-review emails can be cancelled",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post, path = "/emails/{id}/retry", tag = "email",
    params(("id" = i64, Path)),
    responses((status = 202, description = "Re-queued. Only failed or needs-review mail."))
)]
async fn retry(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::OperateSystem)?;
    email_service::retry(&state, id).await?;
    tracing::info!(
        email_id = id,
        user_id = me.user_id,
        "email manually re-queued"
    );
    Ok(StatusCode::ACCEPTED)
}

#[utoipa::path(
    get, path = "/email-templates", tag = "email",
    responses((status = 200, body = Items<EmailTemplate>))
)]
async fn list_templates(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<EmailTemplate>>> {
    Ok(Items::new(templates::list(&state.db).await?))
}

#[derive(Serialize, ToSchema)]
struct TemplateVariable {
    name: &'static str,
    description: &'static str,
}

#[utoipa::path(
    get, path = "/email-templates/variables", tag = "email",
    responses((status = 200, body = Items<TemplateVariable>))
)]
async fn variables(Auth(_): Auth) -> Json<Items<TemplateVariable>> {
    Items::new(
        VARIABLES
            .iter()
            .map(|(name, description)| TemplateVariable { name, description })
            .collect(),
    )
}

fn check_variables(subject: &str, body: &str) -> AppResult<()> {
    let mut unknown = unknown_variables(subject);
    for name in unknown_variables(body) {
        if !unknown.contains(&name) {
            unknown.push(name);
        }
    }
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "unknown template variables: {}",
            unknown.join(", ")
        )))
    }
}

const CATEGORIES: &[&str] = &["sales", "projects", "billing", "marketing", "hr", "general"];
const FOLDERS: &[&str] = &["customer", "workflow", "design"];

fn check_place(category: &str, folder: &str) -> AppResult<()> {
    if !CATEGORIES.contains(&category) {
        return Err(AppError::validation(format!(
            "category must be one of {}",
            CATEGORIES.join(", ")
        )));
    }
    if !FOLDERS.contains(&folder) {
        return Err(AppError::validation(format!(
            "folder must be one of {}",
            FOLDERS.join(", ")
        )));
    }
    Ok(())
}

#[derive(Deserialize, ToSchema)]
struct CreateTemplate {
    /// Permanent, for code to refer to. Omitted: made up from the time (custom_...).
    key: Option<String>,
    name: String,
    subject: String,
    body: String,
    /// sales | projects | billing | marketing | hr | general. Default general.
    category: Option<String>,
    /// customer | workflow | design. Default customer.
    folder: Option<String>,
}

#[utoipa::path(
    post, path = "/email-templates", tag = "email",
    request_body = CreateTemplate,
    responses((status = 201, body = EmailTemplate))
)]
async fn create_template(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateTemplate>,
) -> AppResult<(StatusCode, Json<EmailTemplate>)> {
    me.require(Capability::ManageConfiguration)?;
    let generated = format!("custom_{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f"));
    let key = b
        .key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .unwrap_or(&generated);
    let category = b.category.as_deref().unwrap_or("general");
    let folder = b.folder.as_deref().unwrap_or("customer");
    check_place(category, folder)?;
    let valid_key = key.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid_key {
        return Err(AppError::validation(
            "key must be lowercase letters, digits and underscores, starting with a letter",
        ));
    }
    let (name, subject, body) = (
        required("name", &b.name)?,
        required("subject", &b.subject)?,
        required("body", &b.body)?,
    );
    check_variables(&subject, &body)?;
    let mut tx = state.db.begin().await?;
    let template = templates::insert(
        &mut *tx, key, &name, &subject, &body, category, folder, me.user_id,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "email_template",
        template.id,
        "create",
        json!({ "key": key }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(template)))
}

#[derive(Deserialize, ToSchema)]
struct PatchTemplate {
    name: Option<String>,
    subject: Option<String>,
    body: Option<String>,
    category: Option<String>,
    folder: Option<String>,
    /// true moves it to the bin (Lomtár), false brings it back. A template that code or
    /// a follow-up sends cannot go to the bin.
    archived: Option<bool>,
}

#[utoipa::path(
    patch, path = "/email-templates/{id}", tag = "email",
    params(("id" = i64, Path)),
    request_body = PatchTemplate,
    responses((status = 200, body = EmailTemplate))
)]
async fn update_template(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchTemplate>,
) -> AppResult<Json<EmailTemplate>> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let current = templates::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("email template"))?;
    let name = required("name", &p.name.unwrap_or_else(|| current.name.clone()))?;
    let subject = required(
        "subject",
        &p.subject.unwrap_or_else(|| current.subject.clone()),
    )?;
    let body = required("body", &p.body.unwrap_or_else(|| current.body.clone()))?;
    check_variables(&subject, &body)?;
    let category = p.category.unwrap_or_else(|| current.category.clone());
    let folder = p.folder.unwrap_or_else(|| current.folder.clone());
    check_place(&category, &folder)?;
    let archived = p.archived.unwrap_or(current.archived_at.is_some());
    if archived
        && current.archived_at.is_none()
        && let Some(why) = templates::in_use(&mut *tx, &current.key, current.is_automatic).await?
    {
        return Err(AppError::rule(
            "template_in_use",
            format!("this template cannot go to the bin: {why}"),
        ));
    }
    let updated = templates::update(
        &mut *tx, id, &name, &subject, &body, &category, &folder, archived, me.user_id,
    )
    .await?
    .ok_or(AppError::NotFound("email template"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "email_template",
        id,
        "update",
        audit::diff(&[
            ("name", json!(current.name), json!(updated.name)),
            ("subject", json!(current.subject), json!(updated.subject)),
            ("body", json!(current.body), json!(updated.body)),
            ("category", json!(current.category), json!(updated.category)),
            ("folder", json!(current.folder), json!(updated.folder)),
            (
                "archived",
                json!(current.archived_at.is_some()),
                json!(updated.archived_at.is_some()),
            ),
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[utoipa::path(
    get, path = "/email-suppressions", tag = "email",
    responses((status = 200, body = Items<Suppression>))
)]
async fn list_suppressions(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<Suppression>>> {
    me.require(Capability::SendEmail)?;
    Ok(Items::new(emails::list_suppressions(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct AddSuppression {
    email: String,
    reason: Option<String>,
}

#[utoipa::path(
    post, path = "/email-suppressions", tag = "email",
    request_body = AddSuppression,
    responses((status = 204, description = "Added"))
)]
async fn add_suppression(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<AddSuppression>,
) -> AppResult<StatusCode> {
    me.require(Capability::SendEmail)?;
    let email = normalize_address(&b.email)
        .ok_or_else(|| AppError::validation("email is not a valid address"))?;
    emails::add_suppression(&state.db, &email, optional(b.reason).as_deref(), me.user_id).await?;
    tracing::info!(user_id = me.user_id, "address added to suppression list");
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete, path = "/email-suppressions/{email}", tag = "email",
    params(("email" = String, Path)),
    responses((status = 204, description = "Removed"))
)]
async fn remove_suppression(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(email): ApiPath<String>,
) -> AppResult<StatusCode> {
    // Removing an opt-out re-enables automatic mail to someone who asked not to get it.
    me.require(Capability::ManageConfiguration)?;
    if !emails::remove_suppression(&state.db, &email).await? {
        return Err(AppError::NotFound("suppression"));
    }
    tracing::warn!(
        user_id = me.user_id,
        "address removed from suppression list"
    );
    Ok(StatusCode::NO_CONTENT)
}

/// A copy to edit: "<name> (másolat)", same area and folder, a fresh key. How the design
/// samples are meant to be used, and how a customer letter gets a German twin.
#[utoipa::path(
    post, path = "/email-templates/{id}/copy", tag = "email",
    params(("id" = i64, Path)),
    responses((status = 201, body = EmailTemplate))
)]
async fn copy_template(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<(StatusCode, Json<EmailTemplate>)> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let source = templates::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("email template"))?;
    let key = format!("custom_{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f"));
    // A design sample's copy is a real letter for customers; anything else keeps its folder.
    let folder = if source.folder == "design" {
        "customer"
    } else {
        source.folder.as_str()
    };
    let copy = templates::insert(
        &mut *tx,
        &key,
        &format!("{} (másolat)", source.name),
        &source.subject,
        &source.body,
        &source.category,
        folder,
        me.user_id,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "email_template",
        copy.id,
        "create",
        json!({ "key": key, "copied_from": source.key }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(copy)))
}

#[derive(Deserialize, ToSchema)]
struct TemplatePreviewBody {
    subject: String,
    body: String,
    /// Render against this record (at most one); none shows every variable as missing.
    order_id: Option<i64>,
    lead_id: Option<i64>,
    partner_id: Option<i64>,
}

#[derive(Serialize, ToSchema)]
struct TemplatePreview {
    subject: String,
    body_text: String,
    body_html: String,
    /// Variables this record has no value for (or that do not exist).
    unresolved: Vec<String>,
}

/// A template as it would read for a real order, lead or partner, while it is being edited
/// (nothing is saved or sent). Letter-specific values (invoice, login, newsletter) only
/// exist when that letter is sent, so they show as missing here.
#[utoipa::path(
    post, path = "/email-templates/preview", tag = "email",
    request_body = TemplatePreviewBody,
    responses((status = 200, body = TemplatePreview))
)]
async fn preview_template(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<TemplatePreviewBody>,
) -> AppResult<Json<TemplatePreview>> {
    me.require(Capability::SendEmail)?;
    if [b.order_id, b.lead_id, b.partner_id]
        .iter()
        .flatten()
        .count()
        > 1
    {
        return Err(AppError::validation(
            "preview against at most one of order, lead or partner",
        ));
    }
    let about = email_service::About {
        order_id: b.order_id,
        lead_id: b.lead_id,
        partner_id: b.partner_id,
        blocker_id: None,
    };
    let mut conn = state.db.acquire().await?;
    let values = email_service::template_values(
        &mut conn,
        &about,
        Some(&me.display_name),
        state.config.business_tz,
    )
    .await?;
    let subject = crate::domain::template::render(&b.subject, &values);
    let body = crate::domain::template::render(&b.body, &values);
    let mut unresolved = subject.unresolved;
    unresolved.extend(body.unresolved);
    unresolved.sort();
    unresolved.dedup();
    Ok(Json(TemplatePreview {
        subject: crate::domain::template::single_line(&subject.output),
        body_html: crate::domain::template::email_html(&body.output),
        body_text: body.output,
        unresolved,
    }))
}
