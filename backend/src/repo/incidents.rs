//! The internal incident log (0048): workshop-caused damage, what it cost, how it was
//! settled. Never shown to customers.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Incident {
    pub id: i64,
    pub order_id: i64,
    pub order_number: String,
    pub order_title: String,
    pub inspection_id: Option<i64>,
    /// The kiadás damage it was opened from, when it was.
    pub damage_id: Option<i64>,
    pub title: String,
    pub description: Option<String>,
    pub cost_minor: Option<i64>,
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    pub responsible: Option<String>,
    /// `open` | `resolved`.
    pub status: String,
    pub resolution: Option<String>,
    pub rework_order_id: Option<i64>,
    pub rework_order_number: Option<String>,
    pub created_by: Option<i64>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by_name: Option<String>,
}

pub struct NewIncident<'a> {
    pub order_id: i64,
    pub inspection_id: Option<i64>,
    pub damage_id: Option<i64>,
    pub title: &'a str,
    pub description: Option<&'a str>,
    pub cost_minor: Option<i64>,
    pub currency: &'a str,
    pub responsible: Option<&'a str>,
    pub created_by: i64,
}

pub async fn insert(db: impl PgExecutor<'_>, n: &NewIncident<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO incidents (order_id, inspection_id, damage_id, title, description, cost_minor,
                                currency, responsible, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
        n.order_id,
        n.inspection_id,
        n.damage_id,
        n.title,
        n.description,
        n.cost_minor,
        n.currency,
        n.responsible,
        n.created_by
    )
    .fetch_one(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Incident>> {
    sqlx::query_as!(
        Incident,
        r#"SELECT i.id, i.order_id, o.number AS order_number, o.title AS order_title, i.inspection_id,
                  i.damage_id, i.title, i.description, i.cost_minor, i.currency, i.responsible, i.status,
                  i.resolution, i.rework_order_id, r.number AS "rework_order_number?", i.created_by,
                  cu.display_name AS "created_by_name?", i.created_at, i.resolved_at,
                  ru.display_name AS "resolved_by_name?"
           FROM incidents i
           JOIN orders o ON o.id = i.order_id
           LEFT JOIN orders r ON r.id = i.rework_order_id
           LEFT JOIN users cu ON cu.id = i.created_by
           LEFT JOIN users ru ON ru.id = i.resolved_by
           WHERE i.id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn find_by_damage(db: impl PgExecutor<'_>, damage_id: i64) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!("SELECT id FROM incidents WHERE damage_id = $1", damage_id)
        .fetch_optional(db)
        .await
}

pub async fn list(
    db: impl PgExecutor<'_>,
    status: Option<&str>,
    order_id: Option<i64>,
    inspection_id: Option<i64>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<Incident>> {
    sqlx::query_as!(
        Incident,
        r#"SELECT i.id, i.order_id, o.number AS order_number, o.title AS order_title, i.inspection_id,
                  i.damage_id, i.title, i.description, i.cost_minor, i.currency, i.responsible, i.status,
                  i.resolution, i.rework_order_id, r.number AS "rework_order_number?", i.created_by,
                  cu.display_name AS "created_by_name?", i.created_at, i.resolved_at,
                  ru.display_name AS "resolved_by_name?"
           FROM incidents i
           JOIN orders o ON o.id = i.order_id
           LEFT JOIN orders r ON r.id = i.rework_order_id
           LEFT JOIN users cu ON cu.id = i.created_by
           LEFT JOIN users ru ON ru.id = i.resolved_by
           WHERE ($1::text IS NULL OR i.status = $1)
             AND ($2::bigint IS NULL OR i.order_id = $2 OR i.rework_order_id = $2)
             AND ($3::bigint IS NULL OR i.inspection_id = $3)
           ORDER BY (i.status = 'open') DESC, i.created_at DESC, i.id DESC
           LIMIT $4 OFFSET $5"#,
        status,
        order_id,
        inspection_id,
        limit,
        offset
    )
    .fetch_all(db)
    .await
}

pub struct IncidentUpdate {
    pub title: String,
    pub description: Option<String>,
    pub cost_minor: Option<i64>,
    pub currency: String,
    pub responsible: Option<String>,
    pub resolved: bool,
    pub resolution: Option<String>,
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    u: &IncidentUpdate,
    user_id: i64,
) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE incidents
            SET title = $2, description = $3, cost_minor = $4, currency = $5, responsible = $6,
                status = CASE WHEN $7 THEN 'resolved' ELSE 'open' END,
                resolution = $8,
                resolved_at = CASE WHEN $7 THEN coalesce(resolved_at, now()) ELSE NULL END,
                resolved_by = CASE WHEN $7 THEN coalesce(resolved_by, $9) ELSE NULL END
          WHERE id = $1",
        id,
        u.title,
        u.description,
        u.cost_minor,
        u.currency,
        u.responsible,
        u.resolved,
        u.resolution,
        user_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn set_rework(db: impl PgExecutor<'_>, id: i64, order_id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE incidents SET rework_order_id = $2 WHERE id = $1",
        id,
        order_id
    )
    .execute(db)
    .await?;
    Ok(())
}
