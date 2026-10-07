//! HR recruitment: job listings, the public application form, and the applicant profiles it
//! creates.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

async fn token_for(pool: &PgPool, role: Role) -> String {
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

async fn send(pool: &PgPool, req: Request<Body>) -> (StatusCode, Value) {
    let response = api::router(common::state(pool.clone()))
        .oneshot(req)
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
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
    send(pool, builder.body(body).unwrap()).await
}

const BOUNDARY: &str = "XBOUNDARYX";

/// A `multipart/form-data` body: text fields, then an optional resume file.
fn multipart(fields: &[(&str, &str)], resume: Option<(&str, &[u8])>) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    if let Some((filename, bytes)) = resume {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"resume\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn apply(
    pool: &PgPool,
    slug: &str,
    fields: &[(&str, &str)],
    resume: Option<(&str, &[u8])>,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/public/jobs/{slug}/applications"))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(multipart(fields, resume)))
        .unwrap();
    send(pool, req).await
}

fn good_fields() -> Vec<(&'static str, &'static str)> {
    vec![
        ("full_name", "Kiss Péter"),
        ("email", "Peter@Example.HU"),
        ("phone", "+36 30 123 4567"),
        ("age", "27"),
        ("city", "Budapest"),
        ("message", "Szívesen dolgoznék önöknél."),
    ]
}

/// The slug of a listing, from its public link.
fn slug_of(job: &Value) -> String {
    job["public_url"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string()
}

async fn new_job(pool: &PgPool, admin: &str, publish: bool) -> Value {
    let (status, job) = call(
        pool,
        "POST",
        "/api/hr/jobs",
        admin,
        Some(
            json!({ "title": "Hűtős szerelő", "description": "Műszakban", "location": "Budapest" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    if !publish {
        return job;
    }
    let (status, job) = call(
        pool,
        "POST",
        &format!("/api/hr/jobs/{}/publish", job["id"]),
        admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    job
}

#[sqlx::test(migrations = "./migrations")]
async fn only_hr_can_manage_listings(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let office = token_for(&pool, Role::Office).await;
    let job = new_job(&pool, &admin, false).await;
    let id = job["id"].as_i64().unwrap();

    for (method, uri, body) in [
        ("GET", "/api/hr/jobs".to_string(), None),
        (
            "POST",
            "/api/hr/jobs".to_string(),
            Some(json!({ "title": "X" })),
        ),
        ("GET", format!("/api/hr/jobs/{id}/applications"), None),
        ("POST", format!("/api/hr/jobs/{id}/publish"), None),
        ("DELETE", "/api/hr/applications/1".to_string(), None),
    ] {
        assert_eq!(
            call(&pool, method, &uri, &office, body).await.0,
            StatusCode::FORBIDDEN,
            "{method} {uri}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_listing_is_a_draft_until_published_and_the_link_follows(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let job = new_job(&pool, &admin, false).await;
    assert_eq!(job["status"], "draft");
    assert!(
        job["public_url"]
            .as_str()
            .unwrap()
            .starts_with("http://localhost:3000/hu/jobs/")
    );
    let slug = slug_of(&job);
    let id = job["id"].as_i64().unwrap();

    // A draft does not exist for the public: not to read, not to apply to.
    let anon = Request::builder()
        .uri(format!("/api/public/jobs/{slug}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&pool, anon).await.0, StatusCode::NOT_FOUND);
    let (status, _) = apply(&pool, &slug, &good_fields(), Some(("cv.pdf", b"%PDF-1.4"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Published: the page loads, with no login.
    let (_, published) = call(
        &pool,
        "POST",
        &format!("/api/hr/jobs/{id}/publish"),
        &admin,
        None,
    )
    .await;
    assert_eq!(published["status"], "published");
    assert!(published["published_at"].is_string());
    let anon = Request::builder()
        .uri(format!("/api/public/jobs/{slug}"))
        .body(Body::empty())
        .unwrap();
    let (status, page) = send(&pool, anon).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["title"], "Hűtős szerelő");
    assert_eq!(page["status"], "published");
    // The public page carries nothing internal.
    assert!(page.get("id").is_none() && page.get("application_count").is_none());

    // Closed: still readable (it says so), and applications are refused.
    call(
        &pool,
        "POST",
        &format!("/api/hr/jobs/{id}/close"),
        &admin,
        None,
    )
    .await;
    let anon = Request::builder()
        .uri(format!("/api/public/jobs/{slug}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&pool, anon).await.1["status"], "closed");
    let (status, _) = apply(&pool, &slug, &good_fields(), Some(("cv.pdf", b"%PDF-1.4"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_form_refuses_bad_details_and_unusable_resumes(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let slug = slug_of(&new_job(&pool, &admin, true).await);
    let pdf: &[u8] = b"%PDF-1.4 resume";

    // Each of the required details, missing.
    for missing in ["full_name", "email", "phone", "age"] {
        let fields: Vec<_> = good_fields()
            .into_iter()
            .filter(|(k, _)| *k != missing)
            .collect();
        let (status, _) = apply(&pool, &slug, &fields, Some(("cv.pdf", pdf))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "missing {missing}");
    }
    // A nonsense age.
    let mut fields = good_fields();
    fields.retain(|(k, _)| *k != "age");
    fields.push(("age", "7"));
    assert_eq!(
        apply(&pool, &slug, &fields, Some(("cv.pdf", pdf))).await.0,
        StatusCode::BAD_REQUEST
    );
    // No resume, an empty one, and a renamed executable.
    assert_eq!(
        apply(&pool, &slug, &good_fields(), None).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        apply(&pool, &slug, &good_fields(), Some(("cv.pdf", b"")))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        apply(
            &pool,
            &slug,
            &good_fields(),
            Some(("cv.pdf", b"MZ\x90\x00"))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    // None of it left a profile behind.
    let (_, jobs) = call(&pool, "GET", "/api/hr/jobs", &admin, None).await;
    assert_eq!(jobs["items"][0]["application_count"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_filled_honeypot_is_dropped_without_a_profile(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let job = new_job(&pool, &admin, true).await;
    let mut fields = good_fields();
    fields.push(("company", "Spam Kft."));
    let (status, _) = apply(
        &pool,
        &slug_of(&job),
        &fields,
        Some(("cv.pdf", b"%PDF-1.4")),
    )
    .await;
    // The bot is told it worked.
    assert_eq!(status, StatusCode::CREATED);
    let (_, jobs) = call(&pool, "GET", "/api/hr/jobs", &admin, None).await;
    assert_eq!(jobs["items"][0]["application_count"], 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn an_application_creates_a_profile_with_its_resume(pool: PgPool) {
    let state = common::state(pool.clone());
    if !common::storage_available(&state).await {
        return;
    }
    let admin = token_for(&pool, Role::Admin).await;
    let job = new_job(&pool, &admin, true).await;
    let slug = slug_of(&job);
    let id = job["id"].as_i64().unwrap();

    let (status, _) = apply(
        &pool,
        &slug,
        &good_fields(),
        Some(("Önéletrajz.pdf", b"%PDF-1.4 hello")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // The same person applying twice is one profile.
    let (status, _) = apply(
        &pool,
        &slug,
        &good_fields(),
        Some(("again.pdf", b"%PDF-1.4")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // HR sees the profile, the resume attached, and a notification that it arrived.
    let (status, apps) = call(
        &pool,
        "GET",
        &format!("/api/hr/jobs/{id}/applications"),
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let app = &apps["items"][0];
    assert_eq!(app["full_name"], "Kiss Péter");
    assert_eq!(app["email"], "peter@example.hu");
    assert_eq!(app["age"], 27);
    assert_eq!(app["resume_filename"], "Önéletrajz.pdf");
    assert!(app["resume_url"].as_str().unwrap().contains("attachment"));
    let (_, feed) = call(&pool, "GET", "/api/notifications", &admin, None).await;
    assert_eq!(feed["items"][0]["title"], "Új jelentkezés");

    // Notes from the phone interview.
    let app_id = app["id"].as_i64().unwrap();
    let (status, noted) = call(
        &pool,
        "PATCH",
        &format!("/api/hr/applications/{app_id}"),
        &admin,
        Some(json!({ "notes": "Telefonon jó benyomást tett." })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(noted["notes"], "Telefonon jó benyomást tett.");

    // A listing with applicants cannot be deleted, only closed.
    assert_eq!(
        call(&pool, "DELETE", &format!("/api/hr/jobs/{id}"), &admin, None)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );

    // Deleting the profile is how an applicant stops being a candidate.
    assert_eq!(
        call(
            &pool,
            "DELETE",
            &format!("/api/hr/applications/{app_id}"),
            &admin,
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (_, apps) = call(
        &pool,
        "GET",
        &format!("/api/hr/jobs/{id}/applications"),
        &admin,
        None,
    )
    .await;
    assert!(apps["items"].as_array().unwrap().is_empty());
    assert_eq!(
        call(
            &pool,
            "DELETE",
            &format!("/api/hr/applications/{app_id}"),
            &admin,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    // With nobody left, the listing itself can go.
    assert_eq!(
        call(&pool, "DELETE", &format!("/api/hr/jobs/{id}"), &admin, None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}
