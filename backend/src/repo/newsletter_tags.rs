//! Newsletter tags, the subscriber list as the Marketing page pages through it, and the
//! audience of a blast aimed at tags. Runtime-checked queries, like lead tags.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use super::newsletter::Subscription;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct NewsletterTag {
    pub id: i64,
    /// The heading it is listed under: Listák, Értékesítés, Vevők, Nem vevők...
    pub section: String,
    pub label: String,
    /// `#rrggbb`.
    pub color: String,
    pub position: i32,
    pub archived_at: Option<DateTime<Utc>>,
    /// Confirmed, not unsubscribed: who a blast to this tag would reach (before the
    /// suppression list).
    pub active_subscribers: i64,
    pub total_subscribers: i64,
}

pub struct TagInput {
    pub section: String,
    pub label: String,
    pub color: String,
}

const SELECT: &str = "SELECT t.id, t.section, t.label, t.color, t.position, t.archived_at,
        count(s.id) FILTER (WHERE s.confirmed_at IS NOT NULL AND s.unsubscribed_at IS NULL)
            AS active_subscribers,
        count(s.id) AS total_subscribers
    FROM newsletter_tags t
    LEFT JOIN newsletter_subscription_tags k ON k.tag_id = t.id
    LEFT JOIN newsletter_subscriptions s ON s.id = k.subscription_id";

pub async fn list(db: impl PgExecutor<'_>, archived: bool) -> sqlx::Result<Vec<NewsletterTag>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE (t.archived_at IS NULL) <> $1 GROUP BY t.id ORDER BY t.position, t.id"
    ))
    .bind(archived)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<NewsletterTag>> {
    sqlx::query_as(&format!("{SELECT} WHERE t.id = $1 GROUP BY t.id"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Appended at the end of its section, or at the very end for a new section.
pub async fn insert(db: impl PgExecutor<'_>, t: &TagInput) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO newsletter_tags (section, label, color, position)
         VALUES ($1, $2, $3, coalesce(
             (SELECT max(position) + 1 FROM newsletter_tags WHERE lower(section) = lower($1)),
             (SELECT coalesce(max(position), 0) + 1000 FROM newsletter_tags)))
         RETURNING id",
    )
    .bind(&t.section)
    .bind(&t.label)
    .bind(&t.color)
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    t: &TagInput,
    archived: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE newsletter_tags SET
             position = CASE WHEN lower(section) = lower($2) THEN position ELSE coalesce(
                 (SELECT max(position) + 1 FROM newsletter_tags WHERE lower(section) = lower($2)),
                 (SELECT coalesce(max(position), 0) + 1000 FROM newsletter_tags)) END,
             section = $2, label = $3, color = $4,
             archived_at = CASE WHEN $5 THEN coalesce(archived_at, now()) END
         WHERE id = $1",
    )
    .bind(id)
    .bind(&t.section)
    .bind(&t.label)
    .bind(&t.color)
    .bind(archived)
    .execute(db)
    .await?;
    Ok(())
}

/// Puts one section's tags in the given order, keeping the section where it was.
pub async fn reorder(db: impl PgExecutor<'_>, section: &str, ids: &[i64]) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE newsletter_tags t
         SET position = base.p + o.n::int - 1
         FROM unnest($2::bigint[]) WITH ORDINALITY AS o(id, n),
              (SELECT min(position) AS p FROM newsletter_tags WHERE lower(section) = lower($1)) base
         WHERE t.id = o.id AND lower(t.section) = lower($1)",
    )
    .bind(section)
    .bind(ids)
    .execute(db)
    .await?;
    Ok(())
}

/// A subscriber with the ids of their tags.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SubscriberRow {
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub subscription: Subscription,
    pub tag_ids: Vec<i64>,
    /// Two letters; None: unknown, the main letter (0049).
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubscriberStatus {
    /// Confirmed and not unsubscribed: receives blasts.
    Active,
    /// A website signup waiting for its confirmation click.
    Pending,
    Unsubscribed,
}

pub struct SubscriberFilter<'a> {
    /// An ILIKE pattern over email and name.
    pub pattern: Option<&'a str>,
    pub tag_id: Option<i64>,
    /// Only subscribers with no tag at all.
    pub untagged: bool,
    pub status: Option<SubscriberStatus>,
}

pub async fn search(
    db: impl PgExecutor<'_>,
    f: &SubscriberFilter<'_>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<SubscriberRow>> {
    let status = f.status.map(|s| match s {
        SubscriberStatus::Active => "active",
        SubscriberStatus::Pending => "pending",
        SubscriberStatus::Unsubscribed => "unsubscribed",
    });
    sqlx::query_as(
        "SELECT s.id, s.email, s.name, s.source, s.subscribed_at, s.confirmed_at, s.unsubscribed_at,
                coalesce(array_agg(k.tag_id ORDER BY k.tag_id) FILTER (WHERE k.tag_id IS NOT NULL),
                         '{}') AS tag_ids,
                s.language
         FROM newsletter_subscriptions s
         LEFT JOIN newsletter_subscription_tags k ON k.subscription_id = s.id
         WHERE ($1::text IS NULL OR s.email ILIKE $1 OR s.name ILIKE $1)
           AND ($2::bigint IS NULL OR EXISTS (SELECT 1 FROM newsletter_subscription_tags x
                                              WHERE x.subscription_id = s.id AND x.tag_id = $2))
           AND (NOT $3 OR NOT EXISTS (SELECT 1 FROM newsletter_subscription_tags x
                                      WHERE x.subscription_id = s.id))
           AND ($4::text IS NULL
                OR ($4 = 'active' AND s.confirmed_at IS NOT NULL AND s.unsubscribed_at IS NULL)
                OR ($4 = 'pending' AND s.confirmed_at IS NULL AND s.unsubscribed_at IS NULL)
                OR ($4 = 'unsubscribed' AND s.unsubscribed_at IS NOT NULL))
         GROUP BY s.id
         ORDER BY s.subscribed_at DESC, s.id DESC
         LIMIT $5 OFFSET $6",
    )
    .bind(f.pattern)
    .bind(f.tag_id)
    .bind(f.untagged)
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
}

/// Counts for the list's header: everyone, the active ones, and those with no tag.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SubscriberCounts {
    pub total: i64,
    pub active: i64,
    pub pending: i64,
    pub unsubscribed: i64,
    pub untagged: i64,
}

pub async fn counts(db: impl PgExecutor<'_>) -> sqlx::Result<SubscriberCounts> {
    sqlx::query_as(
        "SELECT count(*) AS total,
                count(*) FILTER (WHERE confirmed_at IS NOT NULL AND unsubscribed_at IS NULL) AS active,
                count(*) FILTER (WHERE confirmed_at IS NULL AND unsubscribed_at IS NULL) AS pending,
                count(*) FILTER (WHERE unsubscribed_at IS NOT NULL) AS unsubscribed,
                count(*) FILTER (WHERE NOT EXISTS (SELECT 1 FROM newsletter_subscription_tags k
                                                   WHERE k.subscription_id = s.id)) AS untagged
         FROM newsletter_subscriptions s",
    )
    .fetch_one(db)
    .await
}

/// How many of `ids` are live tags.
pub async fn count_live(db: impl PgExecutor<'_>, ids: &[i64]) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM newsletter_tags WHERE id = ANY($1) AND archived_at IS NULL",
    )
    .bind(ids)
    .fetch_one(db)
    .await
}

pub async fn tags_of(db: impl PgExecutor<'_>, subscription_id: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar(
        "SELECT tag_id FROM newsletter_subscription_tags WHERE subscription_id = $1 ORDER BY tag_id",
    )
    .bind(subscription_id)
    .fetch_all(db)
    .await
}

/// Adds `tag_ids` to every subscription in `subscription_ids`; existing pairs stay.
/// Returns how many pairs were new.
pub async fn add(
    db: impl PgExecutor<'_>,
    subscription_ids: &[i64],
    tag_ids: &[i64],
) -> sqlx::Result<u64> {
    let done = sqlx::query(
        "INSERT INTO newsletter_subscription_tags (subscription_id, tag_id)
         SELECT s, t FROM unnest($1::bigint[]) s, unnest($2::bigint[]) t
         WHERE EXISTS (SELECT 1 FROM newsletter_subscriptions WHERE id = s)
         ON CONFLICT DO NOTHING",
    )
    .bind(subscription_ids)
    .bind(tag_ids)
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

pub async fn remove(
    db: impl PgExecutor<'_>,
    subscription_ids: &[i64],
    tag_ids: &[i64],
) -> sqlx::Result<u64> {
    let done = sqlx::query(
        "DELETE FROM newsletter_subscription_tags
         WHERE subscription_id = ANY($1) AND tag_id = ANY($2)",
    )
    .bind(subscription_ids)
    .bind(tag_ids)
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

/// Removes every tag of the subscription not in `keep`.
pub async fn remove_others(
    db: impl PgExecutor<'_>,
    subscription_id: i64,
    keep: &[i64],
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM newsletter_subscription_tags
         WHERE subscription_id = $1 AND NOT tag_id = ANY($2)",
    )
    .bind(subscription_id)
    .bind(keep)
    .execute(db)
    .await?;
    Ok(())
}

/// Active addresses a blast would go to, lowercased, deduplicated and with the
/// suppression list already taken out. Empty `tag_ids` is everyone; otherwise a
/// subscriber with any of the tags.
pub async fn audience(db: impl PgExecutor<'_>, tag_ids: &[i64]) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT lower(s.email) FROM newsletter_subscriptions s
         WHERE s.confirmed_at IS NOT NULL AND s.unsubscribed_at IS NULL
           AND (cardinality($1::bigint[]) = 0 OR EXISTS (
                SELECT 1 FROM newsletter_subscription_tags k
                WHERE k.subscription_id = s.id AND k.tag_id = ANY($1)))
           AND NOT EXISTS (SELECT 1 FROM email_suppressions x WHERE x.email = lower(s.email))
         ORDER BY 1",
    )
    .bind(tag_ids)
    .fetch_all(db)
    .await
}

/// What happened to one imported address.
pub enum ImportOutcome {
    Added(i64),
    /// Already on the list and not opted out: kept as it was, tags added.
    Existing(i64),
    /// Unsubscribed earlier: left alone, as the office hand-add is.
    OptedOut,
}

/// One line of an import: a new address is subscribed and confirmed (the office holds
/// the consent, as with a hand-add); a known one keeps its status and name.
pub async fn import_one(
    db: impl PgExecutor<'_>,
    email: &str,
    name: &str,
) -> sqlx::Result<ImportOutcome> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        inserted: bool,
        unsubscribed: bool,
    }
    let row: Row = sqlx::query_as(
        "WITH ins AS (
             INSERT INTO newsletter_subscriptions (email, name, source, confirmed_at)
             VALUES ($1, $2, 'import', now())
             ON CONFLICT (email) DO NOTHING
             RETURNING id)
         SELECT id, true AS inserted, false AS unsubscribed FROM ins
         UNION ALL
         SELECT id, false, unsubscribed_at IS NOT NULL FROM newsletter_subscriptions
         WHERE email = $1 AND NOT EXISTS (SELECT 1 FROM ins)",
    )
    .bind(email)
    .bind(name)
    .fetch_one(db)
    .await?;
    Ok(if row.inserted {
        ImportOutcome::Added(row.id)
    } else if row.unsubscribed {
        ImportOutcome::OptedOut
    } else {
        ImportOutcome::Existing(row.id)
    })
}
