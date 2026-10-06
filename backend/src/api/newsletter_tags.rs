//! Marketing: newsletter tags, the paged subscriber list, tagging (one by one or in bulk),
//! pasting in a list of addresses, and how many a blast to some tags would reach.
//!
//! Reading needs a login; changing anything needs `SendEmail`, like the newsletter itself.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::lead_tag::normalize_color;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::{audit, like_pattern};
use crate::repo::newsletter_tags::{
    self, ImportOutcome, NewsletterTag, SubscriberCounts, SubscriberFilter, SubscriberRow,
    SubscriberStatus, TagInput,
};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_tags, create_tag))
        .routes(routes!(update_tag))
        .routes(routes!(reorder_tags))
        .routes(routes!(search_subscribers))
        .routes(routes!(subscriber_counts))
        .routes(routes!(set_subscription_tags))
        .routes(routes!(bulk_tags))
        .routes(routes!(import))
        .routes(routes!(audience))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct TagsQuery {
    /// The archived tags instead of the live ones.
    #[serde(default)]
    archived: bool,
}

#[utoipa::path(
    get, path = "/newsletter/tags", tag = "newsletter",
    params(TagsQuery),
    responses((status = 200, body = Items<NewsletterTag>))
)]
async fn list_tags(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<TagsQuery>,
) -> AppResult<Json<Items<NewsletterTag>>> {
    Ok(Items::new(newsletter_tags::list(&state.db, q.archived).await?))
}

#[derive(Deserialize, ToSchema)]
struct CreateNewsletterTag {
    /// The heading to list it under; a new heading starts a new section at the end.
    section: String,
    label: String,
    /// `#rrggbb`; light blue when omitted.
    color: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct UpdateNewsletterTag {
    section: Option<String>,
    label: Option<String>,
    color: Option<String>,
    /// true archives: subscribers keep it, but it leaves the lists and cannot be picked.
    archived: Option<bool>,
}

fn tag_input(section: &str, label: &str, color: &str) -> AppResult<TagInput> {
    let section = required("section", section)?;
    let label = required("label", label)?;
    if section.chars().count() > 60 || label.chars().count() > 100 {
        return Err(AppError::validation("section or label is too long"));
    }
    let color = normalize_color(color)
        .ok_or_else(|| AppError::validation("color must look like #a33122"))?;
    Ok(TagInput {
        section,
        label,
        color,
    })
}

#[utoipa::path(
    post, path = "/newsletter/tags", tag = "newsletter",
    request_body = CreateNewsletterTag,
    responses((status = 201, body = NewsletterTag), (status = 409, description = "The label is taken in that section"))
)]
async fn create_tag(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateNewsletterTag>,
) -> AppResult<(StatusCode, Json<NewsletterTag>)> {
    me.require(Capability::SendEmail)?;
    let input = tag_input(&b.section, &b.label, b.color.as_deref().unwrap_or("#dbe8ff"))?;
    let id = newsletter_tags::insert(&state.db, &input).await?;
    audit::record(
        &state.db,
        Some(me.user_id),
        "newsletter_tag",
        id,
        "create",
        json!({ "section": input.section, "label": input.label }),
    )
    .await?;
    let tag = newsletter_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("newsletter tag"))?;
    Ok((StatusCode::CREATED, Json(tag)))
}

#[utoipa::path(
    patch, path = "/newsletter/tags/{id}", tag = "newsletter",
    params(("id" = i64, Path)),
    request_body = UpdateNewsletterTag,
    responses((status = 200, body = NewsletterTag), (status = 409, description = "The label is taken in that section"))
)]
async fn update_tag(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<UpdateNewsletterTag>,
) -> AppResult<Json<NewsletterTag>> {
    me.require(Capability::SendEmail)?;
    let current = newsletter_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("newsletter tag"))?;
    let input = tag_input(
        b.section.as_deref().unwrap_or(&current.section),
        b.label.as_deref().unwrap_or(&current.label),
        b.color.as_deref().unwrap_or(&current.color),
    )?;
    let archived = b.archived.unwrap_or(current.archived_at.is_some());
    newsletter_tags::update(&state.db, id, &input, archived).await?;
    let changes = audit::diff(&[
        ("section", json!(current.section), json!(input.section)),
        ("label", json!(current.label), json!(input.label)),
        ("color", json!(current.color), json!(input.color)),
        (
            "archived",
            json!(current.archived_at.is_some()),
            json!(archived),
        ),
    ]);
    audit::record(&state.db, Some(me.user_id), "newsletter_tag", id, "update", changes).await?;
    let tag = newsletter_tags::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("newsletter tag"))?;
    Ok(Json(tag))
}

#[derive(Deserialize, ToSchema)]
struct ReorderNewsletterTags {
    section: String,
    /// The section's tags, top first.
    ids: Vec<i64>,
}

#[utoipa::path(
    put, path = "/newsletter/tags/order", tag = "newsletter",
    request_body = ReorderNewsletterTags,
    responses((status = 204, description = "Reordered"))
)]
async fn reorder_tags(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ReorderNewsletterTags>,
) -> AppResult<StatusCode> {
    me.require(Capability::SendEmail)?;
    newsletter_tags::reorder(&state.db, b.section.trim(), &b.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct SubscribersQuery {
    /// Part of the address or the name.
    q: Option<String>,
    /// Only subscribers with this tag.
    tag: Option<i64>,
    /// Only subscribers with no tag at all.
    #[serde(default)]
    untagged: bool,
    #[param(inline)]
    status: Option<SubscriberStatus>,
    limit: Option<i64>,
    offset: Option<i64>,
}

/// The subscriber list, a page at a time, newest first, each row with its tag ids.
#[utoipa::path(
    get, path = "/newsletter/subscribers", tag = "newsletter",
    params(SubscribersQuery),
    responses((status = 200, body = Items<SubscriberRow>))
)]
async fn search_subscribers(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<SubscribersQuery>,
) -> AppResult<Json<Items<SubscriberRow>>> {
    let pattern = optional(q.q).and_then(|s| like_pattern(&s));
    let rows = newsletter_tags::search(
        &state.db,
        &SubscriberFilter {
            pattern: pattern.as_deref(),
            tag_id: q.tag,
            untagged: q.untagged,
            status: q.status,
        },
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    Ok(Items::new(rows))
}

#[utoipa::path(
    get, path = "/newsletter/subscribers/counts", tag = "newsletter",
    responses((status = 200, body = SubscriberCounts))
)]
async fn subscriber_counts(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<SubscriberCounts>> {
    Ok(Json(newsletter_tags::counts(&state.db).await?))
}

async fn check_live(state: &AppState, tag_ids: &[i64]) -> AppResult<Vec<i64>> {
    let mut ids = tag_ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if !ids.is_empty() && newsletter_tags::count_live(&state.db, &ids).await? != ids.len() as i64 {
        return Err(AppError::validation("a tag does not exist or is archived"));
    }
    Ok(ids)
}

#[derive(Deserialize, ToSchema)]
struct SubscriptionTagsBody {
    /// The subscriber's tags after the change; every other tag is removed.
    tag_ids: Vec<i64>,
}

#[utoipa::path(
    put, path = "/newsletter/subscriptions/{id}/tags", tag = "newsletter",
    params(("id" = i64, Path)),
    request_body = SubscriptionTagsBody,
    responses((status = 200, body = Vec<i64>, description = "The subscriber's tag ids"))
)]
async fn set_subscription_tags(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<SubscriptionTagsBody>,
) -> AppResult<Json<Vec<i64>>> {
    me.require(Capability::SendEmail)?;
    let mut tx = state.db.begin().await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM newsletter_subscriptions WHERE id = $1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if !exists {
        return Err(AppError::NotFound("subscription"));
    }
    // An archived tag the subscriber already carries may stay; only new ones must be live.
    let before = newsletter_tags::tags_of(&mut *tx, id).await?;
    let mut ids = b.tag_ids.clone();
    ids.sort_unstable();
    ids.dedup();
    let new: Vec<i64> = ids.iter().copied().filter(|t| !before.contains(t)).collect();
    check_live(&state, &new).await?;
    newsletter_tags::remove_others(&mut *tx, id, &ids).await?;
    newsletter_tags::add(&mut *tx, &[id], &new).await?;
    let after = newsletter_tags::tags_of(&mut *tx, id).await?;
    tx.commit().await?;
    Ok(Json(after))
}

#[derive(Deserialize, ToSchema)]
struct BulkTagsBody {
    subscription_ids: Vec<i64>,
    #[serde(default)]
    add: Vec<i64>,
    #[serde(default)]
    remove: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
struct BulkTagsResult {
    added: u64,
    removed: u64,
}

/// Tag or untag many subscribers at once (the ticked rows of the list).
#[utoipa::path(
    post, path = "/newsletter/subscriptions/tags", tag = "newsletter",
    request_body = BulkTagsBody,
    responses((status = 200, body = BulkTagsResult))
)]
async fn bulk_tags(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<BulkTagsBody>,
) -> AppResult<Json<BulkTagsResult>> {
    me.require(Capability::SendEmail)?;
    if b.subscription_ids.len() > 5000 {
        return Err(AppError::validation("at most 5000 subscribers at once"));
    }
    let add = check_live(&state, &b.add).await?;
    let mut tx = state.db.begin().await?;
    let removed = newsletter_tags::remove(&mut *tx, &b.subscription_ids, &b.remove).await?;
    let added = newsletter_tags::add(&mut *tx, &b.subscription_ids, &add).await?;
    tx.commit().await?;
    Ok(Json(BulkTagsResult { added, removed }))
}

#[derive(Deserialize, ToSchema)]
struct ImportBody {
    /// One subscriber per line: `email`, or `email;name` / `email,name` / tab-separated,
    /// as pasted from a spreadsheet. A header line or blank lines are skipped.
    text: String,
    /// Tags to put on every imported address, new or already on the list.
    #[serde(default)]
    tag_ids: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
struct ImportResult {
    /// New addresses, subscribed and confirmed.
    added: usize,
    /// Already on the list: kept as they were, tags added.
    existing: usize,
    /// Unsubscribed earlier: left out, only they can come back.
    opted_out: usize,
    /// Lines that held no valid address, as written (at most 50).
    invalid: Vec<String>,
}

const MAX_IMPORT_LINES: usize = 20_000;

/// `email[sep name]` from one pasted line, or None when it holds no address.
fn parse_line(line: &str) -> Option<(String, String)> {
    let mut parts = line.splitn(2, ['\t', ';', ',']);
    let email = normalize_address(parts.next()?.trim().trim_matches('"'))?;
    let name = parts
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('"')
        .trim()
        .to_string();
    Some((email, name))
}

/// Paste a list of addresses. New ones join as confirmed subscribers: importing says the
/// office holds their consent, as a hand-add does. Opted-out addresses are skipped.
#[utoipa::path(
    post, path = "/newsletter/import", tag = "newsletter",
    request_body = ImportBody,
    responses((status = 200, body = ImportResult))
)]
async fn import(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ImportBody>,
) -> AppResult<Json<ImportResult>> {
    me.require(Capability::SendEmail)?;
    let tag_ids = check_live(&state, &b.tag_ids).await?;
    let lines: Vec<&str> = b.text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.len() > MAX_IMPORT_LINES {
        return Err(AppError::validation(format!(
            "at most {MAX_IMPORT_LINES} lines at once"
        )));
    }
    let mut result = ImportResult {
        added: 0,
        existing: 0,
        opted_out: 0,
        invalid: Vec::new(),
    };
    let mut ids = Vec::new();
    let mut tx = state.db.begin().await?;
    for (i, line) in lines.iter().enumerate() {
        let Some((email, name)) = parse_line(line) else {
            // A first line without an address is a spreadsheet header, not a mistake.
            if i > 0 && result.invalid.len() < 50 {
                result.invalid.push(line.chars().take(120).collect());
            }
            continue;
        };
        match newsletter_tags::import_one(&mut *tx, &email, &name).await? {
            ImportOutcome::Added(id) => {
                result.added += 1;
                ids.push(id);
            }
            ImportOutcome::Existing(id) => {
                result.existing += 1;
                ids.push(id);
            }
            ImportOutcome::OptedOut => result.opted_out += 1,
        }
    }
    newsletter_tags::add(&mut *tx, &ids, &tag_ids).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "newsletter",
        0,
        "import",
        json!({ "added": result.added, "existing": result.existing,
                "opted_out": result.opted_out, "tag_ids": tag_ids }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(result))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct AudienceQuery {
    /// Comma-separated tag ids; none means everyone.
    tags: Option<String>,
}

#[derive(Serialize, ToSchema)]
struct Audience {
    /// Active, not suppressed, deduplicated: exactly who a blast would go to now.
    recipients: usize,
}

#[utoipa::path(
    get, path = "/newsletter/audience", tag = "newsletter",
    params(AudienceQuery),
    responses((status = 200, body = Audience))
)]
async fn audience(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<AudienceQuery>,
) -> AppResult<Json<Audience>> {
    let mut ids = Vec::new();
    for part in q.tags.as_deref().unwrap_or("").split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        ids.push(
            part.parse::<i64>()
                .map_err(|_| AppError::validation("tags must be comma-separated ids"))?,
        );
    }
    let recipients = newsletter_tags::audience(&state.db, &ids).await?.len();
    Ok(Json(Audience { recipients }))
}

#[cfg(test)]
mod tests {
    use super::parse_line;

    #[test]
    fn pasted_lines_give_an_address_and_a_name() {
        assert_eq!(
            parse_line("Info@Pekseg.HU;Kovács Pékség"),
            Some(("info@pekseg.hu".into(), "Kovács Pékség".into()))
        );
        assert_eq!(
            parse_line("\"a@b.hu\",\"Név, Kft.\""),
            Some(("a@b.hu".into(), "Név, Kft.".into()))
        );
        assert_eq!(parse_line("a@b.hu\tNév"), Some(("a@b.hu".into(), "Név".into())));
        assert_eq!(parse_line("a@b.hu"), Some(("a@b.hu".into(), String::new())));
        assert_eq!(parse_line("E-mail;Név"), None);
        assert_eq!(parse_line("nem cím"), None);
    }
}
