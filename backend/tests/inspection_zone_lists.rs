//! The zone-list endpoints as a client sees them: who may read, who may edit, and what a
//! list must look like. The lists themselves (what an alváz with a box needs) are covered
//! in `inspections.rs`; this drives the real router with bearer tokens.

mod common;

use autocrm::api;
use autocrm::domain::role::Role;
use autocrm::repo::inspections::{self, NewInspection};
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

/// A signed-in user of `role`: their bearer token.
async fn token_for(pool: &PgPool, role: Role) -> String {
    let user = common::user(pool, role).await;
    let token = auth::generate_token();
    let hash = auth::token_hash(&token);
    sessions::insert(
        pool,
        NewSession {
            user_id: user.user_id,
            token_hash: &hash,
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

async fn call(
    pool: &PgPool,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let app = api::router(common::state(pool.clone()));
    let mut request = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"));
    let body = match body {
        Some(json) => {
            request = request.header("content-type", "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

async fn project_type_id(pool: &PgPool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM project_types WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn zone(key: &str, position: i32) -> Value {
    json!({
        "zone_key": key, "position": position, "title": key,
        "instruction": format!("Fotó: {key}"), "optional": false, "required": true
    })
}

fn keys(list: &Value) -> Vec<String> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|z| z["zone_key"].as_str().unwrap().to_string())
        .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn anyone_signed_in_reads_the_list_for_a_vehicle_and_a_walkaround(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;
    let alvaz = project_type_id(&pool, "refrigerated_body").await;

    let (status, outgo) = call(
        &pool,
        "GET",
        &format!("/api/inspections/templates?project_type_id={alvaz}&kind=checkin"),
        &viewer,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&outgo).len(), 39);
    assert_eq!(outgo["items"][0]["kind"], "checkin");
    assert_eq!(outgo["items"][0]["project_type_id"], alvaz);
    assert_eq!(outgo["items"][0]["set_key"], "refrigerated_body:checkin");

    // No kind means the first walkaround, which is what clients from before the kind existed
    // asked for.
    let (_, intake) = call(
        &pool,
        "GET",
        &format!("/api/inspections/templates?project_type_id={alvaz}"),
        &viewer,
        None,
    )
    .await;
    assert_eq!(keys(&intake).len(), 20);

    // A vehicle kind with no list of its own is served the general list, and says so.
    let repair = project_type_id(&pool, "repair").await;
    let (_, general) = call(
        &pool,
        "GET",
        &format!("/api/inspections/templates?project_type_id={repair}&kind=checkin"),
        &viewer,
        None,
    )
    .await;
    assert_eq!(keys(&general).len(), 15);
    assert!(general["items"][0]["project_type_id"].is_null());
    assert_eq!(general["items"][0]["set_key"], "default:checkin");

    let (status, _) = call(
        &pool,
        "GET",
        "/api/inspections/templates?kind=arrival",
        &viewer,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test(migrations = "./migrations")]
async fn only_an_admin_edits_a_list(pool: PgPool) {
    let office = token_for(&pool, Role::Office).await;
    let admin = token_for(&pool, Role::Admin).await;
    let heated = project_type_id(&pool, "heated_body").await;
    let body = json!({
        "project_type_id": heated, "kind": "checkin",
        "zones": [zone("rear", 1), zone("front", 2)]
    });

    let (status, _) = call(
        &pool,
        "PUT",
        "/api/inspections/templates",
        &office,
        Some(body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &pool,
        "DELETE",
        &format!("/api/inspections/templates?project_type_id={heated}&kind=checkin"),
        &office,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, saved) = call(
        &pool,
        "PUT",
        "/api/inspections/templates",
        &admin,
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(keys(&saved), ["rear", "front"]);

    // The next read of that vehicle kind is the new list.
    let (_, read) = call(
        &pool,
        "GET",
        &format!("/api/inspections/templates?project_type_id={heated}&kind=checkin"),
        &office,
        None,
    )
    .await;
    assert_eq!(keys(&read), ["rear", "front"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_list_has_to_be_well_formed(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let put = |body: Value| {
        let pool = pool.clone();
        let admin = admin.clone();
        async move {
            call(
                &pool,
                "PUT",
                "/api/inspections/templates",
                &admin,
                Some(body),
            )
            .await
            .0
        }
    };
    let many: Vec<Value> = (1..=81).map(|i| zone(&format!("z{i}"), i)).collect();

    assert_eq!(
        put(json!({ "project_type_id": null, "kind": "checkin", "zones": [] })).await,
        StatusCode::BAD_REQUEST,
        "an empty list would leave a walkaround with nothing to do"
    );
    assert_eq!(
        put(json!({ "project_type_id": null, "kind": "checkin", "zones": many })).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        put(json!({ "project_type_id": null, "kind": "arrival", "zones": [zone("a", 1)] })).await,
        StatusCode::BAD_REQUEST
    );
    let mut untitled = zone("a", 1);
    untitled["title"] = json!("  ");
    assert_eq!(
        put(json!({ "project_type_id": null, "kind": "checkin", "zones": [untitled] })).await,
        StatusCode::BAD_REQUEST,
        "the phone needs a heading for every zone"
    );
    // A zone is required or optional: both or neither would leave the phone guessing.
    let mut both = zone("a", 1);
    both["optional"] = json!(true); // required stays true
    let mut neither = zone("a", 1);
    neither["required"] = json!(false); // optional stays false
    for (zone, what) in [(both, "both"), (neither, "neither")] {
        assert_eq!(
            put(json!({ "project_type_id": null, "kind": "checkin", "zones": [zone] })).await,
            StatusCode::BAD_REQUEST,
            "{what} required and optional"
        );
    }
    assert_eq!(
        put(json!({ "project_type_id": 987654, "kind": "checkin", "zones": [zone("a", 1)] })).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        put(json!({ "project_type_id": null, "kind": "checkin", "zones": [zone("a", 1), zone("a", 2)] })).await,
        StatusCode::CONFLICT,
        "the same zone twice in one list"
    );

    // Nothing above changed the general list.
    let (_, general) = call(
        &pool,
        "GET",
        "/api/inspections/templates?kind=checkin",
        &admin,
        None,
    )
    .await;
    assert_eq!(keys(&general).len(), 15);
}

#[sqlx::test(migrations = "./migrations")]
async fn removing_a_vehicle_kinds_list_falls_back_but_the_general_list_stays(pool: PgPool) {
    let admin = token_for(&pool, Role::Admin).await;
    let van = project_type_id(&pool, "van_conversion").await;
    let uri = format!("/api/inspections/templates?project_type_id={van}&kind=checkout");

    let (status, _) = call(&pool, "DELETE", &uri, &admin, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, read) = call(&pool, "GET", &uri, &admin, None).await;
    assert_eq!(keys(&read).len(), 15, "back to the general list");
    // Only that walkaround: the van's outgo list is untouched.
    let (_, outgo) = call(
        &pool,
        "GET",
        &format!("/api/inspections/templates?project_type_id={van}&kind=checkin"),
        &admin,
        None,
    )
    .await;
    assert_eq!(keys(&outgo).len(), 18);

    // Nothing left to remove.
    let (status, _) = call(&pool, "DELETE", &uri, &admin, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Every walkaround needs a list to fall back to.
    let (status, _) = call(
        &pool,
        "DELETE",
        "/api/inspections/templates?kind=checkin",
        &admin,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ── Titles travel with the inspection ────────────────────────────────────────────

fn draft(order_id: i64, kind: &str, user_id: i64, checkout_id: Option<i64>) -> NewInspection {
    NewInspection {
        order_id,
        kind: kind.to_string(),
        vehicle_plate: "ABC-123".to_string(),
        vehicle_vin: None,
        inspector_name: "Szerelő Sándor".to_string(),
        driver_name: None,
        location: None,
        odometer: None,
        fuel_level: None,
        battery_pct: None,
        warning_lights: None,
        checkout_id,
        created_by: user_id,
    }
}

/// An order of the given project type (or none) with a draft inspection of `kind`.
async fn inspection_on(pool: &PgPool, project_type: Option<&str>, kind: &str) -> (i64, i64) {
    let user = common::user(pool, Role::Office).await;
    let order = common::order(pool, &user, "HUF", vec![]).await;
    if let Some(key) = project_type {
        sqlx::query("UPDATE orders SET project_type_id = $1 WHERE id = $2")
            .bind(project_type_id(pool, key).await)
            .bind(order.id)
            .execute(pool)
            .await
            .unwrap();
    }
    let inspection = inspections::create(pool, &draft(order.id, kind, user.user_id, None))
        .await
        .unwrap();
    (order.id, inspection.id)
}

async fn titles_of(
    pool: &PgPool,
    token: &str,
    inspection_id: i64,
) -> serde_json::Map<String, Value> {
    let (status, detail) = call(
        pool,
        "GET",
        &format!("/api/inspections/{inspection_id}"),
        token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    detail["zone_titles"].as_object().unwrap().clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn an_inspection_names_its_zones_from_the_list_it_was_walked_with(pool: PgPool) {
    let viewer = token_for(&pool, Role::Viewer).await;

    // An alváz at intake: the cab-and-chassis list, named.
    let (order_id, intake) = inspection_on(&pool, Some("refrigerated_body"), "checkout").await;
    let titles = titles_of(&pool, &viewer, intake).await;
    assert_eq!(titles.len(), 20);
    assert_eq!(titles["type_plate"], "Gyári adattábla");
    assert!(!titles.contains_key("box_side_door"), "no box at intake");

    // The same vehicle at outgo (which needs the signed intake): the full list.
    sqlx::query("UPDATE inspections SET status = 'signed', signed_at = now() WHERE id = $1")
        .bind(intake)
        .execute(&pool)
        .await
        .unwrap();
    let user = common::user(&pool, Role::Office).await;
    let outgo = inspections::create(
        &pool,
        &draft(order_id, "checkin", user.user_id, Some(intake)),
    )
    .await
    .unwrap();
    let titles = titles_of(&pool, &viewer, outgo.id).await;
    assert_eq!(titles.len(), 39);
    assert_eq!(titles["box_side_door"], "Doboz oldalajtaja");

    // An order with no project type is named from the general list.
    let (_, plain) = inspection_on(&pool, None, "checkout").await;
    let titles = titles_of(&pool, &viewer, plain).await;
    assert_eq!(titles.len(), 15);
    assert_eq!(titles["front_left"], "Bal első sarok");
}
