//! A browsable page for the API (Swagger UI) and the OpenAPI document it reads.
//!
//! `GET /api/docs` is the page, `GET /api/docs/openapi.json` the live contract. In `dev` and
//! `staging` they are open, so a developer can look around without logging in. In
//! `production` they need an admin session: a public list of every endpoint helps nobody but
//! an attacker. Signed in as admin on the CRM, "Try it out" works against the real API with
//! the same session cookie.
//!
//! Swagger UI itself is loaded from a CDN by the browser, so the page needs internet access
//! on the machine reading it (the server does not).

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use super::extract::Auth;
use crate::AppState;
use crate::config::AppEnv;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};

const PAGE: &str = r##"<!doctype html>
<html lang="hu">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>AutoCRM API</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css">
  <style>body { margin: 0; } .topbar { display: none; }</style>
</head>
<body>
  <div id="ui"></div>
  <noscript>Az API-dokumentációhoz JavaScript kell.</noscript>
  <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.ui = SwaggerUIBundle({
      url: "/api/docs/openapi.json",
      dom_id: "#ui",
      deepLinking: true,
      tryItOutEnabled: __TRY_IT_OUT__,
      docExpansion: "list",
      filter: true,
      // The page is same-origin with the API: the session cookie is the login.
      requestInterceptor: function (req) { req.credentials = "include"; return req; }
    });
  </script>
</body>
</html>
"##;

fn spec() -> AppResult<Response> {
    let json =
        super::openapi::document_json().map_err(|e| AppError::internal(format!("openapi: {e}")))?;
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        json,
    )
        .into_response())
}

/// `try_it_out` lets the page send real requests; it is on where nothing important is at stake.
fn page(try_it_out: bool) -> Response {
    let html = PAGE.replace("__TRY_IT_OUT__", if try_it_out { "true" } else { "false" });
    ([(header::CACHE_CONTROL, "no-store")], Html(html)).into_response()
}

/// Open routes: development and staging.
fn open_routes() -> Router<AppState> {
    Router::new()
        .route("/api/docs", get(|| async { page(true) }))
        .route("/api/docs/openapi.json", get(|| async { spec() }))
}

/// Admin-only routes: production.
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/docs",
            get(|Auth(me): Auth| async move {
                me.require(Capability::ManageConfiguration)?;
                AppResult::Ok(page(false))
            }),
        )
        .route(
            "/api/docs/openapi.json",
            get(|Auth(me): Auth| async move {
                me.require(Capability::ManageConfiguration)?;
                spec()
            }),
        )
}

/// The docs routes for this environment.
pub fn routes(env: AppEnv) -> Router<AppState> {
    match env {
        AppEnv::Production => admin_routes(),
        AppEnv::Dev | AppEnv::Staging => open_routes(),
    }
}
