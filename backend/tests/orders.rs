//! Order invariants that live in the database: stage gates, numbering, currency coupling,
//! immutable evidence, and agreement between the Money type and the reporting views.

mod common;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use autocrm::domain::media::ImageCategory;
use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::images::{self, NewImage};
use autocrm::repo::{order_items, orders, stages};
use autocrm::service::orders::{NewItem, create_in_tx, items_total, order_currency};
use autocrm::service::stages::change_order_stage;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

async fn add_image(
    pool: &PgPool,
    order_id: i64,
    category: ImageCategory,
    seed: u8,
) -> images::Image {
    let hash = [seed; 32];
    let mut conn = pool.acquire().await.unwrap();
    images::insert(
        &mut conn,
        &NewImage {
            order_id,
            category,
            storage_key: &format!("orders/{order_id}/{}/{seed}.jpg", category.as_str()),
            content_type: "image/jpeg",
            original_filename: None,
            content_hash: &hash,
            byte_size: 1234,
            uploaded_by: None,
            source_ref: None,
        },
    )
    .await
    .unwrap()
    .0
}

#[sqlx::test(migrations = "./migrations")]
async fn meo_gate_requires_completion_photos(pool: PgPool) {
    let state = common::state(pool.clone());
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    for stage in ["design", "production", "meo"] {
        change_order_stage(&state, &user, order.id, stage, None)
            .await
            .unwrap();
    }
    let blocked = change_order_stage(&state, &user, order.id, "completed", None).await;
    assert!(
        matches!(
            blocked,
            Err(AppError::Rule {
                code: "stage_gate",
                ..
            })
        ),
        "{blocked:?}"
    );

    add_image(&pool, order.id, ImageCategory::Production, 1).await;
    assert!(
        change_order_stage(&state, &user, order.id, "completed", None)
            .await
            .is_err()
    );

    add_image(&pool, order.id, ImageCategory::Completion, 2).await;
    change_order_stage(&state, &user, order.id, "completed", None)
        .await
        .unwrap();

    let history: Vec<String> = stages::order_history(&pool, order.id)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.stage_key)
        .collect();
    assert_eq!(
        history,
        ["intake", "design", "production", "meo", "completed"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn order_numbers_are_sequential_per_year(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let partner_id = common::partner(&pool, "Seq Kft", None).await;
    let mut numbers = Vec::new();
    for (year, _) in [(2026, 1), (2026, 2), (2027, 3), (2026, 4)] {
        let mut tx = pool.begin().await.unwrap();
        let today = NaiveDate::from_ymd_opt(year, 3, 1).unwrap();
        let o = create_in_tx(
            &mut tx,
            user.user_id,
            today,
            common::fields(partner_id, "HUF"),
            vec![],
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        numbers.push(o.number);
    }
    assert_eq!(
        numbers,
        ["2026-0001", "2026-0002", "2027-0001", "2026-0003"]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn line_items_cannot_use_another_currency(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "EUR", vec![]).await;
    let err = order_items::insert(&pool, order.id, 10, "Rossz deviza", dec("1"), 100, "HUF")
        .await
        .unwrap_err();
    let code = err
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.to_string());
    assert_eq!(code.as_deref(), Some("23503"), "{err}");
}

#[sqlx::test(migrations = "./migrations")]
async fn currency_cannot_change_under_existing_items(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(
        &pool,
        &user,
        "EUR",
        vec![NewItem {
            description: "Hűtőgép".into(),
            quantity: dec("1"),
            unit_price: 100,
        }],
    )
    .await;
    let mut fields = autocrm::repo::orders::OrderFields::from(&order);
    fields.currency = "HUF".into();
    assert!(orders::update(&pool, order.id, &fields).await.is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn intake_images_are_immutable_in_the_database(pool: PgPool) {
    let user = common::user(&pool, Role::Admin).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let intake = add_image(&pool, order.id, ImageCategory::Intake, 9).await;
    assert!(intake.immutable);

    let soft = images::soft_delete(&pool, intake.id, user.user_id)
        .await
        .unwrap_err();
    assert_eq!(
        soft.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("AC001")
    );

    let hard = sqlx::query("DELETE FROM images WHERE id = $1")
        .bind(intake.id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        hard.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("AC001")
    );

    let swap = sqlx::query("UPDATE images SET content_hash = $2 WHERE id = $1")
        .bind(intake.id)
        .bind(vec![0u8; 32])
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        swap.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("AC001")
    );

    // Derived copies can still be attached to an immutable original.
    sqlx::query("UPDATE images SET thumb_key = 'x', processed_at = now() WHERE id = $1")
        .bind(intake.id)
        .execute(&pool)
        .await
        .unwrap();

    // Non-evidence images can be removed.
    let marketing = add_image(&pool, order.id, ImageCategory::Marketing, 10).await;
    assert!(
        images::soft_delete(&pool, marketing.id, user.user_id)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn same_photo_twice_is_one_image(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let first = add_image(&pool, order.id, ImageCategory::Production, 5).await;
    let second = add_image(&pool, order.id, ImageCategory::Production, 5).await;
    assert_eq!(first.id, second.id);
}

#[sqlx::test(migrations = "./migrations")]
async fn reporting_view_agrees_with_money_type(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let items = vec![
        NewItem {
            description: "a".into(),
            quantity: dec("2.5"),
            unit_price: 333,
        }, // 832.5 → 833
        NewItem {
            description: "b".into(),
            quantity: dec("0.125"),
            unit_price: 1999,
        }, // 249.875 → 250
        NewItem {
            description: "c".into(),
            quantity: dec("1"),
            unit_price: -500,
        }, // discount
        NewItem {
            description: "d".into(),
            quantity: dec("0.5"),
            unit_price: -3,
        }, // -1.5 → -2
    ];
    let order = common::order(&pool, &user, "EUR", items).await;
    let rows = order_items::list(&pool, order.id).await.unwrap();
    let money = items_total(order_currency(&order).unwrap(), &rows).unwrap();
    let view = orders::value(&pool, order.id).await.unwrap().unwrap();
    assert_eq!(money.minor(), 833 + 250 - 500 - 2);
    assert_eq!(view.total_minor, money.minor());

    // No rate yet → normalised value is null, not silently converted at some other rate.
    assert_eq!(view.total_huf_minor, None);
    sqlx::query(
        "INSERT INTO fx_rates (day, base, quote, rate) VALUES ('2026-09-07', 'EUR', 'HUF', 362.10)",
    )
    .execute(&pool)
    .await
    .unwrap();
    // Valuation date 2026-09-10 has no rate of its own; the latest earlier one applies.
    let view = orders::value(&pool, order.id).await.unwrap().unwrap();
    assert_eq!(view.fx_day, NaiveDate::from_ymd_opt(2026, 9, 7));
    assert_eq!(
        view.total_huf_minor,
        Some(
            (Decimal::from(581) * dec("362.10"))
                .round()
                .try_into()
                .unwrap()
        )
    );
}
