//! Onboarding by invitation (0066) and the finer HR rights next to it: an admin invites by
//! email, the link sets the look and the password once and signs the browser in, the
//! letter stays out of everyone else's email log; leave and recruitment can be granted on
//! their own, and the leave keeper sees the staff by name only.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::{auth, notifications};
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(t) = bearer {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req
            .header("content-type", "application/json")
            .body(Body::from(b.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let response = api::router(common::state(pool.clone()))
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn bearer(pool: &PgPool, role: Role) -> (String, i64) {
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
    (token, user.user_id)
}

/// The token in the newest invitation letter to `to`.
async fn link_token(pool: &PgPool, to: &str) -> String {
    let body: String = sqlx::query_scalar(
        "SELECT body_text FROM email_messages
          WHERE trigger = 'user_invite' AND to_address = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(to)
    .fetch_one(pool)
    .await
    .unwrap();
    let at = body
        .find("/hu/welcome/")
        .expect("the letter carries the link")
        + "/hu/welcome/".len();
    body[at..].split_whitespace().next().unwrap().to_string()
}

const PASSWORD: &str = "Hutoauto-Teszt-2026!";

#[sqlx::test(migrations = "./migrations")]
async fn an_invited_colleague_picks_a_theme_and_a_password_once(pool: PgPool) {
    let (admin, admin_id) = bearer(&pool, Role::Admin).await;
    let (status, _, invited) = call(
        &pool,
        "POST",
        "/api/users/invite",
        Some(&admin),
        Some(json!({ "email": "Uj.Kollega@Autotherm.test", "display_name": "Új Kolléga", "role": "worker" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{invited}");
    let email = invited["user"]["email"].as_str().unwrap().to_string();
    assert_eq!(invited["user"]["must_change_password"], true);

    // The letter goes out as the admin's, so neither the send window nor the switch holds it.
    let (sent_by, trigger): (Option<i64>, String) =
        sqlx::query_as("SELECT sent_by, trigger FROM email_messages WHERE to_address = $1")
            .bind(&email)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((sent_by, trigger.as_str()), (Some(admin_id), "user_invite"));

    let token = link_token(&pool, &email).await;
    let (status, _, info) = call(
        &pool,
        "GET",
        &format!("/api/onboarding/{token}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(info["display_name"], "Új Kolléga");

    // Nothing usable can be signed in with before the link is used.
    let (status, _, _) = call(
        &pool,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let uri = format!("/api/onboarding/{token}");
    let (status, _, _) = call(
        &pool,
        "POST",
        &uri,
        None,
        Some(json!({ "password": PASSWORD, "theme": "pink" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = call(
        &pool,
        "POST",
        &uri,
        None,
        Some(json!({ "password": "rövid", "theme": "dark" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, headers, welcome) = call(
        &pool,
        "POST",
        &uri,
        None,
        Some(json!({ "password": PASSWORD, "theme": "dark" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{welcome}");
    assert_eq!(welcome["user"]["must_change_password"], false);
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.starts_with("autocrm_session=") && cookie.contains("HttpOnly"));

    let theme: String = sqlx::query_scalar(
        "SELECT s.theme FROM user_settings s JOIN users u ON u.id = s.user_id WHERE u.email = $1",
    )
    .bind(&email)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(theme, "dark");

    // Once only; and the password now works at the login too.
    let (status, _, again) = call(
        &pool,
        "POST",
        &uri,
        None,
        Some(json!({ "password": PASSWORD, "theme": "light" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(again["error"]["code"], "invite_used", "{again}");
    let (status, _, _) = call(
        &pool,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "email": email, "password": PASSWORD })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_new_letter_retires_the_old_link_and_only_admins_invite(pool: PgPool) {
    let (admin, _) = bearer(&pool, Role::Admin).await;
    let (office, _) = bearer(&pool, Role::Office).await;
    let body = json!({ "email": "masik@autotherm.test", "display_name": "Másik Kolléga", "role": "office" });
    let (status, _, _) = call(
        &pool,
        "POST",
        "/api/users/invite",
        Some(&office),
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (_, _, invited) = call(
        &pool,
        "POST",
        "/api/users/invite",
        Some(&admin),
        Some(body.clone()),
    )
    .await;
    let id = invited["user"]["id"].as_i64().unwrap();
    let first = link_token(&pool, "masik@autotherm.test").await;

    // The same address twice is a conflict, not a second account.
    let (status, _, _) = call(&pool, "POST", "/api/users/invite", Some(&admin), Some(body)).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _, _) = call(
        &pool,
        "POST",
        &format!("/api/users/{id}/invite"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second = link_token(&pool, "masik@autotherm.test").await;
    assert_ne!(first, second);
    let (status, _, _) = call(
        &pool,
        "GET",
        &format!("/api/onboarding/{first}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = call(
        &pool,
        "GET",
        &format!("/api/onboarding/{second}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // An expired link says so.
    sqlx::query("UPDATE user_invites SET created_at = now() - interval '9 days', expires_at = now() - interval '1 day' WHERE user_id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _, dead) = call(
        &pool,
        "GET",
        &format!("/api/onboarding/{second}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(dead["error"]["code"], "invite_expired", "{dead}");
}

#[sqlx::test(migrations = "./migrations")]
async fn the_invitation_letter_stays_out_of_the_shared_email_log(pool: PgPool) {
    let (admin, _) = bearer(&pool, Role::Admin).await;
    let (office, _) = bearer(&pool, Role::Office).await;
    call(
        &pool,
        "POST",
        "/api/users/invite",
        Some(&admin),
        Some(json!({ "email": "titkos@autotherm.test", "display_name": "Titkos Link", "role": "worker" })),
    )
    .await;
    let id: i64 = sqlx::query_scalar("SELECT id FROM email_messages WHERE trigger = 'user_invite'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let (_, _, log) = call(&pool, "GET", "/api/emails", Some(&office), None).await;
    assert!(
        log["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["id"] != id),
        "office sees the invitation in the log"
    );
    let (status, _, _) = call(
        &pool,
        "GET",
        &format!("/api/emails/{id}"),
        Some(&office),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, _, log) = call(&pool, "GET", "/api/emails", Some(&admin), None).await;
    assert!(
        log["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == id)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn leave_alone_shows_the_staff_by_name_and_recruitment_alone_hears_of_applicants(
    pool: PgPool,
) {
    let (admin, _) = bearer(&pool, Role::Admin).await;
    let (head, head_id) = bearer(&pool, Role::Designer).await;
    let (hiring, hiring_id) = bearer(&pool, Role::Office).await;
    let (other, other_id) = bearer(&pool, Role::Designer).await;
    sqlx::query("UPDATE users SET permissions = '{manage_leave}' WHERE id = $1")
        .bind(head_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET permissions = '{manage_recruitment}' WHERE id = $1")
        .bind(hiring_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, _, _) = call(
        &pool,
        "POST",
        "/api/hr/employees",
        Some(&admin),
        Some(json!({ "full_name": "Szerelő Szilárd", "personal_phone": "+36 30 123 4567", "email": "szilard@example.hu" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // The leave keeper: names, no phone numbers, no search by them, no directory.
    let (status, _, staff) = call(&pool, "GET", "/api/hr/employees", Some(&head), None).await;
    assert_eq!(status, StatusCode::OK);
    let person = &staff["items"][0];
    assert_eq!(person["full_name"], "Szerelő Szilárd");
    assert!(person["personal_phone"].is_null() && person["email"].is_null());
    let (_, _, by_phone) = call(&pool, "GET", "/api/hr/employees?q=4567", Some(&head), None).await;
    assert_eq!(by_phone["items"].as_array().unwrap().len(), 0);
    let (status, _, _) = call(
        &pool,
        "GET",
        "/api/hr/absences?from=2026-11-01&to=2026-11-30",
        Some(&head),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&pool, "GET", "/api/hr/jobs", Some(&head), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Without the grant: nothing of HR.
    let (status, _, _) = call(&pool, "GET", "/api/hr/employees", Some(&other), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The hiring office hand: the listings, and the applicant notification.
    let (status, _, _) = call(&pool, "GET", "/api/hr/jobs", Some(&hiring), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(&pool, "GET", "/api/hr/employees", Some(&hiring), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    notifications::application_arrived(&pool, 1, "Hűtőszerelő", "Jelentkező János")
        .await
        .unwrap();
    for (user, expected) in [(hiring_id, 1), (head_id, 0), (other_id, 0)] {
        let n: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = 'application'",
        )
        .bind(user)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(n, expected, "user {user}");
    }

    // The menu follows: `me` lists the part, and the full-directory flag stays off.
    let (_, _, me) = call(&pool, "GET", "/api/auth/me", Some(&head), None).await;
    let caps = me["user"]["capabilities"].as_array().unwrap();
    assert!(caps.iter().any(|c| c == "manage_leave"));
    assert_eq!(me["user"]["hr_access"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn colleague_names_are_for_everyone_and_settings_are_not(pool: PgPool) {
    let (worker, _) = bearer(&pool, Role::Worker).await;
    let (admin, _) = bearer(&pool, Role::Admin).await;
    let (status, _, list) = call(&pool, "GET", "/api/colleagues", Some(&worker), None).await;
    assert_eq!(status, StatusCode::OK);
    let first = &list["items"][0];
    assert!(first["display_name"].is_string() && first.get("email").is_none());
    let (status, _, _) = call(&pool, "GET", "/api/users", Some(&worker), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The mail server and where alerts go are not the shop floor's business.
    let (status, _, _) = call(&pool, "GET", "/api/settings", Some(&worker), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, _) = call(&pool, "GET", "/api/settings", Some(&admin), None).await;
    assert_eq!(status, StatusCode::OK);

    // The look is a preference like the others.
    let (status, _, prefs) = call(
        &pool,
        "PUT",
        "/api/auth/preferences",
        Some(&worker),
        Some(json!({ "theme": "dark" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prefs["theme"], "dark");
    let (status, _, _) = call(
        &pool,
        "PUT",
        "/api/auth/preferences",
        Some(&worker),
        Some(json!({ "theme": "neon" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
