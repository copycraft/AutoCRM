//! Lead tags and which leads carry them. Runtime-checked queries, like attribution.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// A tag as the settings list shows it, with how many leads carry it.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LeadTag {
    pub id: i64,
    /// Two-letter market code: hu, ro, de, it...
    pub market: String,
    pub label: String,
    /// `#rrggbb`.
    pub color: String,
    /// Website domains whose leads get this tag on arrival.
    pub domains: Vec<String>,
    pub position: i32,
    pub archived_at: Option<DateTime<Utc>>,
    /// Leads with this tag that are not in a terminal stage.
    pub open_leads: i64,
    pub total_leads: i64,
}

/// A tag on one lead.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct LeadTagRef {
    pub id: i64,
    pub market: String,
    pub label: String,
    pub color: String,
    /// The domain the server recognised when it tagged the lead itself; null when a
    /// person added the tag.
    pub matched_domain: Option<String>,
}

pub struct TagInput {
    pub market: String,
    pub label: String,
    pub color: String,
    pub domains: Vec<String>,
}

const SELECT: &str = "SELECT t.id, t.market, t.label, t.color, t.domains, t.position, t.archived_at,
        count(k.lead_id) FILTER (WHERE NOT sd.is_terminal) AS open_leads,
        count(k.lead_id) AS total_leads
    FROM lead_tags t
    LEFT JOIN lead_tag_links k ON k.tag_id = t.id
    LEFT JOIN lead_current_stage cs ON cs.lead_id = k.lead_id
    LEFT JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key";

pub async fn list(db: impl PgExecutor<'_>, archived: bool) -> sqlx::Result<Vec<LeadTag>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE (t.archived_at IS NULL) <> $1
         GROUP BY t.id ORDER BY t.market, t.position, t.id"
    ))
    .bind(archived)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<LeadTag>> {
    sqlx::query_as(&format!("{SELECT} WHERE t.id = $1 GROUP BY t.id"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Appended at the end of its market's list.
pub async fn insert(db: impl PgExecutor<'_>, t: &TagInput) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO lead_tags (market, label, color, domains, position)
         VALUES ($1, $2, $3, $4,
                 (SELECT coalesce(max(position), 0) + 10 FROM lead_tags WHERE market = $1))
         RETURNING id",
    )
    .bind(&t.market)
    .bind(&t.label)
    .bind(&t.color)
    .bind(&t.domains)
    .fetch_one(db)
    .await
}

/// Moving to another market puts the tag at the end of that list.
pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    t: &TagInput,
    archived: bool,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE lead_tags SET
             position = CASE WHEN market = $2 THEN position ELSE
                 (SELECT coalesce(max(position), 0) + 10 FROM lead_tags WHERE market = $2) END,
             market = $2, label = $3, color = $4, domains = $5,
             archived_at = CASE WHEN $6 THEN coalesce(archived_at, now()) END
         WHERE id = $1",
    )
    .bind(id)
    .bind(&t.market)
    .bind(&t.label)
    .bind(&t.color)
    .bind(&t.domains)
    .bind(archived)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Renumbers one market's tags in the given order. Ids from another market are ignored.
pub async fn reorder(db: impl PgExecutor<'_>, market: &str, ids: &[i64]) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE lead_tags t SET position = o.n * 10
         FROM unnest($2::bigint[]) WITH ORDINALITY AS o(id, n)
         WHERE t.id = o.id AND t.market = $1",
    )
    .bind(market)
    .bind(ids)
    .execute(db)
    .await?;
    Ok(())
}

/// Another live tag already claiming one of `domains`, as (tag label, domain).
pub async fn domain_owner(
    db: impl PgExecutor<'_>,
    domains: &[String],
    except: Option<i64>,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "SELECT t.market || ' / ' || t.label, d
         FROM lead_tags t, unnest(t.domains) d
         WHERE t.archived_at IS NULL AND d = ANY($1) AND ($2::bigint IS NULL OR t.id <> $2)
         LIMIT 1",
    )
    .bind(domains)
    .bind(except)
    .fetch_optional(db)
    .await
}

/// Every live tag that claims a domain, as (id, domains), for matching an arriving lead.
pub async fn with_domains(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<(i64, Vec<String>)>> {
    sqlx::query_as(
        "SELECT id, domains FROM lead_tags
         WHERE archived_at IS NULL AND cardinality(domains) > 0
         ORDER BY market, position, id",
    )
    .fetch_all(db)
    .await
}

pub async fn for_lead(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Vec<LeadTagRef>> {
    sqlx::query_as(
        "SELECT t.id, t.market, t.label, t.color, k.matched_domain
         FROM lead_tag_links k JOIN lead_tags t ON t.id = k.tag_id
         WHERE k.lead_id = $1
         ORDER BY t.market, t.position, t.id",
    )
    .bind(lead_id)
    .fetch_all(db)
    .await
}

/// The tags of many leads at once, as (lead id, tag), for a list page.
pub async fn for_leads(
    db: impl PgExecutor<'_>,
    lead_ids: &[i64],
) -> sqlx::Result<Vec<(i64, LeadTagRef)>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        lead_id: i64,
        #[sqlx(flatten)]
        tag: LeadTagRef,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT k.lead_id, t.id, t.market, t.label, t.color, k.matched_domain
         FROM lead_tag_links k JOIN lead_tags t ON t.id = k.tag_id
         WHERE k.lead_id = ANY($1)
         ORDER BY t.market, t.position, t.id",
    )
    .bind(lead_ids)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.lead_id, r.tag)).collect())
}

/// Adds a tag; a tag the lead already has is left as it was.
pub async fn link(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    tag_id: i64,
    matched_domain: Option<&str>,
    added_by: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO lead_tag_links (lead_id, tag_id, matched_domain, added_by)
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(lead_id)
    .bind(tag_id)
    .bind(matched_domain)
    .bind(added_by)
    .execute(db)
    .await?;
    Ok(())
}

/// Removes every tag of the lead not in `keep`.
pub async fn unlink_others(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    keep: &[i64],
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM lead_tag_links WHERE lead_id = $1 AND NOT tag_id = ANY($2)")
        .bind(lead_id)
        .bind(keep)
        .execute(db)
        .await?;
    Ok(())
}

/// Detaches one tag from the lead.
pub async fn remove_tag(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    tag_id: i64,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM lead_tag_links WHERE lead_id = $1 AND tag_id = $2")
        .bind(lead_id)
        .bind(tag_id)
        .execute(db)
        .await?;
    Ok(())
}

/// How many of `ids` are live tags.
pub async fn count_live(db: impl PgExecutor<'_>, ids: &[i64]) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM lead_tags WHERE id = ANY($1) AND archived_at IS NULL")
        .bind(ids)
        .fetch_one(db)
        .await
}

/// One website's funnel in a period: of its leads, how many got a quote, were won, or
/// became orders. Only leads the server tagged (matched_domain set) are attributed.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SiteFunnel {
    pub site: String,
    pub leads: i64,
    pub quoted: i64,
    pub won: i64,
    pub orders: i64,
    pub lost: i64,
}

pub async fn by_site(
    db: impl PgExecutor<'_>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> sqlx::Result<Vec<SiteFunnel>> {
    sqlx::query_as(
        "SELECT k.matched_domain AS site,
                count(DISTINCT l.id) AS leads,
                count(DISTINCT l.id) FILTER (WHERE l.quoted_value_minor IS NOT NULL) AS quoted,
                count(DISTINCT l.id) FILTER (WHERE cs.stage_key = 'won') AS won,
                count(DISTINCT o.id) AS orders,
                count(DISTINCT l.id) FILTER (WHERE cs.stage_key = 'lost') AS lost
           FROM lead_tag_links k
           JOIN leads l ON l.id = k.lead_id
           JOIN lead_current_stage cs ON cs.lead_id = l.id
           LEFT JOIN orders o ON o.lead_id = l.id
          WHERE l.created_at >= $1 AND l.created_at < $2 AND k.matched_domain IS NOT NULL
          GROUP BY k.matched_domain
          ORDER BY leads DESC, k.matched_domain",
    )
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}
