//! One error type for the HTTP boundary. Every error renders as
//! `{"error": {"code": "...", "message": "..."}}`; internal details are logged, never returned.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::stage::TransitionError;

/// The body of every non-2xx response.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    /// Stable machine-readable code, e.g. `validation`, `stage_gate`, `currency_locked`.
    pub code: String,
    /// Human-readable detail. For `validation` it names the field and the reason.
    pub message: String,
}

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

/// One entry of the shared error-code catalog.
#[derive(Debug, Serialize)]
pub struct ErrorCode {
    pub code: &'static str,
    /// The HTTP status this code is sent with.
    pub status: u16,
    /// Whether repeating the same request unchanged can succeed (a transient condition).
    pub retryable: bool,
    /// User-facing Hungarian text; both clients render this rather than `message`.
    pub hu: &'static str,
    pub en: &'static str,
}

const fn code(
    code: &'static str,
    status: u16,
    retryable: bool,
    hu: &'static str,
    en: &'static str,
) -> ErrorCode {
    ErrorCode {
        code,
        status,
        retryable,
        hu,
        en,
    }
}

/// Every `error.code` the API can send. Published in the OpenAPI document as the
/// `ErrorCode` schema (with this table under `x-error-catalog`), so the web and Android
/// clients are tested against the same list. `tests/error_codes.rs` fails when a
/// `rule("…")`/`conflict("…")` code in the source is missing here, or an entry is unused.
pub const ERROR_CODES: &[ErrorCode] = &[
    // Generic, emitted by `IntoResponse` below.
    code(
        "unauthenticated",
        401,
        false,
        "Nincs bejelentkezve. Jelentkezzen be újra.",
        "Authentication required.",
    ),
    code(
        "forbidden",
        403,
        false,
        "Nincs jogosultsága ehhez a művelethez.",
        "You do not have permission to do this.",
    ),
    code(
        "not_found",
        404,
        false,
        "A kért adat nem található.",
        "Not found.",
    ),
    code(
        "validation",
        400,
        false,
        "Érvényesítési hiba. Ellenőrizze a megadott adatokat.",
        "Validation failed; the message names the field.",
    ),
    code(
        "too_many_requests",
        429,
        true,
        "Túl sok próbálkozás. Próbálja újra később.",
        "Too many attempts; try again later.",
    ),
    code(
        "duplicate",
        409,
        false,
        "Ilyen adat már létezik.",
        "A record with these values already exists.",
    ),
    code(
        "invalid_reference",
        422,
        false,
        "Érvénytelen hivatkozás, vagy a hivatkozott adat még használatban van.",
        "A referenced record does not exist or is still in use.",
    ),
    code(
        "constraint_violation",
        422,
        false,
        "Az adat sérti az adatbázis szabályait.",
        "A value violates a data rule.",
    ),
    code(
        "immutable",
        409,
        false,
        "Ez bizonyítási célból védett adat, nem módosítható és nem törölhető.",
        "Write-once evidence; cannot be changed or deleted.",
    ),
    code(
        "internal",
        500,
        true,
        "Szerverhiba. Próbálja újra később.",
        "Internal error.",
    ),
    // Auth.
    code(
        "password_change_required",
        422,
        false,
        "Első bejelentkezéskor kötelező jelszót változtatni.",
        "The password must be changed before anything else.",
    ),
    code(
        "wrong_password",
        422,
        false,
        "Hibás a jelenlegi jelszó.",
        "The current password is incorrect.",
    ),
    code(
        "last_admin",
        422,
        false,
        "Az utolsó aktív adminisztrátor nem tiltható le.",
        "The last active admin cannot be deactivated.",
    ),
    // Stages and leads.
    code(
        "stage_gate",
        422,
        false,
        "A megrendelés nem léphet tovább, mert egy szükséges feltétel még nem teljesült.",
        "A stage gate is not met.",
    ),
    code(
        "note_required",
        422,
        false,
        "A visszalépéshez indoklás szükséges.",
        "Moving backwards requires a note.",
    ),
    code(
        "invalid_transition",
        422,
        false,
        "Ez a fázisváltás nem engedélyezett.",
        "This stage transition is not allowed.",
    ),
    code(
        "intake_slip_missing",
        422,
        false,
        "Az átvétel lezárásához előbb rögzítsd az átvételi lapot (km-óra).",
        "Record the intake slip (odometer) first.",
    ),
    code(
        "use_conversion",
        422,
        false,
        "A megnyert fázishoz használja az átalakítás funkciót.",
        "Use lead conversion to reach the won stage.",
    ),
    code(
        "lead_converted",
        422,
        false,
        "Ez a lead már megrendeléssé lett alakítva.",
        "The lead has already been converted.",
    ),
    code(
        "stage_required",
        422,
        false,
        "A megnyert lead-fázist az átalakítás használja, nem kapcsolható ki.",
        "The won lead stage is required by conversion.",
    ),
    // Orders and blockers.
    code(
        "currency_locked",
        422,
        false,
        "A pénznem nem módosítható, amíg a megrendelésnek vannak tételei.",
        "Currency cannot change while the order has items.",
    ),
    code(
        "already_resolved",
        409,
        false,
        "Ez az akadály már meg van oldva.",
        "The blocker is already resolved.",
    ),
    code(
        "not_resolved",
        409,
        false,
        "Ez az akadály nincs megoldva, ezért nem nyitható újra.",
        "The blocker is not resolved.",
    ),
    // Media.
    code(
        "upload_missing",
        422,
        false,
        "A feltöltés nem található vagy lejárt. Kezdje újra.",
        "The upload was not found or has expired.",
    ),
    code(
        "upload_mismatch",
        422,
        false,
        "A feltöltött fájl nem egyezik a bejelentettel.",
        "The uploaded file does not match what was announced.",
    ),
    code(
        "invalid_ticket",
        422,
        false,
        "Érvénytelen vagy lejárt feltöltési jegy.",
        "Invalid or expired upload ticket.",
    ),
    // Inspections.
    code(
        "already_attached",
        422,
        false,
        "Ez a fotó már csatolva van.",
        "The photo is already attached.",
    ),
    code(
        "checkout_open",
        422,
        false,
        "Már van nyitott kiadási jegyzőkönyv. Előbb írassa alá vagy dobja el.",
        "An open check-out already exists for this order.",
    ),
    code(
        "checkout_required",
        422,
        false,
        "A visszavételhez előbb aláírt kiadási jegyzőkönyv kell.",
        "Check-in needs a signed check-out first.",
    ),
    code(
        "locked",
        422,
        false,
        "Az aláírt jegyzőkönyv már nem módosítható; utólagos megjegyzést lehet hozzáfűzni.",
        "The signed inspection is locked.",
    ),
    code(
        "signatures_required",
        422,
        false,
        "A lezáráshoz mindkét aláírás szükséges.",
        "Both signatures are required.",
    ),
    code(
        "verdicts_pending",
        422,
        false,
        "Előbb minden új sérülésnél dönteni kell.",
        "Every new damage needs a verdict first.",
    ),
    // Email and jobs.
    code(
        "not_cancellable",
        409,
        false,
        "Ez az e-mail már nem vonható vissza.",
        "The email can no longer be cancelled.",
    ),
    code(
        "not_retryable",
        409,
        false,
        "Ez a művelet nem próbálható újra.",
        "The email cannot be retried.",
    ),
    code(
        "not_failed",
        409,
        false,
        "Csak véglegesen elakadt feladat indítható újra.",
        "Only a failed job can be retried.",
    ),
    // Invoicing.
    code(
        "invoicing_not_configured",
        422,
        false,
        "A számlázás nincs beállítva ezen a szerveren. Szóljon a rendszergazdának.",
        "Invoicing is not configured.",
    ),
    code(
        "invoice_exists",
        409,
        false,
        "Ehhez a megrendeléshez már tartozik élő számla. Előbb sztornózza.",
        "A live invoice already exists.",
    ),
    code(
        "invoice_in_flight",
        409,
        true,
        "A számla bejelentése még folyamatban van a NAV felé. Várja meg a választ.",
        "The invoice is still being reported.",
    ),
    code(
        "invoice_data_missing",
        422,
        false,
        "A partnernél hiányzik egy számlázási adat. Pótolja, majd próbálja újra.",
        "Invoice data is missing.",
    ),
    code(
        "fx_rate_missing",
        422,
        true,
        "Nincs árfolyam a teljesítés napjára. Próbálja újra az árfolyam letöltése után.",
        "No exchange rate for the fulfilment date.",
    ),
    code(
        "not_stornoable",
        422,
        false,
        "Csak kiállított számla sztornózható.",
        "Only an issued invoice can be cancelled.",
    ),
    code(
        "not_annullable",
        422,
        false,
        "Csak bejelentett számla érvényteleníthető technikailag.",
        "Only a reported invoice can be annulled.",
    ),
    code(
        "no_items",
        422,
        false,
        "A számlához legalább egy tétel kell a megrendelésen.",
        "The order has no items to invoice.",
    ),
    code(
        "nav_rejected",
        422,
        false,
        "A NAV elutasította a számlát. A részleteket a számla adatlapja mutatja.",
        "The tax authority rejected the invoice.",
    ),
    code(
        "nav_unreachable",
        422,
        true,
        "A számlázó szolgáltatás nem érhető el. A megrendelés nem sérült; próbálja újra később.",
        "The invoicing service is unreachable.",
    ),
    code(
        "not_issued",
        422,
        false,
        "Csak kiállított számlának van letölthető bizonylata.",
        "Only an issued invoice has a document to fetch.",
    ),
    code(
        "pdf_unavailable",
        422,
        true,
        "A PDF most nem tölthető le. Próbálja újra később.",
        "The PDF cannot be fetched right now.",
    ),
];

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
        let body = ErrorBody {
            error: ErrorDetail {
                code: code.to_string(),
                message,
            },
        };
        (status, Json(body)).into_response()
    }
}
