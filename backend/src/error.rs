//! One error type for the HTTP boundary. Every error renders as
//! `{"error": {"code": "...", "message": "..."}}`; internal details are logged, never returned.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::domain::stage::TransitionError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("you do not have permission to do this")]
    Forbidden,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Validation(String),
    /// A business rule said no (stage gate, immutable record, ...). HTTP 422.
    #[error("{message}")]
    Rule { code: &'static str, message: String },
    /// The request conflicts with current state. HTTP 409.
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("too many attempts, try again later")]
    TooManyRequests,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn validation(message: impl Into<String>) -> Self {
        AppError::Validation(message.into())
    }

    pub fn rule(code: &'static str, message: impl Into<String>) -> Self {
        AppError::Rule {
            code,
            message: message.into(),
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        AppError::Conflict {
            code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Display) -> Self {
        AppError::Internal(anyhow::anyhow!("{message}"))
    }
}

use std::fmt::Display;

impl From<TransitionError> for AppError {
    fn from(e: TransitionError) -> Self {
        let code = match e {
            TransitionError::GateNotMet { .. } => "stage_gate",
            TransitionError::NoteRequired => "note_required",
            _ => "invalid_transition",
        };
        AppError::rule(code, e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message): (StatusCode, &str, String) = match &self {
            AppError::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                self.to_string(),
            ),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", self.to_string()),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, "validation", self.to_string()),
            AppError::Rule { code, message } => {
                (StatusCode::UNPROCESSABLE_ENTITY, code, message.clone())
            }
            AppError::Conflict { code, message } => (StatusCode::CONFLICT, code, message.clone()),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                self.to_string(),
            ),
            AppError::Database(sqlx::Error::RowNotFound) => (
                StatusCode::NOT_FOUND,
                "not_found",
                "record not found".into(),
            ),
            AppError::Database(sqlx::Error::Database(db)) => match db.code().as_deref() {
                Some("23505") => (
                    StatusCode::CONFLICT,
                    "duplicate",
                    format!(
                        "a record with these values already exists ({})",
                        db.constraint().unwrap_or("unique")
                    ),
                ),
                Some("23503") => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_reference",
                    format!(
                        "a referenced record does not exist or is still in use ({})",
                        db.constraint().unwrap_or("foreign key")
                    ),
                ),
                Some("23514") => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "constraint_violation",
                    format!(
                        "value violates a data rule ({})",
                        db.constraint().unwrap_or("check")
                    ),
                ),
                Some("AC001") => (
                    StatusCode::CONFLICT,
                    "immutable",
                    "this record is write-once evidence and cannot be changed or deleted".into(),
                ),
                _ => {
                    tracing::error!(error = %db, "database error");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal",
                        "internal error".into(),
                    )
                }
            },
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error".into(),
                )
            }
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error".into(),
                )
            }
        };
        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}
