//! Migration 0048: lead sources and the quote's exchange rate, comments with mentions, the
//! yard, photo captions and annotations, the stage photo category, document versions,
//! inspection tyres, and incidents with rework jobs.

mod common;

use autocrm::api;
use autocrm::domain::media::{DocumentKind, ImageCategory};
use autocrm::domain::role::Role;
use autocrm::repo::documents::{self, NewDocument, Owner};
use autocrm::repo::images::{self, NewImage};
use autocrm::repo::inspections::{self, NewDamage, NewInspection};
use autocrm::repo::sessions::{self, NewSession, SessionKind};
use autocrm::service::auth;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::NaiveDate;
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

/// A signed-in user: (bearer token, user id).
async fn login(pool: &PgPool, role: Role) -> (String, i64) {
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

#[sqlx::test(migrations = "./migrations")]
async fn a_lead_source_is_a_key_and_free_text_becomes_other(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;

    let (status, list) = call(&pool, "GET", "/api/lead-sources", &office, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["key"] == "trade_fair")
    );

    // A key stays a key.
    let (status, lead) = call(&pool, "POST", "/api/leads", &office,
        Some(json!({ "title": "Vásári érdeklődő", "source": "trade_fair", "source_detail": "Hungexpo 2026" }))).await;
    assert_eq!(status, StatusCode::CREATED, "{lead}");
    assert_eq!(lead["source"], "trade_fair");
    assert_eq!(lead["source_detail"], "Hungexpo 2026");

    // A label typed by an older client becomes its key.
    let (_, lead) = call(
        &pool,
        "POST",
        "/api/leads",
        &office,
        Some(json!({ "title": "Telefonáló", "source": "Telefon" })),
    )
    .await;
    assert_eq!(lead["source"], "phone");

    // Free text becomes `other`, kept word for word.
    let (_, lead) = call(
        &pool,
        "POST",
        "/api/leads",
        &office,
        Some(json!({ "title": "Valaki", "source": "a szomszéd mondta" })),
    )
    .await;
    assert_eq!(lead["source"], "other");
    assert_eq!(lead["source_detail"], "a szomszéd mondta");

    // The system's own sources are never picked by hand.
    let (status, _) = call(
        &pool,
        "POST",
        "/api/leads",
        &office,
        Some(json!({ "title": "Hamis", "source": "website" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A new source gets a folded key.
    let (status, created) = call(
        &pool,
        "POST",
        "/api/lead-sources",
        &office,
        Some(json!({ "label": "Facebook hirdetés" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["key"], "facebook_hirdetes");
}

#[sqlx::test(migrations = "./migrations")]
async fn an_eur_quote_freezes_its_rate_and_the_order_is_valued_on_that_day(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;
    let today = autocrm::service::business_today(chrono_tz::Europe::Budapest);
    let rate_day = today - chrono::TimeDelta::days(2);
    autocrm::repo::fx::upsert(
        &pool,
        rate_day,
        "EUR",
        "HUF",
        "395.5".parse().unwrap(),
        "MNB",
    )
    .await
    .unwrap();
    let partner = common::partner(&pool, "Kühl GmbH", None).await;

    let (status, lead) = call(
        &pool,
        "POST",
        "/api/leads",
        &office,
        Some(json!({
            "title": "Hűtős Sprinter", "partner_id": partner,
            "quoted_value_minor": 4_500_000, "currency": "EUR"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{lead}");
    assert_eq!(lead["quote_fx_rate"], "395.50000000");
    assert_eq!(lead["quote_fx_day"], rate_day.to_string());

    // A HUF quote carries no rate.
    let id = lead["id"].as_i64().unwrap();
    let (_, patched) = call(
        &pool,
        "PATCH",
        &format!("/api/leads/{id}"),
        &office,
        Some(json!({ "currency": "HUF" })),
    )
    .await;
    assert!(patched["quote_fx_rate"].is_null());
    let (_, patched) = call(
        &pool,
        "PATCH",
        &format!("/api/leads/{id}"),
        &office,
        Some(json!({ "currency": "EUR" })),
    )
    .await;
    assert_eq!(patched["quote_fx_day"], rate_day.to_string());

    // The order it becomes is valued on the quote's day.
    let (status, order) = call(
        &pool,
        "POST",
        &format!("/api/leads/{id}/convert"),
        &office,
        Some(json!({ "currency": "EUR" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{order}");
    assert_eq!(order["valuation_date"], rate_day.to_string());
}

#[sqlx::test(migrations = "./migrations")]
async fn a_mention_notifies_the_person_named_and_only_the_author_edits(pool: PgPool) {
    let (office, office_id) = login(&pool, Role::Office).await;
    let (designer, designer_id) = login(&pool, Role::Designer).await;
    let (viewer, _) = login(&pool, Role::Viewer).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    let (status, comment) = call(
        &pool,
        "POST",
        "/api/comments",
        &office,
        Some(json!({
            "entity_type": "order", "entity_id": order.id,
            "body": "@Teszt Elek nézd meg a hátsó ajtót", "mention_ids": [designer_id, office_id]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{comment}");
    assert_eq!(comment["mentions"].as_array().unwrap().len(), 2);

    // The designer is told; the author is not told about their own comment.
    let (_, feed) = call(&pool, "GET", "/api/notifications", &designer, None).await;
    let item = &feed["items"][0];
    assert_eq!(item["kind"], "mention");
    assert_eq!(item["link"], format!("/orders/{}?tab=comments", order.id));
    let (_, own) = call(&pool, "GET", "/api/notifications", &office, None).await;
    assert!(
        own["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n["kind"] != "mention")
    );

    // Someone else cannot edit it; a viewer cannot comment at all.
    let id = comment["id"].as_i64().unwrap();
    let (status, _) = call(
        &pool,
        "PATCH",
        &format!("/api/comments/{id}"),
        &designer,
        Some(json!({ "body": "átírva" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &pool,
        "POST",
        "/api/comments",
        &viewer,
        Some(json!({
            "entity_type": "order", "entity_id": order.id, "body": "szia"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The comment is in the order's history.
    let (_, history) = call(
        &pool,
        "GET",
        &format!("/api/timeline/order/{}", order.id),
        &office,
        None,
    )
    .await;
    assert!(
        history["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "comment")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_vehicle_moves_on_the_yard_and_a_full_bay_warns(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let first = common::order(&pool, &user, "HUF", vec![]).await;
    let mut fields = common::fields(first.partner_id, "HUF");
    fields.vehicle_plate = Some("XYZ-987".into());
    let mut tx = pool.begin().await.unwrap();
    let second = autocrm::service::orders::create_in_tx(
        &mut tx,
        user.user_id,
        NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
        fields,
        vec![],
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let (_, board) = call(&pool, "GET", "/api/yard/board", &office, None).await;
    let vehicles = board["vehicles"].as_array().unwrap();
    assert_eq!(
        vehicles.len(),
        2,
        "both open jobs' vans are on the board: {board}"
    );
    assert!(vehicles.iter().all(|v| v["location_id"].is_null()));
    let bay = board["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["capacity"] == 1)
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let van = |order_id: i64| {
        vehicles.iter().find(|v| v["order_id"] == order_id).unwrap()["vehicle_id"]
            .as_i64()
            .unwrap()
    };

    let (status, moved) = call(
        &pool,
        "POST",
        "/api/yard/moves",
        &office,
        Some(json!({
            "vehicle_id": van(first.id), "location_id": bay, "order_id": first.id
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{moved}");
    assert_eq!(moved["over_capacity"], false);
    let (_, moved) = call(
        &pool,
        "POST",
        "/api/yard/moves",
        &office,
        Some(json!({
            "vehicle_id": van(second.id), "location_id": bay
        })),
    )
    .await;
    assert_eq!(
        moved["over_capacity"], true,
        "a one-car bay with two cars warns"
    );

    let (_, detail) = call(
        &pool,
        "GET",
        &format!("/api/orders/{}", first.id),
        &office,
        None,
    )
    .await;
    assert_eq!(detail["vehicle_locations"][0]["location_id"], bay);
    let (_, history) = call(
        &pool,
        "GET",
        &format!("/api/timeline/order/{}", first.id),
        &office,
        None,
    )
    .await;
    assert!(
        history["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["action"] == "vehicle_moved")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_caption_and_drawings_go_on_evidence_without_touching_it(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let mut conn = pool.acquire().await.unwrap();
    let (image, _) = images::insert(
        &mut conn,
        &NewImage {
            order_id: order.id,
            category: ImageCategory::Intake,
            storage_key: "orders/x/intake/aa.jpg",
            content_type: "image/jpeg",
            original_filename: Some("IMG_1.jpg"),
            content_hash: &[1u8; 32],
            byte_size: 10,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    assert!(image.immutable);

    let (status, patched) = call(
        &pool,
        "PATCH",
        &format!("/api/images/{}", image.id),
        &office,
        Some(json!({ "caption": "Bal hátsó lámpa már törött" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["caption"], "Bal hátsó lámpa már törött");

    let shapes =
        json!([{ "type": "ellipse", "x": 0.2, "y": 0.3, "w": 0.1, "h": 0.1, "color": "#e11d48" }]);
    let (status, saved) = call(
        &pool,
        "PUT",
        &format!("/api/images/{}/annotations", image.id),
        &office,
        Some(json!({ "shapes": shapes })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["shapes"], shapes);
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/images/{}/annotations", image.id),
        &office,
        Some(json!({ "shapes": [{ "type": "script" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (_, list) = call(
        &pool,
        "GET",
        &format!("/api/orders/{}/images", order.id),
        &office,
        None,
    )
    .await;
    assert_eq!(list["items"][0]["annotated"], true);
    assert!(
        list["items"][0]["timestamp"].is_null(),
        "no authority configured: no stamp"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn meo_defaults_new_photos_to_completion(pool: PgPool) {
    let (admin, _) = login(&pool, Role::Admin).await;
    let (_, list) = call(&pool, "GET", "/api/stage-photo-categories", &admin, None).await;
    let meo = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["stage_key"] == "meo")
        .unwrap();
    assert_eq!(meo["category"], "completion");

    let (status, _) = call(
        &pool,
        "PUT",
        "/api/stage-photo-categories/intake",
        &admin,
        Some(json!({ "category": "intake" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "evidence is never a default"
    );

    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let (_, detail) = call(
        &pool,
        "GET",
        &format!("/api/orders/{}", order.id),
        &admin,
        None,
    )
    .await;
    assert_eq!(
        detail["photo_category"], "production",
        "a new order is in intake"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn a_new_version_supersedes_the_old_one_which_stays_reachable(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let mut conn = pool.acquire().await.unwrap();
    let doc = |hash: u8, previous: Option<i64>, version: i32| NewDocument {
        owner: Owner::Order(order.id),
        vehicle_id: None,
        kind: DocumentKind::Cad,
        filename: "felepitmeny.dxf",
        content_type: "application/dxf",
        storage_key: "orders/x/documents/a.dxf",
        content_hash: Box::leak(Box::new([hash; 32])),
        byte_size: 10,
        uploaded_by: Some(user.user_id),
        source_ref: None,
        previous_version_id: previous,
        version,
    };
    let (v1, _) = documents::insert(&mut conn, &doc(1, None, 1))
        .await
        .unwrap();
    let (v2, _) = documents::insert(&mut conn, &doc(2, Some(v1.id), 2))
        .await
        .unwrap();
    documents::supersede(&mut *conn, v1.id).await.unwrap();

    let current = documents::list_for_order(&mut *conn, order.id)
        .await
        .unwrap();
    assert_eq!(
        current.iter().map(|d| d.id).collect::<Vec<_>>(),
        vec![v2.id]
    );
    let all = documents::versions(&mut *conn, v1.id).await.unwrap();
    assert_eq!(
        all.iter().map(|d| d.version).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert_eq!(
        documents::versions(&mut *conn, v2.id).await.unwrap().len(),
        2
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn tyres_are_recorded_and_a_new_damage_becomes_an_incident_with_a_rework_job(pool: PgPool) {
    let (office, _) = login(&pool, Role::Office).await;
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let new = |kind: &str| NewInspection {
        order_id: order.id,
        kind: kind.into(),
        vehicle_plate: "ABC-123".into(),
        vehicle_vin: None,
        inspector_name: "Teszt Elek".into(),
        driver_name: None,
        location: None,
        odometer: None,
        fuel_level: None,
        battery_pct: None,
        warning_lights: None,
        checkout_id: None,
        created_by: user.user_id,
    };
    let checkout = inspections::create(&pool, &new("checkout")).await.unwrap();

    let (status, tyres) = call(
        &pool,
        "PUT",
        &format!("/api/inspections/{}/tyres", checkout.id),
        &office,
        Some(json!({ "tyres": [
            { "position": "front_left", "tread_mm": "6.5", "condition": "ok" },
            { "position": "rear_right", "tread_mm": "1.2", "condition": "worn", "note": "csere" }
        ] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tyres}");
    assert_eq!(tyres.as_array().unwrap().len(), 2);
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/inspections/{}/tyres", checkout.id),
        &office,
        Some(json!({ "tyres": [{ "position": "roof", "condition": "ok" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, detail) = call(
        &pool,
        "GET",
        &format!("/api/inspections/{}", checkout.id),
        &office,
        None,
    )
    .await;
    assert_eq!(detail["tyres"].as_array().unwrap().len(), 2);

    let mut checkin = new("checkin");
    checkin.checkout_id = Some(checkout.id);
    let checkin = inspections::create(&pool, &checkin).await.unwrap();
    let damage = inspections::add_damage(
        &pool,
        &NewDamage {
            inspection_id: checkin.id,
            zone_key: "left_side".into(),
            damage_type: "scratch".into(),
            severity: "moderate".into(),
            note: None,
            x: None,
            y: None,
            view: "top".into(),
        },
    )
    .await
    .unwrap();

    let body = json!({
        "order_id": order.id, "damage_id": damage.id, "title": "Karc a bal oldalon",
        "cost_minor": 4_500_000, "responsible": "Fényező", "open_rework": true
    });
    let (status, incident) =
        call(&pool, "POST", "/api/incidents", &office, Some(body.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{incident}");
    assert_eq!(incident["inspection_id"], checkin.id);
    let rework_id = incident["rework_order_id"]
        .as_i64()
        .expect("a rework job was opened");
    let (_, rework) = call(
        &pool,
        "GET",
        &format!("/api/orders/{rework_id}"),
        &office,
        None,
    )
    .await;
    assert_eq!(rework["order"]["relation"], "rework");
    assert_eq!(rework["order"]["related_order_id"], order.id);

    // The second click opens the first.
    let (status, again) = call(&pool, "POST", "/api/incidents", &office, Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], incident["id"]);

    let id = incident["id"].as_i64().unwrap();
    let (_, resolved) = call(
        &pool,
        "PATCH",
        &format!("/api/incidents/{id}"),
        &office,
        Some(json!({ "status": "resolved", "resolution": "Kijavítva saját költségen" })),
    )
    .await;
    assert_eq!(resolved["status"], "resolved");
    assert!(resolved["resolved_at"].is_string());
    let (_, open) = call(&pool, "GET", "/api/incidents?status=open", &office, None).await;
    assert!(open["items"].as_array().unwrap().is_empty());
}
