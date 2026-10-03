//! The API docs page: open in development, admin-only in production.

mod common;

use autocrm::api;
use autocrm::config::AppEnv;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;

fn state(pool: PgPool, env: AppEnv) -> autocrm::AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.env = env;
    state.config = Arc::new(config);
    state
}

async fn token(pool: &PgPool, role: Role) -> String {
    let user = common::user(pool, role).await;
    let token = auth::generate_token();
    sessions::insert(
        pool,
        NewSession {
            user_id: user.user_id,
            token_hash: &auth::token_hash(&token),
            kind: SessionKind::Mobile,
            device_label: None,
            user_agent: None,
            ip: None,
            expires_at: chrono::Utc::now() + chrono::TimeDelta::days(1),
        },
    )
    .await
    .unwrap();
    token
}

async fn get(
    state: autocrm::AppState,
    uri: &str,
    bearer: Option<&str>,
) -> (StatusCode, String, String) {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(t) = bearer {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    let response = api::router(state)
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        content_type,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn in_development_the_docs_are_open_and_can_send_requests(pool: PgPool) {
    let (status, content_type, page) =
        get(state(pool.clone(), AppEnv::Dev), "/api/docs", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("text/html"));
    assert!(page.contains("swagger-ui") && page.contains("/api/docs/openapi.json"));
    assert!(page.contains("tryItOutEnabled: true"));

    let (status, content_type, spec) = get(
        state(pool.clone(), AppEnv::Staging),
        "/api/docs/openapi.json",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type.starts_with("application/json"));
    let doc: serde_json::Value = serde_json::from_str(&spec).unwrap();
    assert_eq!(doc["info"]["title"], "AutoCRM API");
    assert_eq!(doc["servers"][0]["url"], "/api");
    assert!(
        doc["paths"]["/hr/employees"].is_object(),
        "the live contract, newest routes included"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn in_production_only_an_admin_sees_them_and_they_are_read_only(pool: PgPool) {
    let production = || state(pool.clone(), AppEnv::Production);
    let admin = token(&pool, Role::Admin).await;
    let office = token(&pool, Role::Office).await;

    for uri in ["/api/docs", "/api/docs/openapi.json"] {
        assert_eq!(
            get(production(), uri, None).await.0,
            StatusCode::UNAUTHORIZED,
            "{uri} anonymous"
        );
        assert_eq!(
            get(production(), uri, Some(&office)).await.0,
            StatusCode::FORBIDDEN,
            "{uri} office"
        );
        assert_eq!(
            get(production(), uri, Some(&admin)).await.0,
            StatusCode::OK,
            "{uri} admin"
        );
    }
    let (_, _, page) = get(production(), "/api/docs", Some(&admin)).await;
    assert!(
        page.contains("tryItOutEnabled: false"),
        "no live requests from the production page"
    );
}
