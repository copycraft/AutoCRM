//! The HR module: the staff directory, open only to admins and to users an admin granted
//! `hr_access`; and the users endpoint an admin uses to grant it.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::repo::users;
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// A session token for an existing user.
async fn token(pool: &PgPool, user_id: i64) -> String {
    let token = auth::generate_token();
    sessions::insert(
        pool,
        NewSession {
            user_id,
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

async fn token_for(pool: &PgPool, role: Role) -> (i64, String) {
    let user = common::user(pool, role).await;
    (user.user_id, token(pool, user.user_id).await)
}

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(b) => {
            builder = builder.header("content-type", "application/json");
            Body::from(b.to_string())
        }
        None => Body::empty(),
    };
    let response = api::router(common::state(pool.clone()))
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn only_admins_and_flagged_users_open_the_directory(pool: PgPool) {
    let (_, admin) = token_for(&pool, Role::Admin).await;
    let (office_id, office) = token_for(&pool, Role::Office).await;
    let (_, viewer) = token_for(&pool, Role::Viewer).await;

    // Admin: in. Everyone else: out, whatever their role.
    assert_eq!(
        call(&pool, "GET", "/api/hr/employees", &admin, None)
            .await
            .0,
        StatusCode::OK
    );
    for t in [&office, &viewer] {
        assert_eq!(
            call(&pool, "GET", "/api/hr/employees", t, None).await.0,
            StatusCode::FORBIDDEN
        );
        let (status, _) = call(
            &pool,
            "POST",
            "/api/hr/employees",
            t,
            Some(json!({ "full_name": "Nem Szabad" })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // The admin grants the flag through the users endpoint; the next request already sees it.
    let (status, user) = call(
        &pool,
        "PATCH",
        &format!("/api/users/{office_id}"),
        &admin,
        Some(json!({ "hr_access": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["hr_access"], true);
    assert_eq!(
        call(&pool, "GET", "/api/hr/employees", &office, None)
            .await
            .0,
        StatusCode::OK
    );
    // /auth/me tells the web app to show the menu entry.
    let (_, me) = call(&pool, "GET", "/api/auth/me", &office, None).await;
    assert_eq!(me["user"]["hr_access"], true);
    let (_, me) = call(&pool, "GET", "/api/auth/me", &viewer, None).await;
    assert_eq!(me["user"]["hr_access"], false);

    // The flag is not an admin key: managing users stays admin-only.
    assert_eq!(
        call(&pool, "GET", "/api/users", &office, None).await.0,
        StatusCode::FORBIDDEN
    );

    // And it can be taken away again.
    call(
        &pool,
        "PATCH",
        &format!("/api/users/{office_id}"),
        &admin,
        Some(json!({ "hr_access": false })),
    )
    .await;
    assert_eq!(
        call(&pool, "GET", "/api/hr/employees", &office, None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn the_users_list_shows_everyone_with_their_flag(pool: PgPool) {
    let (_, admin) = token_for(&pool, Role::Admin).await;
    let (office_id, _) = token_for(&pool, Role::Office).await;
    users::update(&pool, office_id, None, None, None, Some(true), None)
        .await
        .unwrap();
    token_for(&pool, Role::Designer).await;

    let (status, list) = call(&pool, "GET", "/api/users", &admin, None).await;
    assert_eq!(status, StatusCode::OK);
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 3);
    let flagged: Vec<_> = items.iter().filter(|u| u["hr_access"] == true).collect();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0]["id"], office_id);
}

#[sqlx::test(migrations = "./migrations")]
async fn employees_are_created_edited_searched_and_archived(pool: PgPool) {
    let (_, admin) = token_for(&pool, Role::Admin).await;

    let (status, created) = call(
        &pool,
        "POST",
        "/api/hr/employees",
        &admin,
        Some(json!({
            "full_name": "  Kiss Péter ",
            "email": "Peter@Example.HU",
            "company_phone": "+36 30 111 2222",
            "personal_phone": "+36 20 333 4444"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["full_name"], "Kiss Péter");
    assert_eq!(created["email"], "peter@example.hu");
    assert_eq!(created["photo_url"], Value::Null);
    let id = created["id"].as_i64().unwrap();

    // Bad input is refused.
    for bad in [
        json!({ "full_name": "  " }),
        json!({ "full_name": "X", "email": "nope" }),
        json!({}),
    ] {
        let (status, _) = call(&pool, "POST", "/api/hr/employees", &admin, Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // PATCH: absent keeps, null clears.
    let (status, patched) = call(
        &pool,
        "PATCH",
        &format!("/api/hr/employees/{id}"),
        &admin,
        Some(json!({ "personal_phone": null, "company_phone": "+36 1 555 0000" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patched["personal_phone"], Value::Null);
    assert_eq!(patched["company_phone"], "+36 1 555 0000");
    assert_eq!(patched["email"], "peter@example.hu");

    call(
        &pool,
        "POST",
        "/api/hr/employees",
        &admin,
        Some(json!({ "full_name": "Nagy Anna", "email": "anna@example.hu" })),
    )
    .await;

    // Search matches name and email; the list is alphabetical.
    let (_, all) = call(&pool, "GET", "/api/hr/employees", &admin, None).await;
    let names: Vec<_> = all["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["full_name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, ["Kiss Péter", "Nagy Anna"]);
    let (_, found) = call(&pool, "GET", "/api/hr/employees?q=anna", &admin, None).await;
    assert_eq!(found["items"].as_array().unwrap().len(), 1);

    // Archiving hides them from the default list, not from the detail or the full list.
    let (status, archived) = call(
        &pool,
        "POST",
        &format!("/api/hr/employees/{id}/archive"),
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(archived["archived_at"].is_string());
    let (_, active) = call(&pool, "GET", "/api/hr/employees", &admin, None).await;
    assert_eq!(active["items"].as_array().unwrap().len(), 1);
    let (_, everyone) = call(
        &pool,
        "GET",
        "/api/hr/employees?include_archived=true",
        &admin,
        None,
    )
    .await;
    assert_eq!(everyone["items"].as_array().unwrap().len(), 2);
    call(
        &pool,
        "POST",
        &format!("/api/hr/employees/{id}/unarchive"),
        &admin,
        None,
    )
    .await;
    let (_, back) = call(
        &pool,
        "GET",
        &format!("/api/hr/employees/{id}"),
        &admin,
        None,
    )
    .await;
    assert_eq!(back["archived_at"], Value::Null);

    assert_eq!(
        call(&pool, "GET", "/api/hr/employees/999999", &admin, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_photo_is_stored_squared_and_served_by_link(pool: PgPool) {
    let state = common::state(pool.clone());
    if !common::storage_available(&state).await {
        return;
    }
    let (_, admin) = token_for(&pool, Role::Admin).await;
    let (_, created) = call(
        &pool,
        "POST",
        "/api/hr/employees",
        &admin,
        Some(json!({ "full_name": "Fotós Feri" })),
    )
    .await;
    let id = created["id"].as_i64().unwrap();

    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::new(900, 600))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let put = |bytes: Vec<u8>| {
        Request::builder()
            .method("PUT")
            .uri(format!("/api/hr/employees/{id}/photo"))
            .header("authorization", format!("Bearer {admin}"))
            .header("content-type", "application/octet-stream")
            .body(Body::from(bytes))
            .unwrap()
    };
    let response = api::router(state.clone())
        .oneshot(put(png.into_inner()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body["photo_url"].as_str().unwrap().contains("hr/employees"));

    // Not an image: refused, and the picture stays.
    let response = api::router(state.clone())
        .oneshot(put(b"not an image".to_vec()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let (status, removed) = call(
        &pool,
        "DELETE",
        &format!("/api/hr/employees/{id}/photo"),
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["photo_url"], Value::Null);
}

#[sqlx::test(migrations = "./migrations")]
async fn admins_grant_single_capabilities_on_top_of_the_role(pool: PgPool) {
    let (_, admin) = token_for(&pool, Role::Admin).await;
    let (viewer_id, viewer) = token_for(&pool, Role::Viewer).await;
    let partner = json!({ "kind": "business", "name": "Grant Kft." });

    let (status, _) = call(&pool, "POST", "/api/partners", &viewer, Some(partner.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, user) = call(
        &pool,
        "PATCH",
        &format!("/api/users/{viewer_id}"),
        &admin,
        Some(json!({ "permissions": ["edit_partners"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(user["permissions"], json!(["edit_partners"]));

    let (status, _) = call(&pool, "POST", "/api/partners", &viewer, Some(partner)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, me) = call(&pool, "GET", "/api/auth/me", &viewer, None).await;
    let caps = me["user"]["capabilities"].as_array().unwrap();
    assert!(caps.contains(&json!("edit_partners")));
    assert!(!caps.contains(&json!("edit_orders")));

    // Managing users is the admin role itself, so it is never granted per user.
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/users/{viewer_id}"),
        &admin,
        Some(json!({ "permissions": ["manage_users"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
