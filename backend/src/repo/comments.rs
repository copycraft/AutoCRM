//! Staff comments on orders and leads, with mentions (0048).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Mention {
    pub user_id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Comment {
    pub id: i64,
    /// `order` | `lead`.
    pub entity_type: String,
    pub entity_id: i64,
    pub body: String,
    pub created_by: i64,
    pub author_name: String,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    /// The people named in it, who were notified.
    pub mentions: Vec<Mention>,
}

struct Row {
    id: i64,
    entity_type: String,
    entity_id: i64,
    body: String,
    created_by: i64,
    author_name: String,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    mention_ids: Vec<i64>,
    mention_names: Vec<String>,
}

impl From<Row> for Comment {
    fn from(r: Row) -> Self {
        Comment {
            id: r.id,
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            body: r.body,
            created_by: r.created_by,
            author_name: r.author_name,
            created_at: r.created_at,
            edited_at: r.edited_at,
            mentions: r
                .mention_ids
                .into_iter()
                .zip(r.mention_names)
                .map(|(user_id, name)| Mention { user_id, name })
                .collect(),
        }
    }
}

pub async fn list(
    db: impl PgExecutor<'_>,
    entity_type: &str,
    entity_id: i64,
) -> sqlx::Result<Vec<Comment>> {
    let rows = sqlx::query_as!(
        Row,
        r#"SELECT c.id, c.entity_type, c.entity_id, c.body, c.created_by, u.display_name AS author_name,
                  c.created_at, c.edited_at,
                  coalesce(array_agg(m.user_id ORDER BY mu.display_name) FILTER (WHERE m.user_id IS NOT NULL), '{}') AS "mention_ids!",
                  coalesce(array_agg(mu.display_name ORDER BY mu.display_name) FILTER (WHERE m.user_id IS NOT NULL), '{}') AS "mention_names!"
           FROM comments c
           JOIN users u ON u.id = c.created_by
           LEFT JOIN comment_mentions m ON m.comment_id = c.id
           LEFT JOIN users mu ON mu.id = m.user_id
           WHERE c.entity_type = $1 AND c.entity_id = $2 AND c.deleted_at IS NULL
           GROUP BY c.id, u.display_name
           ORDER BY c.created_at, c.id"#,
        entity_type,
        entity_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Comment::from).collect())
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Comment>> {
    let row = sqlx::query_as!(
        Row,
        r#"SELECT c.id, c.entity_type, c.entity_id, c.body, c.created_by, u.display_name AS author_name,
                  c.created_at, c.edited_at,
                  coalesce(array_agg(m.user_id ORDER BY mu.display_name) FILTER (WHERE m.user_id IS NOT NULL), '{}') AS "mention_ids!",
                  coalesce(array_agg(mu.display_name ORDER BY mu.display_name) FILTER (WHERE m.user_id IS NOT NULL), '{}') AS "mention_names!"
           FROM comments c
           JOIN users u ON u.id = c.created_by
           LEFT JOIN comment_mentions m ON m.comment_id = c.id
           LEFT JOIN users mu ON mu.id = m.user_id
           WHERE c.id = $1 AND c.deleted_at IS NULL
           GROUP BY c.id, u.display_name"#,
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(Comment::from))
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    entity_type: &str,
    entity_id: i64,
    body: &str,
    user_id: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO comments (entity_type, entity_id, body, created_by) VALUES ($1, $2, $3, $4) RETURNING id",
        entity_type,
        entity_id,
        body,
        user_id
    )
    .fetch_one(db)
    .await
}

pub async fn update_body(db: impl PgExecutor<'_>, id: i64, body: &str) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE comments SET body = $2, edited_at = now() WHERE id = $1 AND deleted_at IS NULL",
        id,
        body
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn soft_delete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE comments SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
        id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Records the mentions not recorded yet; returns the users newly mentioned.
pub async fn add_mentions(
    db: impl PgExecutor<'_>,
    comment_id: i64,
    user_ids: &[i64],
) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar!(
        r#"INSERT INTO comment_mentions (comment_id, user_id)
           SELECT $1, u.id FROM users u WHERE u.id = ANY($2) AND u.is_active
           ON CONFLICT DO NOTHING
           RETURNING user_id"#,
        comment_id,
        user_ids
    )
    .fetch_all(db)
    .await
}

/// Who can be mentioned: every active user, by name.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Mentionable {
    pub id: i64,
    pub display_name: String,
}

pub async fn mentionable(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Mentionable>> {
    sqlx::query_as!(
        Mentionable,
        "SELECT id, display_name FROM users WHERE is_active ORDER BY display_name"
    )
    .fetch_all(db)
    .await
}

pub async fn count(
    db: impl PgExecutor<'_>,
    entity_type: &str,
    entity_id: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM comments WHERE entity_type = $1 AND entity_id = $2 AND deleted_at IS NULL"#,
        entity_type,
        entity_id
    )
    .fetch_one(db)
    .await
}
