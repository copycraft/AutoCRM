//! Email log, manual compose, templates and the suppression list.

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/emails", get(list).post(send))
        .route("/emails/preview", post(preview))
        .route("/emails/{id}", get(detail))
        .route("/emails/{id}/cancel", post(cancel))
        .route("/emails/{id}/retry", post(retry))
        .route(
            "/email-templates",
            get(list_templates).post(create_template),
        )
        .route("/email-templates/variables", get(variables))
        .route("/email-templates/{id}", patch(update_template))
        .route(
            "/email-suppressions",
            get(list_suppressions).post(add_suppression),
        )
        .route("/email-suppressions/{email}", delete(remove_suppression))
}

#[derive(Deserialize)]
struct ListQuery {
    order_id: Option<i64>,
    lead_id: Option<i64>,
    partner_id: Option<i64>,
    status: Option<EmailStatus>,
    #[serde(default)]
    attention: bool,
    limit: Option<i64>,
    offset: Option<i64>,
}

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

async fn preview(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(req): ApiJson<ComposeRequest>,
) -> AppResult<Json<Preview>> {
    me.require(Capability::SendEmail)?;
    Ok(Json(email_service::preview(&state, &me, &req).await?))
}

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

async fn list_templates(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<EmailTemplate>>> {
    Ok(Items::new(templates::list(&state.db).await?))
}

#[derive(Serialize)]
struct Variable {
    name: &'static str,
    description: &'static str,
}

async fn variables(Auth(_): Auth) -> Json<Items<Variable>> {
    Items::new(
        VARIABLES
            .iter()
            .map(|(name, description)| Variable { name, description })
            .collect(),
    )
}

fn check_variables(subject: &str, body: &str) -> AppResult<()> {
    let mut unknown = unknown_variables(subject);
    unknown.extend(unknown_variables(body));
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "unknown template variables: {}",
            unknown.join(", ")
        )))
    }
}

#[derive(Deserialize)]
struct CreateTemplate {
    key: String,
    name: String,
    subject: String,
    body: String,
}

async fn create_template(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateTemplate>,
) -> AppResult<(StatusCode, Json<EmailTemplate>)> {
    me.require(Capability::ManageConfiguration)?;
    let key = b.key.trim();
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
    let template = templates::insert(&mut *tx, key, &name, &subject, &body, me.user_id).await?;
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

#[derive(Deserialize)]
struct PatchTemplate {
    name: Option<String>,
    subject: Option<String>,
    body: Option<String>,
}

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
    let updated = templates::update(&mut *tx, id, &name, &subject, &body, me.user_id)
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
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

async fn list_suppressions(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<Suppression>>> {
    me.require(Capability::SendEmail)?;
    Ok(Items::new(emails::list_suppressions(&state.db).await?))
}

#[derive(Deserialize)]
struct AddSuppression {
    email: String,
    reason: Option<String>,
}

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
