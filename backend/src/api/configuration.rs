//! Stage definitions, project types and settings: everything that is configuration rather
//! than code, editable without a deploy.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveTime;
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, patch as patch_field, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::media::ImageCategory;
use crate::domain::role::Capability;
use crate::domain::stage::{StageDefinition, StageEntity, initial_stage, keys};
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::repo::config::{
    self, NewStageDefinition, ProjectType, Settings, SettingsUpdate, StageDefinitionUpdate,
};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_stages, create_stage))
        .routes(routes!(update_stage))
        .routes(routes!(list_project_types, create_project_type))
        .routes(routes!(update_project_type))
        .routes(routes!(get_settings, put_settings))
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    key.len() <= 50
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct StagesQuery {
    entity: Option<StageEntity>,
}

#[utoipa::path(
    get, path = "/stage-definitions", tag = "configuration",
    params(StagesQuery),
    responses((status = 200, body = Items<StageDefinition>))
)]
async fn list_stages(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<StagesQuery>,
) -> AppResult<Json<Items<StageDefinition>>> {
    let rows = match q.entity {
        Some(entity) => config::stage_definitions(&state.db, entity).await?,
        None => config::all_stage_definitions(&state.db).await?,
    };
    Ok(Items::new(rows))
}

#[derive(Deserialize, ToSchema)]
struct CreateStage {
    entity: StageEntity,
    /// Permanent; lowercase letters, digits and underscores.
    key: String,
    label_hu: String,
    position: i32,
    #[serde(default)]
    min_images: i32,
    required_image_category: Option<ImageCategory>,
    #[serde(default)]
    is_terminal: bool,
    #[serde(default)]
    is_exit: bool,
    stall_after_days: Option<i32>,
}

fn validate_gate(
    min_images: i32,
    category: Option<ImageCategory>,
    stall_after_days: Option<i32>,
) -> AppResult<()> {
    if min_images < 0 {
        return Err(AppError::validation("min_images cannot be negative"));
    }
    if min_images > 0 && category.is_none() {
        return Err(AppError::validation(
            "an image gate needs required_image_category",
        ));
    }
    if stall_after_days.is_some_and(|d| d <= 0) {
        return Err(AppError::validation("stall_after_days must be positive"));
    }
    Ok(())
}

#[utoipa::path(
    post, path = "/stage-definitions", tag = "configuration",
    request_body = CreateStage,
    responses((status = 201, body = StageDefinition))
)]
async fn create_stage(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateStage>,
) -> AppResult<(StatusCode, Json<StageDefinition>)> {
    me.require(Capability::ManageConfiguration)?;
    if !valid_key(&b.key) {
        return Err(AppError::validation(
            "key must be lowercase letters, digits and underscores, starting with a letter",
        ));
    }
    if b.is_exit && !b.is_terminal {
        return Err(AppError::validation("an exit stage must also be terminal"));
    }
    validate_gate(b.min_images, b.required_image_category, b.stall_after_days)?;
    let mut tx = state.db.begin().await?;
    let created = config::insert_stage_definition(
        &mut *tx,
        &NewStageDefinition {
            entity: b.entity,
            key: b.key,
            label_hu: required("label_hu", &b.label_hu)?,
            position: b.position,
            min_images: b.min_images,
            required_image_category: b.required_image_category,
            is_terminal: b.is_terminal,
            is_exit: b.is_exit,
            stall_after_days: b.stall_after_days,
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "stage_definition",
        created.id,
        "create",
        json!(created),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[derive(Deserialize, ToSchema)]
struct PatchStage {
    label_hu: Option<String>,
    position: Option<i32>,
    min_images: Option<i32>,
    #[serde(default, deserialize_with = "patch_field")]
    required_image_category: Option<Option<ImageCategory>>,
    #[serde(default, deserialize_with = "patch_field")]
    stall_after_days: Option<Option<i32>>,
    is_active: Option<bool>,
}

#[utoipa::path(
    patch, path = "/stage-definitions/{id}", tag = "configuration",
    params(("id" = i64, Path)),
    request_body = PatchStage,
    responses((status = 200, body = StageDefinition))
)]
async fn update_stage(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchStage>,
) -> AppResult<Json<StageDefinition>> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let current = config::find_stage_definition(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("stage definition"))?;
    let update = StageDefinitionUpdate {
        label_hu: required(
            "label_hu",
            &p.label_hu.unwrap_or_else(|| current.label_hu.clone()),
        )?,
        position: p.position.unwrap_or(current.position),
        min_images: p.min_images.unwrap_or(current.min_images),
        required_image_category: p
            .required_image_category
            .unwrap_or(current.required_image_category),
        stall_after_days: p.stall_after_days.unwrap_or(current.stall_after_days),
        is_active: p.is_active.unwrap_or(current.is_active),
    };
    validate_gate(
        update.min_images,
        update.required_image_category,
        update.stall_after_days,
    )?;
    if !update.is_active
        && current.entity == StageEntity::Lead.as_str()
        && current.key == keys::LEAD_WON
    {
        return Err(AppError::rule(
            "stage_required",
            "the 'won' lead stage is used by lead conversion and cannot be deactivated",
        ));
    }
    let updated = config::update_stage_definition(&mut *tx, id, &update)
        .await?
        .ok_or(AppError::NotFound("stage definition"))?;

    let entity = if updated.entity == StageEntity::Lead.as_str() {
        StageEntity::Lead
    } else {
        StageEntity::Order
    };
    let all = config::stage_definitions(&mut *tx, entity).await?;
    if initial_stage(&all).is_none() {
        return Err(AppError::rule(
            "stage_required",
            "at least one active, non-terminal stage must remain",
        ));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "stage_definition",
        id,
        "update",
        audit::diff(&[
            ("label_hu", json!(current.label_hu), json!(updated.label_hu)),
            ("position", json!(current.position), json!(updated.position)),
            (
                "min_images",
                json!(current.min_images),
                json!(updated.min_images),
            ),
            (
                "required_image_category",
                json!(current.required_image_category),
                json!(updated.required_image_category),
            ),
            (
                "stall_after_days",
                json!(current.stall_after_days),
                json!(updated.stall_after_days),
            ),
            (
                "is_active",
                json!(current.is_active),
                json!(updated.is_active),
            ),
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[utoipa::path(
    get, path = "/project-types", tag = "configuration",
    responses((status = 200, body = Items<ProjectType>))
)]
async fn list_project_types(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<ProjectType>>> {
    Ok(Items::new(config::project_types(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct CreateProjectType {
    key: String,
    label_hu: String,
    position: i32,
}

#[utoipa::path(
    post, path = "/project-types", tag = "configuration",
    request_body = CreateProjectType,
    responses((status = 201, body = ProjectType))
)]
async fn create_project_type(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateProjectType>,
) -> AppResult<(StatusCode, Json<ProjectType>)> {
    me.require(Capability::ManageConfiguration)?;
    if !valid_key(&b.key) {
        return Err(AppError::validation(
            "key must be lowercase letters, digits and underscores, starting with a letter",
        ));
    }
    let mut tx = state.db.begin().await?;
    let created = config::insert_project_type(
        &mut *tx,
        &b.key,
        &required("label_hu", &b.label_hu)?,
        b.position,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "project_type",
        created.id,
        "create",
        json!(created),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[derive(Deserialize, ToSchema)]
struct PatchProjectType {
    label_hu: Option<String>,
    position: Option<i32>,
    is_active: Option<bool>,
}

#[utoipa::path(
    patch, path = "/project-types/{id}", tag = "configuration",
    params(("id" = i64, Path)),
    request_body = PatchProjectType,
    responses((status = 200, body = ProjectType))
)]
async fn update_project_type(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchProjectType>,
) -> AppResult<Json<ProjectType>> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let current = config::project_types(&mut *tx)
        .await?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or(AppError::NotFound("project type"))?;
    let label = required(
        "label_hu",
        &p.label_hu.unwrap_or_else(|| current.label_hu.clone()),
    )?;
    let updated = config::update_project_type(
        &mut *tx,
        id,
        &label,
        p.position.unwrap_or(current.position),
        p.is_active.unwrap_or(current.is_active),
    )
    .await?
    .ok_or(AppError::NotFound("project type"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "project_type",
        id,
        "update",
        audit::diff(&[
            ("label_hu", json!(current.label_hu), json!(updated.label_hu)),
            ("position", json!(current.position), json!(updated.position)),
            (
                "is_active",
                json!(current.is_active),
                json!(updated.is_active),
            ),
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[utoipa::path(
    get, path = "/settings", tag = "configuration",
    responses((status = 200, body = Settings))
)]
async fn get_settings(State(state): State<AppState>, Auth(_): Auth) -> AppResult<Json<Settings>> {
    Ok(Json(config::settings(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct SettingsBody {
    /// The kill switch for automatic email.
    automatic_email_enabled: bool,
    max_auto_emails_per_recipient_day: i32,
    send_window_start: NaiveTime,
    send_window_end: NaiveTime,
    send_window_weekdays_only: bool,
    nudge_interval_days: i32,
    nudge_escalate_after: i32,
    stage_change_notifications: bool,
    #[serde(default)]
    stalled_alert_recipients: Vec<String>,
}

#[utoipa::path(
    put, path = "/settings", tag = "configuration",
    request_body = SettingsBody,
    responses((status = 200, body = Settings))
)]
async fn put_settings(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<SettingsBody>,
) -> AppResult<Json<Settings>> {
    me.require(Capability::ManageSettings)?;
    if b.send_window_start >= b.send_window_end {
        return Err(AppError::validation(
            "send_window_start must be before send_window_end",
        ));
    }
    if b.max_auto_emails_per_recipient_day < 0
        || b.nudge_interval_days < 1
        || b.nudge_escalate_after < 1
    {
        return Err(AppError::validation("limits must be positive"));
    }
    let recipients = b
        .stalled_alert_recipients
        .iter()
        .map(|r| {
            normalize_address(r)
                .ok_or_else(|| AppError::validation(format!("'{r}' is not a valid address")))
        })
        .collect::<AppResult<Vec<_>>>()?;

    let mut tx = state.db.begin().await?;
    let before = config::settings(&mut *tx).await?;
    let after = config::update_settings(
        &mut *tx,
        &SettingsUpdate {
            automatic_email_enabled: b.automatic_email_enabled,
            max_auto_emails_per_recipient_day: b.max_auto_emails_per_recipient_day,
            send_window_start: b.send_window_start,
            send_window_end: b.send_window_end,
            send_window_weekdays_only: b.send_window_weekdays_only,
            nudge_interval_days: b.nudge_interval_days,
            nudge_escalate_after: b.nudge_escalate_after,
            stage_change_notifications: b.stage_change_notifications,
            stalled_alert_recipients: recipients,
        },
        me.user_id,
    )
    .await?;
    if before.automatic_email_enabled != after.automatic_email_enabled {
        tracing::warn!(
            user_id = me.user_id,
            enabled = after.automatic_email_enabled,
            "automatic email kill switch changed"
        );
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "settings",
        1,
        "update",
        json!({ "before": before, "after": after }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}
