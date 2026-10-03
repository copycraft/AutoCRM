mod common;

use autocrm::domain::media::ImageCategory;
use autocrm::domain::role::Role;
use autocrm::repo::images::{self, NewImage};
use autocrm::repo::inspections::{self, NewDamage, NewInspection};

fn draft(order_id: i64, kind: &str, user_id: i64, checkout_id: Option<i64>) -> NewInspection {
    NewInspection {
        order_id,
        kind: kind.to_string(),
        vehicle_plate: "ABC-123".to_string(),
        vehicle_vin: None,
        inspector_name: "Szerelő Sándor".to_string(),
        driver_name: Some("Vevő Viktor".to_string()),
        location: Some("Telephely".to_string()),
        odometer: Some(123_456),
        fuel_level: Some("1/2".to_string()),
        battery_pct: None,
        warning_lights: None,
        checkout_id,
        created_by: user_id,
    }
}

async fn image(pool: &sqlx::PgPool, order_id: i64, user_id: i64) -> i64 {
    let mut conn = pool.acquire().await.unwrap();
    images::insert(
        &mut conn,
        &NewImage {
            order_id,
            category: ImageCategory::Inspection,
            storage_key: "orders/1/inspection/abc.jpg",
            content_type: "image/jpeg",
            original_filename: Some("front.jpg"),
            content_hash: &[7; 32],
            byte_size: 42,
            uploaded_by: Some(user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap()
    .0
    .id
}

#[sqlx::test(migrations = "./migrations")]
async fn checkout_checkin_roundtrip_with_verdicts(pool: sqlx::PgPool) {
    let me = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &me, "HUF", vec![]).await;

    // Open check-out.
    let checkout = inspections::create(&pool, &draft(order.id, "checkout", me.user_id, None))
        .await
        .unwrap();
    assert_eq!(checkout.status, "draft");

    // A second draft check-out for the same order is refused.
    assert!(
        inspections::create(&pool, &draft(order.id, "checkout", me.user_id, None))
            .await
            .is_err()
    );

    // Damage + photo, then signatures and lock.
    let damage = inspections::add_damage(
        &pool,
        &NewDamage {
            inspection_id: checkout.id,
            zone_key: "front".to_string(),
            damage_type: "scratch".to_string(),
            severity: "minor".to_string(),
            note: None,
            x: Some(0.5),
            y: Some(0.25),
            view: "top".to_string(),
        },
    )
    .await
    .unwrap();
    let image_id = image(&pool, order.id, me.user_id).await;
    inspections::attach_photo(
        &pool,
        checkout.id,
        image_id,
        "front",
        "closeup",
        Some(damage.id),
        chrono::Utc::now(),
        None,
        None,
    )
    .await
    .unwrap();
    // The same image cannot attach twice.
    assert!(
        inspections::attach_photo(
            &pool,
            checkout.id,
            image_id,
            "front",
            "overview",
            None,
            chrono::Utc::now(),
            None,
            None,
        )
        .await
        .is_err()
    );
    inspections::sign(&pool, checkout.id, None).await.unwrap();

    // Locked: drafts can neither change nor be discarded.
    assert!(
        inspections::patch_draft(
            &pool,
            checkout.id,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(!inspections::remove_draft(&pool, checkout.id).await.unwrap());

    // Check-in auto-links the signed check-out.
    let linked = inspections::latest_signed_checkout(&pool, order.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(linked.id, checkout.id);
    let checkin = inspections::create(
        &pool,
        &draft(order.id, "checkin", me.user_id, Some(linked.id)),
    )
    .await
    .unwrap();

    // Same scratch again plus a genuinely new dent.
    let same = inspections::add_damage(
        &pool,
        &NewDamage {
            inspection_id: checkin.id,
            zone_key: "front".to_string(),
            damage_type: "scratch".to_string(),
            severity: "minor".to_string(),
            note: None,
            x: None,
            y: None,
            view: "top".to_string(),
        },
    )
    .await
    .unwrap();
    let fresh = inspections::add_damage(
        &pool,
        &NewDamage {
            inspection_id: checkin.id,
            zone_key: "rear".to_string(),
            damage_type: "dent".to_string(),
            severity: "moderate".to_string(),
            note: None,
            x: None,
            y: None,
            view: "top".to_string(),
        },
    )
    .await
    .unwrap();
    inspections::set_verdict(
        &pool,
        checkin.id,
        same.id,
        Some(damage.id),
        "preexisting",
        None,
        me.user_id,
    )
    .await
    .unwrap();
    inspections::set_verdict(&pool, checkin.id, fresh.id, None, "new", None, me.user_id)
        .await
        .unwrap();
    let verdicts = inspections::verdicts_for(&pool, checkin.id).await.unwrap();
    assert_eq!(verdicts.len(), 2);

    // Follow-up notes live on after the lock.
    inspections::add_note(&pool, checkin.id, "Ügyfél vitatja a horpadást.", me.user_id)
        .await
        .unwrap();
    assert_eq!(
        inspections::notes_for(&pool, checkin.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

// ── Zone lists per vehicle kind and walkaround kind ──────────────────────────────

use autocrm::repo::inspections::{NewZone, ZoneTemplate};

async fn list(pool: &sqlx::PgPool, project_type: Option<&str>, kind: &str) -> Vec<ZoneTemplate> {
    let mut conn = pool.acquire().await.unwrap();
    let id = match project_type {
        Some(key) => Some(project_type_id(pool, key).await),
        None => None,
    };
    inspections::templates_for(&mut conn, id, kind)
        .await
        .unwrap()
}

async fn project_type_id(pool: &sqlx::PgPool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM project_types WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

fn keys(zones: &[ZoneTemplate]) -> Vec<&str> {
    zones.iter().map(|z| z.zone_key.as_str()).collect()
}

fn zone(key: &str, position: i32, optional: bool) -> NewZone {
    NewZone {
        zone_key: key.into(),
        position,
        title: key.into(),
        instruction: format!("Fotó: {key}"),
        optional,
        required: !optional,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn the_general_list_serves_both_walkarounds_and_every_type_without_its_own(
    pool: sqlx::PgPool,
) {
    for kind in ["checkout", "checkin"] {
        let general = list(&pool, None, kind).await;
        assert_eq!(general.len(), 15, "{kind}");
        assert!(general.iter().any(|z| z.zone_key == "front" && !z.optional));
        assert!(general.iter().any(|z| z.zone_key == "roof" && z.optional));
        assert!(general.iter().all(|z| z.project_type_id.is_none()));
        assert!(general.iter().all(|z| !z.title.is_empty()));
        // Project types with no list of their own are served the general one.
        for key in ["heated_body", "repair", "other"] {
            assert_eq!(
                keys(&list(&pool, Some(key), kind).await),
                keys(&general),
                "{key} {kind}"
            );
        }
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn a_converted_van_keeps_the_general_zones_plus_the_cargo_extras(pool: sqlx::PgPool) {
    // What the old `default` + `cooling` lookup gave refrigerated project types, now stored
    // as the project type's own list.
    for key in ["van_conversion", "unit_install"] {
        for kind in ["checkout", "checkin"] {
            let zones = list(&pool, Some(key), kind).await;
            assert_eq!(zones.len(), 18, "{key} {kind}");
            assert!(zones.iter().all(|z| z.project_type_id.is_some()));
            for extra in ["cargo_box", "cargo_doors", "refrigeration_unit"] {
                assert!(keys(&zones).contains(&extra), "{key} {kind} {extra}");
            }
        }
    }
}

/// Every photo of the example set of a finished alváz-with-box (Elkészült Meo.zip),
/// in the order they were taken.
const ALVAZ_BOX_OUTGO: [&str; 38] = [
    "type_plate",
    "engine_bay",
    "cab_reefer_display",
    "cab_driver_side",
    "door_right_inner",
    "cab_passenger_side",
    "door_left_inner",
    "box_side_door",
    "box_side_threshold",
    "box_interior_side",
    "box_interior_rear",
    "reefer_unit_inside",
    "box_interior_full",
    "wheelhouse_1",
    "wheelhouse_2",
    "rear_doors_open",
    "rear_door_1",
    "rear_door_2",
    "rear_hinge",
    "roof_corner_1",
    "roof_corner_2",
    "reefer_unit_roof",
    "front_right",
    "mirror_right",
    "shore_power",
    "wheel_rear_1",
    "rear_lights_1",
    "rear_right",
    "rear",
    "rear_left",
    "rear_lights_2",
    "wheel_rear_2",
    "chassis_side",
    "left_side",
    "mirror_left",
    "headlight_bumper",
    "front_left",
    "vin_windshield",
];

#[sqlx::test(migrations = "./migrations")]
async fn an_alvaz_with_box_leaves_with_every_photo_of_the_example_set(pool: sqlx::PgPool) {
    let outgo = list(&pool, Some("refrigerated_body"), "checkin").await;
    // The 38 photos, in their order, all required; plus an optional odometer shot that is
    // not part of the example.
    let example: Vec<&str> = keys(&outgo)
        .into_iter()
        .filter(|k| *k != "interior_dashboard")
        .collect();
    assert_eq!(example, ALVAZ_BOX_OUTGO);
    let owner = project_type_id(&pool, "refrigerated_body").await;
    for z in &outgo {
        assert_eq!(
            z.optional,
            z.zone_key == "interior_dashboard",
            "{}",
            z.zone_key
        );
        assert_eq!(z.required, !z.optional, "{}", z.zone_key);
        assert_eq!(z.project_type_id, Some(owner));
    }
    // Positions are the order: distinct and increasing.
    assert!(outgo.windows(2).all(|w| w[0].position < w[1].position));
}

#[sqlx::test(migrations = "./migrations")]
async fn an_alvaz_arrives_as_a_bare_cab_and_every_intake_zone_can_be_compared_at_outgo(
    pool: sqlx::PgPool,
) {
    let intake = list(&pool, Some("refrigerated_body"), "checkout").await;
    let outgo = list(&pool, Some("refrigerated_body"), "checkin").await;

    // The box does not exist yet: nothing about it is asked for at intake.
    for box_zone in [
        "box_side_door",
        "box_interior_side",
        "box_interior_rear",
        "reefer_unit_inside",
        "reefer_unit_roof",
        "rear_doors_open",
        "roof_corner_1",
        "shore_power",
        "cab_reefer_display",
    ] {
        assert!(!keys(&intake).contains(&box_zone), "{box_zone}");
    }
    for cab_zone in [
        "type_plate",
        "vin_windshield",
        "engine_bay",
        "cab_driver_side",
        "front_left",
    ] {
        assert!(keys(&intake).contains(&cab_zone), "{cab_zone}");
    }
    // A damage is compared by zone key between the two walkarounds, so an intake zone with
    // no outgo twin would never be checked for new damage.
    for z in &intake {
        assert!(
            keys(&outgo).contains(&z.zone_key.as_str()),
            "intake zone {} has no outgo zone to compare with",
            z.zone_key
        );
    }
    assert!(outgo.len() > intake.len());
}

#[sqlx::test(migrations = "./migrations")]
async fn editing_one_list_leaves_the_others_alone_and_removing_it_falls_back(pool: sqlx::PgPool) {
    let heated = project_type_id(&pool, "heated_body").await;
    let general_before: Vec<String> = keys(&list(&pool, None, "checkin").await)
        .into_iter()
        .map(String::from)
        .collect();

    let mut tx = pool.begin().await.unwrap();
    let saved = inspections::replace_template_list(
        &mut tx,
        Some(heated),
        "checkin",
        &[zone("rear", 1, false), zone("front", 2, true)],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(keys(&saved), ["rear", "front"]);
    assert_eq!(saved[0].set_key, "heated_body:checkin");

    // Its own list wins for that walkaround only.
    assert_eq!(
        keys(&list(&pool, Some("heated_body"), "checkin").await),
        ["rear", "front"]
    );
    assert_eq!(list(&pool, Some("heated_body"), "checkout").await.len(), 15);
    // Nobody else moved.
    let general: Vec<String> = keys(&list(&pool, None, "checkin").await)
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(general, general_before);
    assert_eq!(
        list(&pool, Some("refrigerated_body"), "checkin")
            .await
            .len(),
        39
    );

    // Replacing again replaces, it does not append.
    let mut tx = pool.begin().await.unwrap();
    inspections::replace_template_list(&mut tx, Some(heated), "checkin", &[zone("roof", 1, true)])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        keys(&list(&pool, Some("heated_body"), "checkin").await),
        ["roof"]
    );

    // Removing it serves the general list again; removing it twice finds nothing.
    let mut tx = pool.begin().await.unwrap();
    assert!(
        inspections::delete_template_list(&mut tx, Some(heated), "checkin")
            .await
            .unwrap()
    );
    assert!(
        !inspections::delete_template_list(&mut tx, Some(heated), "checkin")
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(list(&pool, Some("heated_body"), "checkin").await.len(), 15);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_list_cannot_hold_the_same_zone_twice(pool: sqlx::PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let twice = inspections::replace_template_list(
        &mut tx,
        None,
        "checkin",
        &[zone("front", 1, false), zone("front", 2, false)],
    )
    .await;
    assert!(
        twice.is_err(),
        "a duplicate zone key in one list must be refused"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn every_zone_is_either_required_or_optional(pool: sqlx::PgPool) {
    // The seeds agree (the roof used to be optional and required at once)...
    let disagreeing: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM inspection_zone_templates WHERE required = optional",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(disagreeing, 0);
    let roof: bool = sqlx::query_scalar(
        "SELECT required FROM inspection_zone_templates WHERE zone_key = 'roof' AND project_type_id IS NULL AND kind = 'checkin'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!roof, "the optional roof is not also required");

    // ...and the database refuses a row that does not.
    let both = sqlx::query(
        "INSERT INTO inspection_zone_templates (kind, zone_key, position, title, instruction, optional, required)
         VALUES ('checkin', 'x', 99, 'X', 'X', true, true)",
    )
    .execute(&pool)
    .await;
    assert!(both.is_err());
}
