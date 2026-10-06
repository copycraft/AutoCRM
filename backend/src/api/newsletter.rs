//! Newsletter: the subscription list, the website signup, and the BCC blast.
//!
//! Authenticated routes (the settings list, the blast itself) need `SendEmail`: a
//! newsletter goes to hundreds of strangers, so it is office work. The public routes
//! are the website signup — behind `X-Newsletter-Key`, off entirely without
//! `NEWSLETTER_API_KEY`, and double opt-in — plus confirming and unsubscribing, which
//! must work from a bare link.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::newsletter::{self, Subscription};
use crate::repo::newsletter_tags;
use crate::service::email::{self, NewsletterRequest};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(subscriptions, add_subscription))
        .routes(routes!(remove_subscription))
        .routes(routes!(send))
        .routes(routes!(subscribe, unsubscribe))
        .routes(routes!(confirm))
}

/// Everyone on the list, unsubscribed included: the office sees who opted out, because a
/// deleted address would silently resubscribe on the next import.
#[utoipa::path(
    get, path = "/newsletter/subscriptions", tag = "newsletter",
    responses((status = 200, body = Items<Subscription>))
)]
async fn subscriptions(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<Subscription>>> {
    Ok(Items::new(newsletter::list(&state.db).await?))
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct SubscriptionBody {
    pub email: String,
    #[serde(default)]
    pub name: String,
}

/// The office hand-add: confirmed on insert, because the office holds the consent.
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct AddSubscriptionBody {
    pub email: String,
    #[serde(default)]
    pub name: String,
    /// Newsletter tags to file the subscriber under.
    #[serde(default)]
    pub tag_ids: Vec<i64>,
}

#[utoipa::path(
    post, path = "/newsletter/subscriptions", tag = "newsletter",
    request_body = AddSubscriptionBody,
    responses((status = 201, body = Subscription))
)]
async fn add_subscription(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<AddSubscriptionBody>,
) -> AppResult<(StatusCode, Json<Subscription>)> {
    me.require(Capability::SendEmail)?;
    let email = normalize_address(&body.email)
        .ok_or_else(|| AppError::validation("email address is invalid"))?;
    // An opted-out address stays opted out until they come back themselves: the office
    // hand-add must not silently resubscribe someone, or the tombstone that guards the
    // next import is pointless (MAIL-L6).
    refuse_resubscribe(&newsletter::list(&state.db).await?, &email)?;
    let mut tag_ids = body.tag_ids.clone();
    tag_ids.sort_unstable();
    tag_ids.dedup();
    if !tag_ids.is_empty()
        && newsletter_tags::count_live(&state.db, &tag_ids).await? != tag_ids.len() as i64
    {
        return Err(AppError::validation("a tag does not exist or is archived"));
    }
    let mut tx = state.db.begin().await?;
    let sub = newsletter::subscribe(&mut *tx, &email, &body.name, "office").await?;
    newsletter_tags::add(&mut *tx, &[sub.id], &tag_ids).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(sub)))
}

/// Whether the office may hand-add this address. Pure, so the rule is unit-tested:
/// website resubscribes ("coming back is saying yes again") keep working through the
/// public endpoint, which does not call this.
fn refuse_resubscribe(existing: &[newsletter::Subscription], email: &str) -> AppResult<()> {
    if existing
        .iter()
        .any(|s| s.unsubscribed_at.is_some() && s.email == email)
    {
        return Err(AppError::conflict(
            "duplicate",
            "this address previously unsubscribed from the newsletter; only they can resubscribe via the website form",
        ));
    }
    Ok(())
}

#[utoipa::path(
    delete, path = "/newsletter/subscriptions/{id}", tag = "newsletter",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Removed"))
)]
async fn remove_subscription(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::SendEmail)?;
    if !newsletter::remove(&state.db, id).await? {
        return Err(AppError::NotFound("subscription"));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// One row, everyone in BCC. The answer says how many addresses made the list, so the
/// office knows what "sent" meant.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct NewsletterSent {
    pub email_id: i64,
    pub recipients: usize,
}

#[utoipa::path(
    post, path = "/newsletter/send", tag = "newsletter",
    request_body = NewsletterRequest,
    responses((status = 202, body = NewsletterSent))
)]
async fn send(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<NewsletterRequest>,
) -> AppResult<(StatusCode, Json<NewsletterSent>)> {
    me.require(Capability::SendEmail)?;
    let (email_id, recipients) = email::send_newsletter(&state, &me, &body).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(NewsletterSent {
            email_id,
            recipients,
        }),
    ))
}

/// Website signup: `POST` with the address and `X-Newsletter-Key`.
///
/// Double opt-in: this records a pending signup and mails a confirmation link; the address
/// joins the list only when its owner clicks it. The answer is the same 202 whatever the
/// address's history (new, pending, active, opted out), so the endpoint says nothing
/// about who is on the list.
#[utoipa::path(
    post, path = "/newsletter/subscribe", tag = "newsletter",
    request_body = SubscriptionBody,
    responses(
        (status = 202, description = "Accepted; a confirmation link is mailed unless the address is already subscribed"),
        (status = 403, description = "Missing or wrong key, or signup is off"),
    )
)]
async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<SubscriptionBody>,
) -> AppResult<StatusCode> {
    check_website_key(&state, &headers)?;
    normalize_address(&body.email)
        .ok_or_else(|| AppError::validation("email address is invalid"))?;
    email::newsletter_signup(&state, &body.email, &body.name).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, serde::Deserialize, IntoParams)]
pub struct ConfirmQuery {
    /// The token from the confirmation letter.
    pub token: String,
}

#[derive(Debug, serde::Serialize, ToSchema)]
pub struct Confirmed {
    /// False for an unknown, already used or expired link.
    pub confirmed: bool,
}

/// The confirmation click. Public: the reader arrives from the letter, not logged in.
#[utoipa::path(
    get, path = "/newsletter/confirm", tag = "newsletter",
    params(ConfirmQuery),
    responses((status = 200, body = Confirmed))
)]
async fn confirm(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ConfirmQuery>,
) -> AppResult<Json<Confirmed>> {
    let token = q.token.trim();
    let confirmed = !token.is_empty() && newsletter::confirm(&state.db, token).await?.is_some();
    Ok(Json(Confirmed { confirmed }))
}

#[derive(Debug, serde::Deserialize, IntoParams)]
pub struct UnsubscribeQuery {
    /// The per-address token from an earlier mail. The page without a token asks for the
    /// address instead — a BCC blast cannot carry personal links.
    pub token: Option<String>,
    pub email: Option<String>,
}

/// Unsubscribe from a link or a typed address. Always answers 200.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct Unsubscribed {
    /// For a token link: whether this click changed anything (a link clicked twice is
    /// not an error). For a typed address: always true — "if it was on the list, it is
    /// off now" — so typing addresses cannot reveal who is subscribed.
    pub unsubscribed: bool,
}

#[utoipa::path(
    get, path = "/newsletter/unsubscribe", tag = "newsletter",
    params(UnsubscribeQuery),
    responses((status = 200, body = Unsubscribed))
)]
async fn unsubscribe(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<UnsubscribeQuery>,
) -> AppResult<Json<Unsubscribed>> {
    if let Some(token) = q.token.filter(|t| !t.trim().is_empty()) {
        let changed = newsletter::unsubscribe_by_token(&state.db, token.trim())
            .await?
            .is_some();
        return Ok(Json(Unsubscribed {
            unsubscribed: changed,
        }));
    }
    if let Some(email) = q.email.filter(|e| !e.trim().is_empty()) {
        if let Some(normalized) = normalize_address(&email) {
            sqlx::query!(
                "UPDATE newsletter_subscriptions SET unsubscribed_at = now()
                 WHERE lower(email) = $1 AND unsubscribed_at IS NULL",
                normalized
            )
            .execute(&state.db)
            .await?;
        }
        return Ok(Json(Unsubscribed { unsubscribed: true }));
    }
    Ok(Json(Unsubscribed {
        unsubscribed: false,
    }))
}

fn check_website_key(state: &AppState, headers: &HeaderMap) -> AppResult<()> {
    super::check_api_key(
        state.config.newsletter_api_key.as_deref(),
        headers,
        "x-newsletter-key",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn sub(email: &str, unsubscribed: bool) -> newsletter::Subscription {
        newsletter::Subscription {
            id: 1,
            email: email.into(),
            name: "".into(),
            source: "website".into(),
            subscribed_at: Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap(),
            confirmed_at: Some(Utc.with_ymd_and_hms(2026, 1, 1, 9, 5, 0).unwrap()),
            unsubscribed_at: unsubscribed
                .then(|| Utc.with_ymd_and_hms(2026, 2, 1, 9, 0, 0).unwrap()),
        }
    }

    #[test]
    fn the_office_cannot_resubscribe_an_opted_out_address() {
        let existing = vec![sub("a@example.hu", false), sub("x@example.hu", true)];
        // Active and unknown addresses are fine.
        assert!(refuse_resubscribe(&existing, "a@example.hu").is_ok());
        assert!(refuse_resubscribe(&existing, "new@example.hu").is_ok());
        // An opted-out address is refused, with a message that names the way back.
        let err = refuse_resubscribe(&existing, "x@example.hu").unwrap_err();
        let message = format!("{err:?}");
        assert!(
            message.contains("previously unsubscribed") && message.contains("website"),
            "got {message}",
        );
    }
}
