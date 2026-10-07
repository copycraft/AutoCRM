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
use super::{Items, optional, patch as patch_field, required};
use crate::AppState;
use crate::config::{AppEnv, check_smtp_target};
use crate::domain::email::normalize_address;
use crate::domain::lookups::Lookups;
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
        .routes(routes!(lookups))
        .routes(routes!(stage_photo_categories))
        .routes(routes!(set_stage_photo_category))
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
    /// `heating` | `cooling` | omitted. Decides which build-spec section the order form
    /// shows for this type; omitted means the type has no build spec.
    spec_form: Option<String>,
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
    let spec_form = validate_spec_form(b.spec_form)?;
    let mut tx = state.db.begin().await?;
    let created = config::insert_project_type(
        &mut *tx,
        &b.key,
        &required("label_hu", &b.label_hu)?,
        b.position,
        spec_form.as_deref(),
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
    /// Absent keeps the current form; explicit null removes the build-spec section.
    #[serde(default, deserialize_with = "patch_field")]
    spec_form: Option<Option<String>>,
}

/// `heating` | `cooling` | none. A typo here would silently hide the spec section on
/// every order of that type, so it is rejected rather than ignored.
fn validate_spec_form(value: Option<String>) -> AppResult<Option<String>> {
    match optional(value) {
        Some(f) if matches!(f.as_str(), "heating" | "cooling") => Ok(Some(f)),
        Some(_) => Err(AppError::validation("spec_form must be heating or cooling")),
        None => Ok(None),
    }
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
    let spec_form = match p.spec_form {
        Some(v) => validate_spec_form(v)?,
        None => current.spec_form.clone(),
    };
    let updated = config::update_project_type(
        &mut *tx,
        id,
        &label,
        p.position.unwrap_or(current.position),
        p.is_active.unwrap_or(current.is_active),
        spec_form.as_deref(),
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

/// Every client-facing enumeration in one document: damage types, severities,
/// verdicts, fuel levels, task entity types, currencies, invoice payment
/// methods, annulment codes, image categories and email composer starters.
///
/// Any signed-in user may read it (like the zone lists): the phone fetches it
/// on the Átvétel-átadás screen and caches it for the yard, the web keeps it
/// in react-query. Values and labels come from `domain::lookups`, next to the
/// validation that accepts them — changing one is a server deploy, never an
/// app update.
#[utoipa::path(
    get, path = "/config/lookups", tag = "configuration",
    responses((status = 200, body = Lookups))
)]
async fn lookups(Auth(_): Auth) -> AppResult<Json<Lookups>> {
    Ok(Json(Lookups::current()))
}

/// Email transport overrides. Every field is optional: absent keeps the stored
/// value, explicit null returns it to "inherit from the environment".
/// `mode: null` clears the whole transport back to environment behaviour.
/// `smtp_password`: absent keeps, null clears, a value replaces. Blank
/// strings are treated as absent everywhere (never a destructive surprise).
#[derive(Debug, Default, Deserialize, ToSchema)]
struct EmailTransportBody {
    #[serde(default, deserialize_with = "patch_field")]
    mode: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_host: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_port: Option<Option<i32>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_security: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_username: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_password: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_helo_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    smtp_force_ipv4: Option<Option<bool>>,
    #[serde(default, deserialize_with = "patch_field")]
    redirect_to: Option<Option<String>>,
}

/// One merged transport field set, ready to validate and store.
/// `None` everywhere means "inherit from the environment".
#[derive(Debug, Default, PartialEq)]
pub(crate) struct MergedTransport {
    pub(crate) mode: Option<String>,
    pub(crate) host: Option<String>,
    pub(crate) port: Option<i32>,
    pub(crate) security: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) helo_name: Option<String>,
    pub(crate) force_ipv4: Option<bool>,
    pub(crate) redirect_to: Option<String>,
}

fn blank_to_none(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn merge_transport(current: &Settings, b: &EmailTransportBody) -> MergedTransport {
    if b.mode == Some(None) || matches!(&b.mode, Some(v) if v.as_deref().map(str::trim) == Some(""))
    {
        // Back to environment behaviour: siblings are meaningless, drop them.
        return MergedTransport::default();
    }
    let pick = |field: &Option<Option<String>>, cur: &Option<String>| match field {
        None => cur.clone(),
        Some(v) => blank_to_none(v.clone()),
    };
    MergedTransport {
        mode: match &b.mode {
            None => current.email_mode.clone(),
            // Explicit null was handled above (inherit everything).
            Some(v) => blank_to_none(v.clone()),
        },
        host: pick(&b.smtp_host, &current.smtp_host),
        port: match &b.smtp_port {
            None => current.smtp_port,
            Some(v) => *v,
        },
        security: pick(&b.smtp_security, &current.smtp_security),
        username: pick(&b.smtp_username, &current.smtp_username),
        helo_name: pick(&b.smtp_helo_name, &current.smtp_helo_name),
        force_ipv4: match &b.smtp_force_ipv4 {
            None => current.smtp_force_ipv4,
            Some(v) => *v,
        },
        redirect_to: pick(&b.redirect_to, &current.redirect_to),
    }
}

pub(crate) fn validate_transport(m: &MergedTransport, env: AppEnv) -> AppResult<()> {
    let mode = match m.mode.as_deref() {
        None => return Ok(()),
        Some(mode) => mode,
    };
    if mode != "dry_run" && mode != "smtp" {
        return Err(AppError::validation("email_mode: expected dry_run or smtp"));
    }
    if mode == "dry_run" {
        return Ok(());
    }
    let host = m.host.as_deref().unwrap_or("");
    if host.is_empty() {
        return Err(AppError::validation(
            "smtp_host is required when email_mode is smtp",
        ));
    }
    if let Some(port) = m.port {
        if !(1..=65535).contains(&port) {
            return Err(AppError::validation("smtp_port must be 1-65535"));
        }
    }
    if let Some(security) = m.security.as_deref() {
        if !["none", "starttls", "tls"].contains(&security) {
            return Err(AppError::validation(
                "smtp_security: expected none, starttls or tls",
            ));
        }
    }
    if let Some(helo) = m.helo_name.as_deref() {
        if helo.chars().any(char::is_whitespace) {
            return Err(AppError::validation(
                "smtp_helo_name must be a bare domain name",
            ));
        }
    }
    if let Some(redirect) = m.redirect_to.as_deref() {
        if normalize_address(redirect).is_none() {
            return Err(AppError::validation(
                "redirect_to is not a valid email address",
            ));
        }
    }
    if let Err(e) = check_smtp_target(env, host, m.redirect_to.as_deref()) {
        return Err(AppError::validation(e));
    }
    Ok(())
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
    /// Email transport overrides; absent keeps everything stored.
    #[serde(default)]
    email: EmailTransportBody,
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
    let email = merge_transport(&before, &b.email);
    validate_transport(&email, state.config.env)?;
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
            email_mode: email.mode,
            smtp_host: email.host,
            smtp_port: email.port,
            smtp_security: email.security,
            smtp_username: email.username,
            // Sealed at rest (0049); blank is "keep", like every other field here.
            smtp_password: match b.email.smtp_password.clone() {
                Some(Some(p)) if p.trim().is_empty() => None,
                other => other.map(|v| v.map(|p| crate::service::secrets::seal(&state.config, &p))),
            },
            smtp_helo_name: email.helo_name,
            smtp_force_ipv4: email.force_ipv4,
            redirect_to: email.redirect_to,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::config::Settings;

    fn stored() -> Settings {
        Settings {
            automatic_email_enabled: false,
            max_auto_emails_per_recipient_day: 3,
            send_window_start: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            send_window_end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            send_window_weekdays_only: true,
            nudge_interval_days: 3,
            nudge_escalate_after: 2,
            stage_change_notifications: false,
            stalled_alert_recipients: vec![],
            updated_at: chrono::Utc::now(),
            updated_by: None,
            email_mode: Some("smtp".into()),
            smtp_host: Some("smtp-relay.gmail.com".into()),
            smtp_port: Some(587),
            smtp_security: Some("starttls".into()),
            smtp_username: None,
            has_password: false,
            smtp_helo_name: None,
            smtp_force_ipv4: None,
            redirect_to: Some("staging@example.hu".into()),
        }
    }

    fn body() -> EmailTransportBody {
        EmailTransportBody::default()
    }

    #[test]
    fn absent_fields_keep_stored_values() {
        let merged = merge_transport(&stored(), &body());
        assert_eq!(merged.mode.as_deref(), Some("smtp"));
        assert_eq!(merged.host.as_deref(), Some("smtp-relay.gmail.com"));
        assert_eq!(merged.redirect_to.as_deref(), Some("staging@example.hu"));
    }

    #[test]
    fn explicit_null_mode_clears_the_whole_transport() {
        let mut b = body();
        b.mode = Some(None);
        b.smtp_host = Some(Some("smtp-relay.gmail.com".into()));
        let merged = merge_transport(&stored(), &b);
        assert_eq!(merged, MergedTransport::default());
    }

    #[test]
    fn explicit_values_replace_stored_ones() {
        let mut b = body();
        b.smtp_host = Some(Some("mailpit".into()));
        b.smtp_port = Some(Some(1025));
        let merged = merge_transport(&stored(), &b);
        assert_eq!(merged.host.as_deref(), Some("mailpit"));
        assert_eq!(merged.port, Some(1025));
        assert_eq!(merged.mode.as_deref(), Some("smtp"));
    }

    #[test]
    fn validation_rejects_bad_transport() {
        let env = AppEnv::Dev;
        let mut m = MergedTransport {
            mode: Some("smtp".into()),
            ..MergedTransport::default()
        };
        assert!(validate_transport(&m, env).is_err());
        m.host = Some("smtp-relay.gmail.com".into());
        assert!(validate_transport(&m, env).is_err());
        m.redirect_to = Some("staging@example.hu".into());
        assert!(validate_transport(&m, env).is_ok());
        m.host = Some("localhost".into());
        m.redirect_to = None;
        assert!(validate_transport(&m, env).is_ok());
        m.mode = Some("carrier_pigeon".into());
        assert!(validate_transport(&m, env).is_err());
    }
}

// ── Default photo category per stage (0048) ──────────────────────────────────

/// Which category a new photo takes while an order is in each stage. Clients preselect it
/// and offer it next to the categories anyone may file by hand.
#[utoipa::path(
    get, path = "/stage-photo-categories", tag = "configuration",
    responses((status = 200, body = Items<crate::repo::config::StagePhotoCategory>))
)]
async fn stage_photo_categories(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<crate::repo::config::StagePhotoCategory>>> {
    Ok(Items::new(
        crate::repo::config::stage_photo_categories(&state.db).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct PhotoCategoryBody {
    /// Null: no default (production is preselected).
    category: Option<ImageCategory>,
}

#[utoipa::path(
    put, path = "/stage-photo-categories/{key}", tag = "configuration",
    params(("key" = String, Path, description = "An order stage key")),
    request_body = PhotoCategoryBody,
    responses((status = 204, description = "Saved"))
)]
async fn set_stage_photo_category(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(key): ApiPath<String>,
    ApiJson(b): ApiJson<PhotoCategoryBody>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageConfiguration)?;
    // Evidence has its own flows: intake photos and walkaround shots are never filed by
    // default from an ordinary photo button.
    if matches!(
        b.category,
        Some(ImageCategory::Intake) | Some(ImageCategory::Inspection)
    ) {
        return Err(AppError::validation(
            "intake and inspection photos come from their own flows, not a stage default",
        ));
    }
    if !crate::repo::config::set_stage_photo_category(&state.db, &key, b.category).await? {
        return Err(AppError::NotFound("stage"));
    }
    audit::record(
        &state.db,
        Some(me.user_id),
        "stage_definition",
        0,
        "photo_category",
        json!({ "stage": key, "category": b.category }),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
