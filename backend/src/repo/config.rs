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

#[derive(Debug, Clone, Serialize)]
pub struct ProjectType {
    pub id: i64,
    pub key: String,
    pub label_hu: String,
    pub position: i32,
    pub is_active: bool,
}

pub async fn project_types(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<ProjectType>> {
    sqlx::query_as!(
        ProjectType,
        "SELECT id, key, label_hu, position, is_active FROM project_types ORDER BY position, id"
    )
    .fetch_all(db)
    .await
}

pub async fn insert_project_type(
    db: impl PgExecutor<'_>,
    key: &str,
    label_hu: &str,
    position: i32,
) -> sqlx::Result<ProjectType> {
    sqlx::query_as!(
        ProjectType,
        "INSERT INTO project_types (key, label_hu, position) VALUES ($1, $2, $3)
         RETURNING id, key, label_hu, position, is_active",
        key,
        label_hu,
        position
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
) -> sqlx::Result<Option<ProjectType>> {
    sqlx::query_as!(
        ProjectType,
        "UPDATE project_types SET label_hu = $2, position = $3, is_active = $4 WHERE id = $1
         RETURNING id, key, label_hu, position, is_active",
        id,
        label_hu,
        position,
        is_active
    )
    .fetch_optional(db)
    .await
}

#[derive(Debug, Clone, Serialize)]
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
}

pub async fn settings(db: impl PgExecutor<'_>) -> sqlx::Result<Settings> {
    sqlx::query_as!(
        Settings,
        "SELECT automatic_email_enabled, max_auto_emails_per_recipient_day, send_window_start, send_window_end,
                send_window_weekdays_only, nudge_interval_days, nudge_escalate_after, stage_change_notifications,
                stalled_alert_recipients, updated_at, updated_by
         FROM settings"
    )
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
}

pub async fn update_settings(
    db: impl PgExecutor<'_>,
    s: &SettingsUpdate,
    user_id: i64,
) -> sqlx::Result<Settings> {
    sqlx::query_as!(
        Settings,
        "UPDATE settings
         SET automatic_email_enabled = $1, max_auto_emails_per_recipient_day = $2, send_window_start = $3,
             send_window_end = $4, send_window_weekdays_only = $5, nudge_interval_days = $6, nudge_escalate_after = $7,
             stage_change_notifications = $8, stalled_alert_recipients = $9, updated_by = $10
         RETURNING automatic_email_enabled, max_auto_emails_per_recipient_day, send_window_start, send_window_end,
                   send_window_weekdays_only, nudge_interval_days, nudge_escalate_after, stage_change_notifications,
                   stalled_alert_recipients, updated_at, updated_by",
        s.automatic_email_enabled,
        s.max_auto_emails_per_recipient_day,
        s.send_window_start,
        s.send_window_end,
        s.send_window_weekdays_only,
        s.nudge_interval_days,
        s.nudge_escalate_after,
        s.stage_change_notifications,
        &s.stalled_alert_recipients,
        user_id
    )
    .fetch_one(db)
    .await
}
