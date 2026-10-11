//! `GET /api/mobile/app-version` + `GET /api/mobile/app.apk`: the Android self-update.
//!
//! Both are public: a phone with an expired session and a stale build must still
//! learn that it is stale. Unset APP_APK_PATH and both answer 404, quietly.

mod common;

use std::sync::Arc;

use autocrm::AppState;
use autocrm::api;
use autocrm::config::AppUpdateConfig;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

fn state_with_apk(pool: PgPool, apk: Option<std::path::PathBuf>) -> AppState {
    let mut state = common::state(pool);
    let mut config = (*state.config).clone();
    config.app_update = apk.map(|apk_path| AppUpdateConfig {
        apk_path,
        version_code: 14,
        version_name: "1.4.0".into(),
        changelog_hu: "Gyorsabb szkennelés.".into(),
        mandatory: false,
    });
    state.config = Arc::new(config);
    state
}

fn fake_apk() -> std::path::PathBuf {
    // An APK is a ZIP; the server never opens it, it only hashes and serves the
    // bytes, so fixed filler bytes exercise the whole path.
    let path = std::env::temp_dir().join(format!("autocrm-test-{}.apk", common::rand_suffix()));
    std::fs::write(&path, b"PK fake-apk-bytes-for-the-update-test").unwrap();
    path
}

async fn get(state: AppState, uri: &str) -> (StatusCode, Vec<u8>) {
    let response = api::router(state)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, body)
}

#[sqlx::test(migrations = "./migrations")]
async fn version_and_apk_serve_the_configured_release(pool: PgPool) {
    let apk = fake_apk();
    let state = state_with_apk(pool, Some(apk.clone()));

    let (status, body) = get(state.clone(), "/api/mobile/app-version").await;
    assert_eq!(status, StatusCode::OK);
    let version: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(version["version_code"], 14);
    assert_eq!(version["version_name"], "1.4.0");
    assert_eq!(version["apk_url"], "/api/mobile/app.apk");
    assert_eq!(version["mandatory"], false);
    assert_eq!(version["changelog_hu"], "Gyorsabb szkennelés.");
    assert_eq!(
        version["size_bytes"],
        std::fs::metadata(&apk).unwrap().len() as u64
    );
    let expected_sha = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(std::fs::read(&apk).unwrap()))
    };
    assert_eq!(version["sha256"], expected_sha);

    let (status, body) = get(state, "/api/mobile/app.apk").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, std::fs::read(&apk).unwrap());

    std::fs::remove_file(&apk).ok();
}

#[sqlx::test(migrations = "./migrations")]
async fn without_a_release_both_endpoints_are_quietly_absent(pool: PgPool) {
    let state = state_with_apk(pool, None);
    let (status, _) = get(state.clone(), "/api/mobile/app-version").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(state, "/api/mobile/app.apk").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
