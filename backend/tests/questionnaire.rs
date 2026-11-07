//! The build questionnaire (0065), end to end over HTTP: the seeded sections, answers on a
//! lead, the conversion that refuses while required answers are missing, the answers on
//! the order, the phone's cooling serial, and settings.

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

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    bearer: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {bearer}"));
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

async fn login(pool: &PgPool, role: Role) -> String {
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

async fn type_id(pool: &PgPool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM project_types WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn lead(pool: &PgPool, office: &str) -> i64 {
    let partner = common::partner(pool, "Hűtő Kft", None).await;
    let (status, lead) = call(
        pool,
        "POST",
        "/api/leads",
        office,
        Some(json!({ "title": "Doboz hűtéssel", "partner_id": partner })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{lead}");
    lead["id"].as_i64().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn the_client_types_and_the_seeded_sections_are_there(pool: PgPool) {
    let office = login(&pool, Role::Office).await;
    let (status, types) = call(&pool, "GET", "/api/project-types", &office, None).await;
    assert_eq!(status, StatusCode::OK);
    let active: Vec<&str> = types["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["is_active"] == true)
        .map(|t| t["label_hu"].as_str().unwrap())
        .collect();
    assert_eq!(
        active,
        [
            "Doboz",
            "Furgon szigetelés",
            "Halottszállító",
            "Egyedi projekt"
        ]
    );

    let (status, qn) = call(&pool, "GET", "/api/config/questionnaire", &office, None).await;
    assert_eq!(status, StatusCode::OK);
    let section = |key: &str| {
        qn["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .cloned()
            .unwrap()
    };
    assert_eq!(section("heating")["shown_when"], "heats");
    assert_eq!(section("cooling")["shown_when"], "cools");
    assert_eq!(section("freezer")["shown_when"], "freezes");
    assert_eq!(section("general")["project_type_ids"], json!([]));
    assert_eq!(
        section("hearse")["project_type_ids"],
        json!([type_id(&pool, "hearse").await])
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn converting_refuses_until_the_required_answers_are_in(pool: PgPool) {
    let office = login(&pool, Role::Office).await;
    let id = lead(&pool, &office).await;
    let box_type = type_id(&pool, "refrigerated_body").await;

    // Filled in part on the lead: a box that heats and cools, the inner size half given.
    let (status, spec) = call(
        &pool,
        "PUT",
        &format!("/api/leads/{id}/spec"),
        &office,
        Some(json!({
            "project_type_id": box_type,
            "answers": {
                "heats": true,
                "cools": true,
                "box_length_mm": "4200",
                "hearse_coffins": 2,
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{spec}");
    assert_eq!(spec["project_type_id"], box_type);
    assert_eq!(
        spec["answers"],
        json!({ "heats": true, "cools": true, "box_length_mm": 4200 }),
        "a hearse question is not asked of a box"
    );

    let (status, detail) = call(&pool, "GET", &format!("/api/leads/{id}"), &office, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["spec"]["answers"]["box_length_mm"], 4200);

    // Converting now names what is missing, and nothing is created.
    let (status, err) = call(
        &pool,
        "POST",
        &format!("/api/leads/{id}/convert"),
        &office,
        Some(json!({ "currency": "HUF" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    assert_eq!(err["error"]["code"], "spec_incomplete");
    let message = err["error"]["message"].as_str().unwrap();
    for key in [
        "box_width_mm",
        "box_height_mm",
        "cool_temp_c",
        "freezes",
        "heat_temp_c",
    ] {
        assert!(message.contains(key), "{message} should name {key}");
    }
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders WHERE lead_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(orders, 0);

    // A freezer adds its own required question.
    let (status, err) = call(
        &pool,
        "POST",
        &format!("/api/leads/{id}/convert"),
        &office,
        Some(json!({
            "currency": "HUF",
            "spec_answers": {
                "box_width_mm": 2400, "box_height_mm": 2200, "cool_temp_c": 2,
                "heat_temp_c": 20, "freezes": true,
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    assert!(
        err["error"]["message"]
            .as_str()
            .unwrap()
            .ends_with("freeze_temp_c"),
        "{err}"
    );

    // The dialog fills the rest: the order is made with every answer, and the lead keeps
    // them.
    let (status, order) = call(
        &pool,
        "POST",
        &format!("/api/leads/{id}/convert"),
        &office,
        Some(json!({
            "currency": "HUF",
            "spec_answers": {
                "box_width_mm": 2400, "box_height_mm": 2200, "cool_temp_c": "-18,5",
                "heat_temp_c": 20, "freezes": true, "freeze_temp_c": -22,
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    assert_eq!(order["project_type_id"], box_type, "the lead's type");
    let order_id = order["id"].as_i64().unwrap();

    let (_, detail) = call(
        &pool,
        "GET",
        &format!("/api/orders/{order_id}"),
        &office,
        None,
    )
    .await;
    let answers = &detail["spec_answers"];
    assert_eq!(answers["cool_temp_c"], -18.5);
    assert_eq!(answers["freeze_temp_c"], -22);
    assert_eq!(answers["box_length_mm"], 4200);
    assert_eq!(answers["heats"], true);

    let (_, lead_detail) = call(&pool, "GET", &format!("/api/leads/{id}"), &office, None).await;
    assert_eq!(lead_detail["spec"]["answers"], *answers);
}

#[sqlx::test(migrations = "./migrations")]
async fn answers_are_checked_and_hidden_ones_dropped_on_the_order(pool: PgPool) {
    let office = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let van = type_id(&pool, "van_conversion").await;
    sqlx::query("UPDATE orders SET project_type_id = $2 WHERE id = $1")
        .bind(order.id)
        .bind(van)
        .execute(&pool)
        .await
        .unwrap();

    let (status, err) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/spec", order.id),
        &office,
        Some(json!({ "answers": { "cools": true, "cool_temp_c": -60 } })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");
    assert!(
        err["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cool_temp_c"),
        "{err}"
    );

    let (status, saved) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/spec", order.id),
        &office,
        Some(json!({ "answers": {
            "heats": false, "cools": true, "cool_temp_c": 4, "van_size": " L2H2 ",
            "defrost": "Kézi", "heat_temp_c": 30, "box_length_mm": 4000,
        } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(
        saved["answers"],
        json!({
            "heats": false, "cools": true, "cool_temp_c": 4, "van_size": "L2H2",
            "defrost": "Kézi",
        })
    );

    let (status, err) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/spec", order.id),
        &office,
        Some(json!({ "answers": { "cools": true, "defrost": "Néha" } })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");

    // The workshop reads the spec but does not write it.
    let worker = login(&pool, Role::Worker).await;
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/spec", order.id),
        &worker,
        Some(json!({ "answers": {} })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn the_phone_files_the_cooling_serial_only_where_cooling_is_asked(pool: PgPool) {
    let office = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/cooling-serial", order.id),
        &office,
        Some(json!({ "serial": "TK-1" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no cooling on this order");

    // An old value today's limits would refuse does not stop the serial.
    sqlx::query(
        r#"UPDATE orders SET spec_answers = '{"cools": true, "heats": true, "heat_output_kw": 250}' WHERE id = $1"#,
    )
    .bind(order.id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, saved) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/cooling-serial", order.id),
        &office,
        Some(json!({ "serial": "  TK-998877 " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["answers"]["cooling_unit_serial"], "TK-998877");
    assert_eq!(saved["answers"]["heat_output_kw"], 250);

    let (_, cleared) = call(
        &pool,
        "PUT",
        &format!("/api/orders/{}/cooling-serial", order.id),
        &office,
        Some(json!({ "serial": null })),
    )
    .await;
    assert!(cleared["answers"].get("cooling_unit_serial").is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn settings_add_questions_and_refuse_loops(pool: PgPool) {
    let admin = login(&pool, Role::Admin).await;
    let office = login(&pool, Role::Office).await;
    let hearse = type_id(&pool, "hearse").await;

    let (status, _) = call(
        &pool,
        "POST",
        "/api/config/questionnaire/sections",
        &office,
        Some(json!({ "key": "ramp", "title": "Rámpa" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, qn) = call(
        &pool,
        "POST",
        "/api/config/questionnaire/sections",
        &admin,
        Some(json!({
            "key": "ramp", "title": "Rámpa", "shown_when": "heats",
            "project_type_ids": [hearse],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{qn}");
    let ramp = qn["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "ramp")
        .unwrap()
        .clone();
    assert_eq!(ramp["shown_when"], "heats");
    let ramp_id = ramp["id"].as_i64().unwrap();

    let (status, qn) = call(
        &pool,
        "POST",
        "/api/config/questionnaire/questions",
        &admin,
        Some(json!({
            "section_id": ramp_id, "key": "ramp_kind", "label": "Rámpa fajtája",
            "kind": "multi_choice", "options": ["Kézi", " Elektromos ", "Kézi", ""],
            "required": true,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{qn}");
    let question = qn["sections"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["questions"].as_array().unwrap())
        .find(|q| q["key"] == "ramp_kind")
        .unwrap()
        .clone();
    assert_eq!(question["options"], json!(["Kézi", "Elektromos"]));

    // A yes/no question inside the section, then the section waiting on it: refused.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/config/questionnaire/questions",
        &admin,
        Some(json!({ "section_id": ramp_id, "key": "ramp_lit", "label": "Világítás", "kind": "yes_no" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, err) = call(
        &pool,
        "PATCH",
        &format!("/api/config/questionnaire/sections/{ramp_id}"),
        &admin,
        Some(json!({ "shown_when": "ramp_lit" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{err}");

    // Only a yes/no question opens a section.
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/config/questionnaire/sections/{ramp_id}"),
        &admin,
        Some(json!({ "shown_when": "cool_temp_c" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The heating switch cannot be switched off while sections open on it.
    let heats_id: i64 = sqlx::query_scalar("SELECT id FROM spec_questions WHERE key = 'heats'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/config/questionnaire/questions/{heats_id}"),
        &admin,
        Some(json!({ "is_active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Options only for choices.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/config/questionnaire/questions",
        &admin,
        Some(json!({ "section_id": ramp_id, "key": "ramp_note", "label": "Megjegyzés", "kind": "text", "options": ["x"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
