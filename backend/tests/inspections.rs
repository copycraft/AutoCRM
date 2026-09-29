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

#[sqlx::test(migrations = "./migrations")]
async fn zone_templates_seed_default_and_cooling(pool: sqlx::PgPool) {
    let zones = inspections::templates_for_sets(&pool, &["default".to_string()])
        .await
        .unwrap();
    assert_eq!(zones.len(), 15);
    assert!(zones.iter().any(|z| z.zone_key == "front" && !z.optional));
    assert!(zones.iter().any(|z| z.zone_key == "roof" && z.optional));

    let cooling = inspections::templates_for_sets(&pool, &["cooling".to_string()])
        .await
        .unwrap();
    assert_eq!(cooling.len(), 3);
    assert!(cooling.iter().any(|z| z.zone_key == "refrigeration_unit"));
}
