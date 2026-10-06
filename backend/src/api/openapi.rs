//! The OpenAPI document: the API contract the web and mobile clients are generated from.
//! Written to `openapi/openapi.json` by `autocrm openapi`; a test fails if that file is stale.

use utoipa::openapi::extensions::ExtensionsBuilder;
use utoipa::openapi::path::Operation;
use utoipa::openapi::schema::{ObjectBuilder, Schema, Type};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::openapi::{ContentBuilder, Ref, RefOr, ResponseBuilder};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;

use crate::AppState;
use crate::error::ErrorBody;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "AutoCRM API",
        version = "1",
        description = "Autotherm project lifecycle system. Money is integer minor units with an explicit currency; quantities are decimal strings. PATCH: omit a field to keep it, send null to clear it."
    ),
    servers((url = "/api")),
    components(schemas(ErrorBody)),
    security(("session_cookie" = []), ("bearer" = [])),
    tags(
        (name = "auth"), (name = "users"), (name = "configuration"), (name = "partners"),
        (name = "migration"), (name = "leads"), (name = "orders"), (name = "blockers"),
        (name = "media"), (name = "mobile"), (name = "newsletter"), (name = "email"),
        (name = "reports"), (name = "search"), (name = "tasks"), (name = "admin"),
        (name = "vehicles"), (name = "inspections"), (name = "invoices"),
        (name = "incoming_invoices"), (name = "timeline"), (name = "assistant"),
        (name = "followups")
    )
)]
struct ApiDoc;

/// Security schemes, plus the shared error response on every operation (all of them can
/// fail with the standard error body).
struct SecurityAndErrors;

impl Modify for SecurityAndErrors {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session_cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("autocrm_session"))),
        );
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );

        // The shared error-code catalog: every `error.code` value as an enum, with the
        // full table (status, retryable, HU/EN text) as an extension both clients test
        // against. `ErrorDetail.code` stays a plain string so an older client decodes a
        // newer code instead of failing the whole response.
        components.schemas.insert(
            "ErrorCode".to_string(),
            RefOr::T(Schema::Object(
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .description(Some(
                        "Every machine code `error.code` can carry. `x-error-catalog` gives \
                         each one's HTTP status, whether a retry can succeed, and the \
                         user-facing text.",
                    ))
                    .enum_values(Some(crate::error::ERROR_CODES.iter().map(|c| c.code)))
                    .extensions(Some(
                        ExtensionsBuilder::new()
                            .add(
                                "x-error-catalog",
                                serde_json::to_value(crate::error::ERROR_CODES)
                                    .expect("the catalog serializes"),
                            )
                            .build(),
                    ))
                    .build(),
            )),
        );

        let error = |description: &str| {
            RefOr::T(
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name("ErrorBody")))
                            .build(),
                    )
                    .build(),
            )
        };
        for item in openapi.paths.paths.values_mut() {
            let operations: [&mut Option<Operation>; 5] = [
                &mut item.get,
                &mut item.post,
                &mut item.put,
                &mut item.patch,
                &mut item.delete,
            ];
            for operation in operations.into_iter().flatten() {
                // Handler names repeat across modules (`detail`, `search`); generated clients
                // key operations by id, so qualify each with its tag: `orders_detail`.
                if let (Some(tag), Some(id)) = (
                    operation.tags.as_ref().and_then(|t| t.first()).cloned(),
                    operation.operation_id.clone(),
                ) {
                    if !id.starts_with(&format!("{tag}_")) {
                        operation.operation_id = Some(format!("{tag}_{id}"));
                    }
                }
                operation
                    .responses
                    .responses
                    .entry("4XX".to_string())
                    .or_insert_with(|| error("Client error; see `error.code`"));
                operation
                    .responses
                    .responses
                    .entry("5XX".to_string())
                    .or_insert_with(|| error("Server error"));
            }
        }
    }
}

pub fn document() -> utoipa::openapi::OpenApi {
    let (_, mut doc) = OpenApiRouter::<AppState>::with_openapi(ApiDoc::openapi())
        .merge(super::api_routes())
        .split_for_parts();
    SecurityAndErrors.modify(&mut doc);
    doc
}

/// Pretty JSON with a trailing newline, exactly as committed.
pub fn document_json() -> anyhow::Result<String> {
    Ok(format!("{}\n", document().to_pretty_json()?))
}
