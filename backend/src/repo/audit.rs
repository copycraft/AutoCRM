use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgExecutor;

/// Record a change. Call inside the same transaction as the change itself, so the audit
/// trail can never disagree with the data.
pub async fn record(
    db: impl PgExecutor<'_>,
    user_id: Option<i64>,
    entity: &str,
    entity_id: i64,
    action: &str,
    changes: Value,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO audit_log (user_id, entity, entity_id, action, changes) VALUES ($1, $2, $3, $4, $5)",
        user_id,
        entity,
        entity_id,
        action,
        changes
    )
    .execute(db)
    .await?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub at: DateTime<Utc>,
    pub user_id: Option<i64>,
    pub user_name: Option<String>,
    pub entity: String,
    pub entity_id: i64,
    pub action: String,
    pub changes: Value,
}

pub async fn list_for(
    db: impl PgExecutor<'_>,
    entity: &str,
    entity_id: i64,
    limit: i64,
) -> sqlx::Result<Vec<AuditEntry>> {
    sqlx::query_as!(
        AuditEntry,
        r#"SELECT a.id, a.at, a.user_id, u.display_name AS "user_name?", a.entity, a.entity_id, a.action, a.changes
           FROM audit_log a
           LEFT JOIN users u ON u.id = a.user_id
           WHERE a.entity = $1 AND a.entity_id = $2
           ORDER BY a.at DESC, a.id DESC
           LIMIT $3"#,
        entity,
        entity_id,
        limit
    )
    .fetch_all(db)
    .await
}

/// Builds a `{field: [old, new]}` diff containing only fields that changed.
pub fn diff(pairs: &[(&str, Value, Value)]) -> Value {
    let map = pairs
        .iter()
        .filter(|(_, old, new)| old != new)
        .map(|(field, old, new)| {
            (
                field.to_string(),
                Value::Array(vec![old.clone(), new.clone()]),
            )
        })
        .collect();
    Value::Object(map)
}
