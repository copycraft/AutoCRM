//! Order invariants that live in the database: stage gates, numbering, currency coupling,
//! immutable evidence, and agreement between the Money type and the reporting views.

mod common;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use autocrm::domain::media::{DocumentKind, ImageCategory};
use autocrm::domain::role::Role;
use autocrm::error::AppError;
use autocrm::repo::images::{self, NewImage};
use autocrm::repo::{documents, order_items, order_specs, orders, partners, stages, vehicles};
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

    // Leaving `intake` needs the slip (0018); this test is about the MEO gate, so get
    // past the earlier one rather than assert it here.
    let mut f = orders::OrderFields::from(&order);
    f.mileage_in = Some(120_000);
    orders::update(&pool, order.id, &f).await.unwrap().unwrap();

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

fn priced(description: &str, unit_price: i64) -> NewItem {
    NewItem {
        description: description.into(),
        quantity: dec("1"),
        unit_price,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn search_sorting_orders_results(pool: PgPool) {
    use autocrm::repo::orders::OrderFilter;

    let user = common::user(&pool, Role::Office).await;
    let a = common::order(&pool, &user, "HUF", vec![priced("a", 1000)]).await;
    let b = common::order(&pool, &user, "HUF", vec![priced("b", 3000)]).await;
    let c = common::order(&pool, &user, "HUF", vec![priced("c", 2000)]).await;
    let filter = OrderFilter::default();

    let ids = |rows: Vec<autocrm::repo::orders::OrderSummary>| {
        rows.into_iter().map(|r| r.id).collect::<Vec<_>>()
    };

    // Default: newest first.
    let rows = orders::search(&pool, &filter, "-created_at", 10, 0)
        .await
        .unwrap();
    assert_eq!(ids(rows), vec![c.id, b.id, a.id]);

    // Totals both directions.
    let rows = orders::search(&pool, &filter, "total", 10, 0)
        .await
        .unwrap();
    assert_eq!(ids(rows), vec![a.id, c.id, b.id]);
    let rows = orders::search(&pool, &filter, "-total", 10, 0)
        .await
        .unwrap();
    assert_eq!(ids(rows), vec![b.id, c.id, a.id]);

    // Numbers follow allocation order.
    let rows = orders::search(&pool, &filter, "number", 10, 0)
        .await
        .unwrap();
    assert_eq!(ids(rows), vec![a.id, b.id, c.id]);
}

/// V2.1: the vehicle becomes a real row the moment an order names a plate, and the same
/// van coming back years later matches the existing record rather than duplicating it.
#[sqlx::test(migrations = "./migrations")]
async fn a_plate_on_an_order_becomes_a_vehicle(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let first = common::order(&pool, &user, "HUF", vec![]).await;

    let vehicles = vehicles::list_for_order(&pool, first.id).await.unwrap();
    assert_eq!(vehicles.len(), 1);
    assert_eq!(vehicles[0].plate.as_deref(), Some("ABC-123"));
    assert_eq!(vehicles[0].plate_norm.as_deref(), Some("ABC123"));
    assert_eq!(vehicles[0].make.as_deref(), Some("Iveco"));

    // The same van, plate written differently, on a later job.
    let partner_id = common::partner(&pool, "Második ügyfél", None).await;
    let mut fields = common::fields(partner_id, "HUF");
    fields.vehicle_plate = Some("abc 123".into());
    fields.vehicle_vin = Some("WDB9066571S123456".into());
    let mut tx = pool.begin().await.unwrap();
    let second = create_in_tx(
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

    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM vehicles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 1, "one van, however the plate was typed");
    let history = vehicles::orders_for_vehicle(&pool, vehicles[0].id)
        .await
        .unwrap();
    assert_eq!(history.len(), 2, "both jobs hang off the same vehicle");
    // The VIN the second job supplied fills in a blank without overwriting anything.
    let v = vehicles::find(&pool, vehicles[0].id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(v.vin.as_deref(), Some("WDB9066571S123456"));
    assert!(history.iter().any(|(id, _, _)| *id == second.id));
}

/// V2.1: a plate search has to reach the vehicle, not only the order's own text column —
/// that column is empty on every migrated order until the mapping fills it in.
#[sqlx::test(migrations = "./migrations")]
async fn plate_search_matches_through_the_vehicle(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    // Clear the order's own columns: the vehicle row is now the only place the plate lives.
    sqlx::query("UPDATE orders SET vehicle_plate = NULL, vehicle_vin = NULL WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await
        .unwrap();

    let filter = orders::OrderFilter {
        pattern: Some("%abc123%".into()),
        plate_pattern: "%ABC123%".into(),
        ..Default::default()
    };
    let found = orders::search(&pool, &filter, orders::DEFAULT_SORT, 20, 0)
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, order.id);
}

/// V2.7: three identical Sprinters from one enquiry are three orders, and all three keep
/// their origin. Before this the second conversion was refused and the other two vans
/// became orphan orders with no record of where they came from.
#[sqlx::test(migrations = "./migrations")]
async fn a_lead_converts_as_many_times_as_it_has_vehicles(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let partner_id = common::partner(&pool, "Müller Kühltransporte GmbH", None).await;
    let lead = autocrm::service::leads::create(
        &pool,
        &user,
        autocrm::repo::leads::LeadInput {
            title: "Három Sprinter".into(),
            partner_id: Some(partner_id),
            contact_id: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            source: None,
            description: None,
            assigned_to: None,
            quoted_value_minor: Some(4_500_000),
            currency: Some("EUR".into()),
            quote_valid_until: NaiveDate::from_ymd_opt(2026, 12, 31),
        },
        &[],
    )
    .await
    .unwrap();

    let today = NaiveDate::from_ymd_opt(2026, 9, 10).unwrap();
    let mut numbers = Vec::new();
    for plate in ["AAA-111", "BBB-222", "CCC-333"] {
        let mut fields = common::fields(partner_id, "EUR");
        fields.vehicle_plate = Some(plate.into());
        let order = autocrm::service::leads::convert(
            &pool,
            &user,
            lead.id,
            today,
            autocrm::service::leads::Conversion {
                partner_id: Some(partner_id),
                fields,
                items: vec![],
            },
        )
        .await
        .unwrap();
        numbers.push(order.number);
    }
    assert_eq!(numbers.len(), 3);

    let from_lead = orders::find_by_lead(&pool, lead.id).await.unwrap();
    assert_eq!(from_lead.len(), 3, "every order keeps its origin");
    let vehicles_total: i64 = sqlx::query_scalar("SELECT count(*) FROM vehicles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(vehicles_total, 3, "three vans, not one");
}

/// V2.2: the relation and its target are set together, and never point at the order itself.
#[sqlx::test(migrations = "./migrations")]
async fn a_warranty_job_points_at_the_job_it_repairs(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let original = common::order(&pool, &user, "HUF", vec![]).await;

    let mut fields = common::fields(original.partner_id, "HUF");
    fields.related_order_id = Some(original.id);
    fields.relation = Some("warranty".into());
    let mut tx = pool.begin().await.unwrap();
    let warranty = create_in_tx(
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
    assert_eq!(warranty.related_order_id, Some(original.id));
    assert_eq!(warranty.relation.as_deref(), Some("warranty"));

    // A target with no relation says nothing, and is refused before it reaches the database.
    let mut half = common::fields(original.partner_id, "HUF");
    half.related_order_id = Some(original.id);
    let mut tx = pool.begin().await.unwrap();
    let error = create_in_tx(
        &mut tx,
        user.user_id,
        NaiveDate::from_ymd_opt(2026, 9, 10).unwrap(),
        half,
        vec![],
        None,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, AppError::Validation(m) if m.contains("set together")),
        "{error:?}"
    );
}

/// V2.4: a quotation PDF hangs off the lead. `documents.order_id` used to be NOT NULL,
/// which is why a quotation could not be filed or emailed from this system at all.
#[sqlx::test(migrations = "./migrations")]
async fn a_document_can_belong_to_a_lead(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let lead = autocrm::service::leads::create(
        &pool,
        &user,
        autocrm::repo::leads::LeadInput {
            title: "Árajánlat".into(),
            partner_id: None,
            contact_id: None,
            contact_name: None,
            contact_email: None,
            contact_phone: None,
            source: None,
            description: None,
            assigned_to: None,
            quoted_value_minor: None,
            currency: None,
            quote_valid_until: None,
        },
        &[],
    )
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let (doc, created) = documents::insert(
        &mut conn,
        &documents::NewDocument {
            owner: documents::Owner::Lead(lead.id),
            vehicle_id: None,
            kind: DocumentKind::Other,
            filename: "arajanlat.pdf",
            content_type: "application/pdf",
            storage_key: &format!("leads/{}/documents/ab.pdf", lead.id),
            content_hash: &[9u8; 32],
            byte_size: 2048,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    assert!(created);
    assert_eq!(doc.lead_id, Some(lead.id));
    assert_eq!(doc.order_id, None);
    assert_eq!(
        documents::list_for_lead(&pool, lead.id)
            .await
            .unwrap()
            .len(),
        1
    );

    // Exactly one owner, enforced by the database rather than by a handler.
    let bad = sqlx::query(
        "INSERT INTO documents (order_id, lead_id, kind, filename, content_type, storage_key,
                                content_hash, byte_size)
         VALUES (NULL, NULL, 'other', 'x.pdf', 'application/pdf', 'k', $1, 1)",
    )
    .bind(&[1u8; 32][..])
    .execute(&pool)
    .await;
    assert!(bad.is_err(), "a document with no owner must be refused");
}

/// V2.5: "which ATP certificates expire next quarter" is a cross-order question, and had
/// no answer at any layer before this.
#[sqlx::test(migrations = "./migrations")]
async fn certificates_can_be_found_by_expiry_across_orders(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;
    let mut conn = pool.acquire().await.unwrap();
    let (doc, _) = documents::insert(
        &mut conn,
        &documents::NewDocument {
            owner: documents::Owner::Order(order.id),
            vehicle_id: None,
            kind: DocumentKind::Certificate,
            filename: "atp.pdf",
            content_type: "application/pdf",
            storage_key: "orders/1/documents/atp.pdf",
            content_hash: &[3u8; 32],
            byte_size: 1024,
            uploaded_by: Some(user.user_id),
            source_ref: None,
        },
    )
    .await
    .unwrap();
    documents::set_validity(
        &pool,
        doc.id,
        &documents::Validity {
            issuer: Some("NKH".into()),
            valid_from: NaiveDate::from_ymd_opt(2020, 1, 1),
            valid_until: NaiveDate::from_ymd_opt(2026, 12, 1),
        },
        None,
    )
    .await
    .unwrap();

    let expiring = documents::search(
        &pool,
        Some(DocumentKind::Certificate),
        NaiveDate::from_ymd_opt(2026, 12, 31),
        None,
        50,
        0,
    )
    .await
    .unwrap();
    assert_eq!(expiring.len(), 1);
    assert_eq!(expiring[0].issuer.as_deref(), Some("NKH"));

    let not_yet = documents::search(
        &pool,
        Some(DocumentKind::Certificate),
        NaiveDate::from_ymd_opt(2026, 6, 30),
        None,
        50,
        0,
    )
    .await
    .unwrap();
    assert!(not_yet.is_empty());
}

/// V2.6: the paint shop stops appearing in the customer picker, and a partner nobody has
/// classified yet still counts as a customer.
#[sqlx::test(migrations = "./migrations")]
async fn the_supplier_filter_keeps_customers_visible(pool: PgPool) {
    let unclassified = common::partner(&pool, "Régi ügyfél", None).await;
    let supplier = partners::insert(
        &pool,
        &partners::PartnerInput {
            kind: autocrm::domain::partner::PartnerKind::Business,
            name: "Fényező Kft.".into(),
            tax_number: None,
            eu_tax_number: None,
            country: "HU".into(),
            default_currency: "HUF".into(),
            email: None,
            phone: None,
            website: None,
            postal_code: None,
            city: None,
            address_line: None,
            notes: None,
            role: Some("supplier".into()),
        },
    )
    .await
    .unwrap();

    let customers = partners::search(
        &pool,
        None,
        None,
        None,
        Some("customer"),
        false,
        "name",
        50,
        0,
    )
    .await
    .unwrap();
    assert!(customers.iter().any(|p| p.id == unclassified));
    assert!(
        !customers.iter().any(|p| p.id == supplier.id),
        "a supplier is not a customer"
    );

    let suppliers = partners::search(
        &pool,
        None,
        None,
        None,
        Some("supplier"),
        false,
        "name",
        50,
        0,
    )
    .await
    .unwrap();
    assert_eq!(suppliers.len(), 1);
    assert_eq!(suppliers[0].id, supplier.id);
}

/// The build spec follows the project type, not the request: a cooling order cannot be
/// given heater fields, and a project type with no spec form has no spec at all.
#[sqlx::test(migrations = "./migrations")]
async fn the_spec_form_comes_from_the_project_type(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    let cooling_type: i64 =
        sqlx::query_scalar("SELECT id FROM project_types WHERE key = 'refrigerated_body'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let repair_type: i64 = sqlx::query_scalar("SELECT id FROM project_types WHERE key = 'repair'")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(
        order_specs::form_for_project_type(&pool, cooling_type)
            .await
            .unwrap()
            .as_deref(),
        Some("cooling")
    );
    assert_eq!(
        order_specs::form_for_project_type(&pool, repair_type)
            .await
            .unwrap(),
        None,
        "a repair has no build spec section"
    );

    // Heater fields sent on a cooling order are dropped, not written: `for_form` blanks
    // them, and the CHECK constraint would refuse the row if it did not.
    let fields = order_specs::SpecFields {
        form: "cooling".into(),
        target_temp_c: Some(dec("-18.0")),
        insulation_mm: Some(80),
        cooling_unit_make: Some("Carrier".into()),
        atp_class: Some("FRC".into()),
        heater_make: Some("Webasto".into()),
        heat_output_kw: Some(dec("5.0")),
        ..Default::default()
    }
    .for_form();
    assert_eq!(fields.heater_make, None);
    assert_eq!(fields.heat_output_kw, None);

    let written = order_specs::upsert(&pool, order.id, &fields).await.unwrap();
    assert_eq!(written.form, "cooling");
    assert_eq!(written.target_temp_c, Some(dec("-18.0")));
    assert_eq!(written.atp_class.as_deref(), Some("FRC"));
    assert_eq!(written.heater_make, None);

    // Switching the order to a heating type replaces the spec cleanly rather than leaving
    // a row that means two things at once.
    let heating = order_specs::SpecFields {
        form: "heating".into(),
        target_temp_c: Some(dec("20.0")),
        heater_make: Some("Webasto".into()),
        heat_output_kw: Some(dec("5.0")),
        fuel: Some("diesel".into()),
        thermostat: Some(true),
        cooling_unit_make: Some("Carrier".into()),
        ..Default::default()
    }
    .for_form();
    let written = order_specs::upsert(&pool, order.id, &heating)
        .await
        .unwrap();
    assert_eq!(written.form, "heating");
    assert_eq!(written.cooling_unit_make, None);
    assert_eq!(written.heater_make.as_deref(), Some("Webasto"));

    // The database refuses a mismatched row even if a future code path builds one.
    let refused =
        sqlx::query("UPDATE order_specs SET cooling_unit_make = 'Carrier' WHERE order_id = $1")
            .bind(order.id)
            .execute(&pool)
            .await;
    assert!(
        refused.is_err(),
        "a heating spec must not hold cooling fields"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn intake_extras_round_trip_and_valuables_stay_paired(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![]).await;

    // The three extras are optional, so an order arrives with none of them recorded —
    // which is not the same as an order recorded as having nothing in it.
    assert_eq!(order.fuel_level, None);
    assert_eq!(order.key_count, None);
    assert_eq!(order.valuables_declared, None);

    let mut f = orders::OrderFields::from(&order);
    f.mileage_in = Some(182_450);
    f.fuel_level = Some("3/4".into());
    f.key_count = Some(2);
    f.valuables_declared = Some(true);
    f.valuables = Some("Navigáció a kesztyűtartóban".into());
    let saved = orders::update(&pool, order.id, &f).await.unwrap().unwrap();
    assert_eq!(saved.fuel_level.as_deref(), Some("3/4"));
    assert_eq!(saved.key_count, Some(2));
    assert_eq!(saved.valuables_declared, Some(true));

    // "Asked, and the car was empty" is a recorded answer with no description.
    f.valuables_declared = Some(false);
    f.valuables = None;
    let emptied = orders::update(&pool, order.id, &f).await.unwrap().unwrap();
    assert_eq!(emptied.valuables_declared, Some(false));
    assert_eq!(emptied.valuables, None);

    // A gauge reading the gauge does not have.
    let bad_fuel = sqlx::query("UPDATE orders SET fuel_level = 'fél' WHERE id = $1")
        .bind(order.id)
        .execute(&pool)
        .await;
    assert!(
        bad_fuel.is_err(),
        "fuel_level is limited to the gauge marks"
    );

    // A description with nobody having ticked the box: the pairing is the database's,
    // not just the handler's, so a future code path cannot write a dangling list.
    let dangling = sqlx::query(
        "UPDATE orders SET valuables = 'laptop', valuables_declared = false WHERE id = $1",
    )
    .bind(order.id)
    .execute(&pool)
    .await;
    assert!(
        dangling.is_err(),
        "a valuables description must carry its declaration"
    );
}
