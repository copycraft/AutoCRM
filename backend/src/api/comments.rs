//! Staff comments on orders and leads (0048). A mention notifies the person named, on the
//! web bell and the phone; the comment shows in the record's history too.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::role::{Capability, Role};
use crate::error::{AppError, AppResult};
use crate::repo::comments::{self, Comment, Mentionable};
use crate::repo::{leads, notifications, orders};

const MAX_BODY: usize = 5000;
const MAX_MENTIONS: usize = 20;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update, remove))
        .routes(routes!(mentionable))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// `order` | `lead`.
    entity_type: String,
    entity_id: i64,
}

/// What a comment is about, checked to exist, with the words and link a notification uses.
async fn subject(
    state: &AppState,
    entity_type: &str,
    entity_id: i64,
) -> AppResult<(String, String)> {
    match entity_type {
        "order" => {
            let o = orders::find(&state.db, entity_id)
                .await?
                .ok_or(AppError::NotFound("order"))?;
            Ok((
                format!("#{} · {}", o.number, o.title),
                format!("/orders/{entity_id}?tab=comments"),
            ))
        }
        "lead" => {
            let l = leads::find(&state.db, entity_id)
                .await?
                .ok_or(AppError::NotFound("lead"))?;
            Ok((l.title, format!("/leads/{entity_id}#comments")))
        }
        _ => Err(AppError::validation("entity_type must be order or lead")),
    }
}

#[utoipa::path(
    get, path = "/comments", tag = "comments",
    params(ListQuery),
    responses((status = 200, body = Items<Comment>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<Comment>>> {
    subject(&state, &q.entity_type, q.entity_id).await?;
    Ok(Items::new(
        comments::list(&state.db, &q.entity_type, q.entity_id).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct CommentBody {
    /// `order` | `lead`.
    entity_type: String,
    entity_id: i64,
    body: String,
    /// The users named in the text (`@Név`). Each is notified once.
    #[serde(default)]
    mention_ids: Vec<i64>,
}

fn clean_body(body: &str) -> AppResult<String> {
    let body = super::required("body", body)?;
    if body.chars().count() > MAX_BODY {
        return Err(AppError::validation(format!(
            "body is at most {MAX_BODY} characters"
        )));
    }
    Ok(body)
}

fn clean_mentions(ids: &[i64]) -> AppResult<Vec<i64>> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    if ids.len() > MAX_MENTIONS {
        return Err(AppError::validation(format!(
            "at most {MAX_MENTIONS} people can be mentioned"
        )));
    }
    Ok(ids)
}

fn snippet(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 140 {
        flat
    } else {
        format!("{}…", flat.chars().take(140).collect::<String>().trim_end())
    }
}

/// Notifies everyone newly mentioned, except the author.
async fn notify(
    conn: &mut sqlx::PgConnection,
    newly: &[i64],
    author_id: i64,
    author_name: &str,
    label: &str,
    link: &str,
    body: &str,
) -> AppResult<()> {
    let title = format!("{author_name} megemlítette: {label}");
    for &user_id in newly.iter().filter(|id| **id != author_id) {
        notifications::notify_user(
            &mut *conn,
            user_id,
            "mention",
            &title,
            Some(&snippet(body)),
            Some(link),
        )
        .await?;
    }
    Ok(())
}

#[utoipa::path(
    post, path = "/comments", tag = "comments",
    request_body = CommentBody,
    responses((status = 201, body = Comment))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CommentBody>,
) -> AppResult<(StatusCode, Json<Comment>)> {
    me.require(Capability::Comment)?;
    let (label, link) = subject(&state, &b.entity_type, b.entity_id).await?;
    let body = clean_body(&b.body)?;
    let mention_ids = clean_mentions(&b.mention_ids)?;
    let mut tx = state.db.begin().await?;
    let id = comments::insert(&mut *tx, &b.entity_type, b.entity_id, &body, me.user_id).await?;
    let newly = comments::add_mentions(&mut *tx, id, &mention_ids).await?;
    notify(
        &mut tx,
        &newly,
        me.user_id,
        &me.display_name,
        &label,
        &link,
        &body,
    )
    .await?;
    tx.commit().await?;
    let comment = comments::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("comment"))?;
    Ok((StatusCode::CREATED, Json(comment)))
}

#[derive(Deserialize, ToSchema)]
struct EditBody {
    body: String,
    /// Everyone named in the edited text. Earlier mentions stay; new ones are notified.
    #[serde(default)]
    mention_ids: Vec<i64>,
}

/// Only the author edits a comment, or an admin.
fn may_change(me: &crate::service::auth::AuthUser, comment: &Comment) -> AppResult<()> {
    if comment.created_by == me.user_id || me.role == Role::Admin {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

#[utoipa::path(
    patch, path = "/comments/{id}", tag = "comments",
    params(("id" = i64, Path)),
    request_body = EditBody,
    responses((status = 200, body = Comment))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<EditBody>,
) -> AppResult<Json<Comment>> {
    me.require(Capability::Comment)?;
    let current = comments::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("comment"))?;
    may_change(&me, &current)?;
    let (label, link) = subject(&state, &current.entity_type, current.entity_id).await?;
    let body = clean_body(&b.body)?;
    let mention_ids = clean_mentions(&b.mention_ids)?;
    let mut tx = state.db.begin().await?;
    comments::update_body(&mut *tx, id, &body).await?;
    let newly = comments::add_mentions(&mut *tx, id, &mention_ids).await?;
    notify(
        &mut tx,
        &newly,
        me.user_id,
        &me.display_name,
        &label,
        &link,
        &body,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        comments::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("comment"))?,
    ))
}

#[utoipa::path(
    delete, path = "/comments/{id}", tag = "comments",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted"))
)]
async fn remove(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::Comment)?;
    let current = comments::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("comment"))?;
    may_change(&me, &current)?;
    comments::soft_delete(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Everyone who can be mentioned: the active users, by name. Open to every signed-in
/// user (the user list itself is admin-only).
#[utoipa::path(
    get, path = "/comments/mentionable", tag = "comments",
    responses((status = 200, body = Items<Mentionable>))
)]
async fn mentionable(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<Mentionable>>> {
    Ok(Items::new(comments::mentionable(&state.db).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_are_deduplicated_and_capped() {
        assert_eq!(clean_mentions(&[3, 1, 3]).unwrap(), vec![1, 3]);
        assert!(clean_mentions(&(0..30).collect::<Vec<_>>()).is_err());
    }

    #[test]
    fn a_notification_snippet_is_one_short_line() {
        assert_eq!(snippet("  szia\n\n  nézd meg  "), "szia nézd meg");
        assert!(snippet(&"a ".repeat(200)).ends_with('…'));
    }
}
