//! The CRM assistant (service::assistant): questions about the data, answered by a local
//! model through read-only tools, and list filters it builds for the user.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, Auth};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::service::assistant::{self, AssistantReply, ChatMessage};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(chat))
}

#[derive(Serialize, ToSchema)]
struct AssistantStatus {
    /// False when no model is configured (AI_URL unset): the UI hides the assistant.
    enabled: bool,
    model: Option<String>,
}

#[utoipa::path(
    get, path = "/assistant", tag = "assistant",
    responses((status = 200, body = AssistantStatus))
)]
async fn status(State(state): State<AppState>, Auth(_): Auth) -> Json<AssistantStatus> {
    Json(AssistantStatus {
        enabled: state.config.ai.is_some(),
        model: state.config.ai.as_ref().map(|a| a.model.clone()),
    })
}

#[derive(Deserialize, ToSchema)]
struct ChatBody {
    /// The conversation so far, oldest first; the last one is the user's question.
    messages: Vec<ChatMessage>,
}

/// One turn of the conversation. Read-only: the assistant looks things up and builds
/// filters, it never changes data.
#[utoipa::path(
    post, path = "/assistant/chat", tag = "assistant",
    request_body = ChatBody,
    responses(
        (status = 200, body = AssistantReply),
        (status = 422, description = "No model configured, or the model is not reachable"),
    )
)]
async fn chat(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiJson(b): ApiJson<ChatBody>,
) -> AppResult<Json<AssistantReply>> {
    let ai = state.config.ai.as_ref().ok_or_else(|| {
        AppError::rule("assistant_unavailable", "no assistant model is configured")
    })?;
    match b.messages.last() {
        Some(m) if m.role == "user" && !m.content.trim().is_empty() => {}
        _ => {
            return Err(AppError::validation(
                "the last message must be the user's question",
            ));
        }
    }
    if b.messages.len() > 50 {
        return Err(AppError::validation("at most 50 messages"));
    }
    let today = crate::service::business_today(state.config.business_tz).to_string();
    Ok(Json(
        assistant::chat(&state.db, ai, &b.messages, &today).await?,
    ))
}
