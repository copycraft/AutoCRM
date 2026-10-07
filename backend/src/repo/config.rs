//! Configuration tables: stage definitions, project types, settings.

use chrono::{DateTime, NaiveTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

use crate::domain::media::ImageCategory;
use crate::domain::stage::{StageDefinition, StageEntity};

pub async fn stage_definitions(
    db: impl PgExecutor<'_>,
    entity: StageEntity,
) -> sqlx::Result<Vec<StageDefinition>> {
    sqlx::query_as!(
        StageDefinition,
        r#"SELECT id, entity, key, label_hu, position, min_images,
                  required_image_category AS "required_image_category: ImageCategory",
                  is_terminal, is_exit, stall_after_days, is_active
           FROM stage_definitions WHERE entity = $1 ORDER BY position, id"#,
        entity.as_str()
    )
    .fetch_all(db)
    .await
}

pub async fn all_stage_definitions(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<StageDefinition>> {
    sqlx::query_as!(
        StageDefinition,
        r#"SELECT id, entity, key, label_hu, position, min_images,
                  required_image_category AS "required_image_category: ImageCategory",
                  is_terminal, is_exit, stall_after_days, is_active
           FROM stage_definitions ORDER BY entity, position, id"#
    )
    .fetch_all(db)
    .await
}

pub async fn find_stage_definition(
    db: impl PgExecutor<'_>,
    id: i64,
) -> sqlx::Result<Option<StageDefinition>> {
    sqlx::query_as!(
        StageDefinition,
        r#"SELECT id, entity, key, label_hu, position, min_images,
                  required_image_category AS "required_image_category: ImageCategory",
                  is_terminal, is_exit, stall_after_days, is_active
           FROM stage_definitions WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub struct StageDefinitionUpdate {
    pub label_hu: String,
    pub position: i32,
    pub min_images: i32,
    pub required_image_category: Option<ImageCategory>,
    pub stall_after_days: Option<i32>,
    pub is_active: bool,
}

/// Key, entity and the terminal/exit flags are structural and never change after creation.
pub async fn update_stage_definition(
    db: impl PgExecutor<'_>,
    id: i64,
    u: &StageDefinitionUpdate,
) -> sqlx::Result<Option<StageDefinition>> {
    sqlx::query_as!(
        StageDefinition,
        r#"UPDATE stage_definitions
           SET label_hu = $2, position = $3, min_images = $4, required_image_category = $5,
               stall_after_days = $6, is_active = $7
           WHERE id = $1
           RETURNING id, entity, key, label_hu, position, min_images,
                     required_image_category AS "required_image_category: ImageCategory",
                     is_terminal, is_exit, stall_after_days, is_active"#,
        id,
        u.label_hu,
        u.position,
        u.min_images,
        u.required_image_category as Option<ImageCategory>,
        u.stall_after_days,
        u.is_active
    )
    .fetch_optional(db)
    .await
}

pub struct NewStageDefinition {
    pub entity: StageEntity,
    pub key: String,
    pub label_hu: String,
    pub position: i32,
    pub min_images: i32,
    pub required_image_category: Option<ImageCategory>,
    pub is_terminal: bool,
    pub is_exit: bool,
    pub stall_after_days: Option<i32>,
}

pub async fn insert_stage_definition(
    db: impl PgExecutor<'_>,
    n: &NewStageDefinition,
) -> sqlx::Result<StageDefinition> {
    sqlx::query_as!(
        StageDefinition,
        r#"INSERT INTO stage_definitions
               (entity, key, label_hu, position, min_images, required_image_category, is_terminal, is_exit, stall_after_days)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, entity, key, label_hu, position, min_images,
                     required_image_category AS "required_image_category: ImageCategory",
                     is_terminal, is_exit, stall_after_days, is_active"#,
        n.entity.as_str(),
        n.key,
        n.label_hu,
        n.position,
        n.min_images,
        n.required_image_category as Option<ImageCategory>,
        n.is_terminal,
        n.is_exit,
        n.stall_after_days
    )
    .fetch_one(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProjectType {
    pub id: i64,
    pub key: String,
    pub label_hu: String,
    pub position: i32,
    pub is_active: bool,
    /// Which build-spec section the order form shows for this type: `heating`,
    /// `cooling`, or null for work that has no build spec (repairs). Configuration, so
    /// the office can retype a category without a deploy.
    pub spec_form: Option<String>,
}

pub async fn project_types(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<ProjectType>> {
    sqlx::query_as!(
        ProjectType,
        "SELECT id, key, label_hu, position, is_active, spec_form
           FROM project_types ORDER BY position, id"
    )
    .fetch_all(db)
    .await
}

pub async fn insert_project_type(
    db: impl PgExecutor<'_>,
    key: &str,
    label_hu: &str,
    position: i32,
    spec_form: Option<&str>,
) -> sqlx::Result<ProjectType> {
    sqlx::query_as!(
        ProjectType,
        "INSERT INTO project_types (key, label_hu, position, spec_form) VALUES ($1, $2, $3, $4)
         RETURNING id, key, label_hu, position, is_active, spec_form",
        key,
        label_hu,
        position,
        spec_form
    )
    .fetch_one(db)
    .await
}

pub async fn update_project_type(
    db: impl PgExecutor<'_>,
    id: i64,
    label_hu: &str,
    position: i32,
    is_active: bool,
    spec_form: Option<&str>,
) -> sqlx::Result<Option<ProjectType>> {
    sqlx::query_as!(
        ProjectType,
        "UPDATE project_types SET label_hu = $2, position = $3, is_active = $4, spec_form = $5
          WHERE id = $1
         RETURNING id, key, label_hu, position, is_active, spec_form",
        id,
        label_hu,
        position,
        is_active,
        spec_form
    )
    .fetch_optional(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Settings {
    pub automatic_email_enabled: bool,
    pub max_auto_emails_per_recipient_day: i32,
    pub send_window_start: NaiveTime,
    pub send_window_end: NaiveTime,
    pub send_window_weekdays_only: bool,
    pub nudge_interval_days: i32,
    pub nudge_escalate_after: i32,
    pub stage_change_notifications: bool,
    pub stalled_alert_recipients: Vec<String>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<i64>,
    // Email transport overrides. All NULL means "inherit from the environment".
    // The secret itself is never selected here; see email_secret().
    pub email_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_security: Option<String>,
    pub smtp_username: Option<String>,
    pub has_password: bool,
    pub smtp_helo_name: Option<String>,
    pub smtp_force_ipv4: Option<bool>,
    pub redirect_to: Option<String>,
}

pub async fn settings(db: impl PgExecutor<'_>) -> sqlx::Result<Settings> {
    sqlx::query_as!(
        Settings,
        "SELECT automatic_email_enabled, max_auto_emails_per_recipient_day, send_window_start, send_window_end,
                send_window_weekdays_only, nudge_interval_days, nudge_escalate_after, stage_change_notifications,
                stalled_alert_recipients, updated_at, updated_by,
                email_mode, smtp_host, smtp_port, smtp_security, smtp_username,
                (smtp_password IS NOT NULL) AS \"has_password!\",
                smtp_helo_name, smtp_force_ipv4, redirect_to
         FROM settings"
    )
    .fetch_one(db)
    .await
}

/// The stored SMTP password, if any. Internal: never serialized to the API.
pub async fn email_secret(db: impl PgExecutor<'_>) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!("SELECT smtp_password FROM settings")
        .fetch_one(db)
        .await
}

pub struct SettingsUpdate {
    pub automatic_email_enabled: bool,
    pub max_auto_emails_per_recipient_day: i32,
    pub send_window_start: NaiveTime,
    pub send_window_end: NaiveTime,
    pub send_window_weekdays_only: bool,
    pub nudge_interval_days: i32,
    pub nudge_escalate_after: i32,
    pub stage_change_notifications: bool,
    pub stalled_alert_recipients: Vec<String>,
    pub email_mode: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_security: Option<String>,
    pub smtp_username: Option<String>,
    /// None keeps the stored secret, Some(None) clears it, Some(Some) replaces it.
    pub smtp_password: Option<Option<String>>,
    pub smtp_helo_name: Option<String>,
    pub smtp_force_ipv4: Option<bool>,
    pub redirect_to: Option<String>,
}

pub async fn update_settings(
    db: impl PgExecutor<'_>,
    s: &SettingsUpdate,
    user_id: i64,
) -> sqlx::Result<Settings> {
    // Tri-state password in a single statement: keep, or replace (None clears).
    let (keep_password, new_password) = match &s.smtp_password {
        None => (true, None),
        Some(v) => (false, v.clone()),
    };
    sqlx::query_as!(
        Settings,
        "UPDATE settings
         SET automatic_email_enabled = $1, max_auto_emails_per_recipient_day = $2, send_window_start = $3,
             send_window_end = $4, send_window_weekdays_only = $5, nudge_interval_days = $6, nudge_escalate_after = $7,
             stage_change_notifications = $8, stalled_alert_recipients = $9, updated_by = $10,
             email_mode = $11, smtp_host = $12, smtp_port = $13, smtp_security = $14, smtp_username = $15,
             smtp_password = CASE WHEN $19 THEN smtp_password ELSE $20 END,
             smtp_helo_name = $16, smtp_force_ipv4 = $17, redirect_to = $18
         RETURNING automatic_email_enabled, max_auto_emails_per_recipient_day, send_window_start, send_window_end,
                   send_window_weekdays_only, nudge_interval_days, nudge_escalate_after, stage_change_notifications,
                   stalled_alert_recipients, updated_at, updated_by,
                   email_mode, smtp_host, smtp_port, smtp_security, smtp_username,
                   (smtp_password IS NOT NULL) AS \"has_password!\",
                   smtp_helo_name, smtp_force_ipv4, redirect_to",
        s.automatic_email_enabled,
        s.max_auto_emails_per_recipient_day,
        s.send_window_start,
        s.send_window_end,
        s.send_window_weekdays_only,
        s.nudge_interval_days,
        s.nudge_escalate_after,
        s.stage_change_notifications,
        &s.stalled_alert_recipients,
        user_id,
        s.email_mode,
        s.smtp_host,
        s.smtp_port,
        s.smtp_security,
        s.smtp_username,
        s.smtp_helo_name,
        s.smtp_force_ipv4,
        s.redirect_to,
        keep_password,
        new_password
    )
    .fetch_one(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct UserSettings {
    pub user_id: i64,
    pub density: String,
    pub page_size: i32,
    /// Put under hand-written mail when the composer opens (0049). Plain text.
    pub email_signature: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub async fn user_settings(
    db: impl PgExecutor<'_>,
    user_id: i64,
) -> sqlx::Result<Option<UserSettings>> {
    sqlx::query_as!(
        UserSettings,
        "SELECT user_id, density, page_size, email_signature, updated_at FROM user_settings WHERE user_id = $1",
        user_id
    )
    .fetch_optional(db)
    .await
}

pub async fn upsert_user_settings(
    db: impl PgExecutor<'_>,
    user_id: i64,
    density: &str,
    page_size: i32,
    email_signature: Option<&str>,
) -> sqlx::Result<UserSettings> {
    sqlx::query_as!(
        UserSettings,
        "INSERT INTO user_settings (user_id, density, page_size, email_signature) VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id) DO UPDATE SET density = $2, page_size = $3, email_signature = $4
         RETURNING user_id, density, page_size, email_signature, updated_at",
        user_id,
        density,
        page_size,
        email_signature
    )
    .fetch_one(db)
    .await
}

/// Which category a new photo takes while an order is in a stage (0048).
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StagePhotoCategory {
    pub stage_key: String,
    pub label_hu: String,
    pub position: i32,
    pub category: Option<ImageCategory>,
}

pub async fn stage_photo_categories(
    db: impl PgExecutor<'_>,
) -> sqlx::Result<Vec<StagePhotoCategory>> {
    sqlx::query_as!(
        StagePhotoCategory,
        r#"SELECT key AS stage_key, label_hu, position,
                  default_image_category AS "category: ImageCategory"
           FROM stage_definitions WHERE entity = 'order' ORDER BY position, id"#
    )
    .fetch_all(db)
    .await
}

pub async fn photo_category_for_stage(
    db: impl PgExecutor<'_>,
    stage_key: &str,
) -> sqlx::Result<Option<ImageCategory>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT default_image_category AS "category: ImageCategory"
           FROM stage_definitions WHERE entity = 'order' AND key = $1"#,
        stage_key
    )
    .fetch_optional(db)
    .await?
    .flatten())
}

pub async fn set_stage_photo_category(
    db: impl PgExecutor<'_>,
    stage_key: &str,
    category: Option<ImageCategory>,
) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE stage_definitions SET default_image_category = $2 WHERE entity = 'order' AND key = $1",
        stage_key,
        category as Option<ImageCategory>
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// The secret in a user's calendar-feed link, if they made one (0049).
pub async fn calendar_token(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT calendar_token FROM user_settings WHERE user_id = $1",
        user_id
    )
    .fetch_optional(db)
    .await?
    .flatten())
}

/// Sets (Some) or removes (None) the calendar-feed secret.
pub async fn set_calendar_token(
    db: impl PgExecutor<'_>,
    user_id: i64,
    token: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO user_settings (user_id, calendar_token) VALUES ($1, $2)
         ON CONFLICT (user_id) DO UPDATE SET calendar_token = $2",
        user_id,
        token
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Whose calendar a feed link opens: the active user owning the token.
pub async fn calendar_owner(
    db: impl PgExecutor<'_>,
    token: &str,
) -> sqlx::Result<Option<(i64, String)>> {
    let row = sqlx::query!(
        "SELECT u.id, u.display_name FROM user_settings s JOIN users u ON u.id = s.user_id
         WHERE s.calendar_token = $1 AND u.is_active",
        token
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| (r.id, r.display_name)))
}

/// Replaces the stored SMTP password as it is (already sealed by the caller).
pub async fn set_email_secret(db: impl PgExecutor<'_>, sealed: &str) -> sqlx::Result<()> {
    sqlx::query!("UPDATE settings SET smtp_password = $1", sealed)
        .execute(db)
        .await?;
    Ok(())
}

/// Who gets the Monday report (0049).
pub async fn weekly_report_recipients(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!("SELECT weekly_report_recipients FROM settings")
        .fetch_one(db)
        .await
}

pub async fn set_weekly_report_recipients(
    db: impl PgExecutor<'_>,
    recipients: &[String],
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE settings SET weekly_report_recipients = $1",
        recipients
    )
    .execute(db)
    .await?;
    Ok(())
}
