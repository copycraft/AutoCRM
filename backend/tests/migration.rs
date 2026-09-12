//! MiniCRM load + reconcile against fixture data shaped like the R3 API output.

mod common;

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde_json::{Value, json};
use sqlx::PgPool;

use autocrm::migration::load::{self, Mapping};
use autocrm::migration::reconcile;

fn write(path: PathBuf, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn fixture() -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("autocrm-migration-test-{}", rand::random::<u64>()));
    let raw = dir.join("raw");
    write(
        raw.join("contacts/100.json"),
        &json!({"Id": 100, "Type": "Business", "Name": "Müller Kühlung GmbH", "Email": "info@mueller.example",
                "BusinessId": 0, "EuVatNumber": "DE123456789", "CountryId": "Deutschland", "Penznem": "EUR",
                "Comment": "Nagy ügyfél"}),
    );
    // AddressList: downloaded by extract.rs since day one, read by nothing until V1.2.
    write(
        raw.join("addresses/100.json"),
        &json!({"7001": {"Id": 7001, "Type": "Számlázási", "PostalCode": "80331", "City": "München",
                         "Address": "Bahnhofstrasse 1"}}),
    );
    write(
        raw.join("contacts/101.json"),
        &json!({"Id": 101, "Type": "Person", "FirstName": "Michael", "LastName": "Müller", "Email": "michael@mueller.example", "BusinessId": 100}),
    );
    write(
        raw.join("contacts/102.json"),
        &json!({"Id": 102, "Type": "Person", "Name": "Kovács János", "Email": "nem-email", "BusinessId": ""}),
    );
    write(
        raw.join("projects/500.json"),
        &json!({
            "Id": 500, "Name": "Iveco hűtős felépítmény (ABC-123)", "CategoryId": 12, "ContactId": 101, "StatusId": 3003,
            "CreatedAt": "2019-05-10 12:33:00", "Osszeg": "1 234 567,50", "Egyedi_mezo": "megmarad",
            "Sorszam": "2019/0042",
            "Jarmu": {"Marka": "Iveco", "Tipus": "Daily", "Alvazszam": "wjmm1vnt0 0c123456"},
            "Leiras": "Hűtős felépítmény gyártása.", "Hatarido": "2019-08-01", "Projekttipus": "Hutofelepitmeny",
            "Felelos": "77",
            "Atveteli_fotok": ["https://r3.minicrm.hu/files/500/IMG_1.jpg"]
        }),
    );
    // The project history that had no destination table until V1.3.
    write(
        raw.join("todos/500.json"),
        &json!({"Results": {
            "9001": {"Id": 9001, "Comment": "Ügyfél jóváhagyta a tervet.", "UserName": "Nagy Béla",
                     "CreatedAt": "2019-05-20 09:15:00"},
            "9002": {"Id": 9002, "Comment": "Fényezés alvállalkozónál.", "UserName": "Nagy Béla",
                     "CreatedAt": "2019-06-02 14:00:00"}
        }}),
    );
    write(
        raw.join("projects/501.json"),
        &json!({"Id": 501, "Name": "Érdeklődés", "CategoryId": 15, "ContactId": 102, "StatusId": 2002}),
    );
    write(
        raw.join("projects/502.json"),
        &json!({"Id": 502, "Name": "Ismeretlen kategória", "CategoryId": 99, "ContactId": 100}),
    );
    write(
        raw.join("projects/503.json"),
        &json!({"Id": 503, "Name": "Hiányzó kontakt", "CategoryId": 12, "ContactId": 999}),
    );

    let sha = "ab".repeat(32);
    std::fs::write(
        dir.join("manifest.jsonl"),
        format!(
            "{}\n",
            json!({"source_url": "https://r3.minicrm.hu/files/500/IMG_1.jpg", "minicrm_project_id": 500, "minicrm_contact_id": 101,
                   "field": "$.Atveteli_fotok[0]", "filename": "IMG_1.jpg", "extension": "jpg"})
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("fetched.jsonl"),
        format!(
            "{}\n",
            json!({"source_url": "https://r3.minicrm.hu/files/500/IMG_1.jpg", "sha256": sha, "byte_size": 2048,
                   "content_type": "image/jpeg", "storage_key": format!("migration/originals/{sha}.jpg"), "fetched_at": "2026-09-11T10:00:00Z"})
        ),
    )
    .unwrap();
    dir
}

fn mapping_value(assignee_email: &str) -> Value {
    json!({
        "categories": {"12": "order", "15": "lead"},
        "order_statuses": {"3003": "production"},
        "lead_statuses": {"2002": "contacted"},
        "default_order_stage": "completed",
        "default_lead_stage": "lost",
        "image_fields": {"$.Atveteli_fotok[]": "intake"},
        "order_value_field": "Osszeg",
        "order_number_field": "Sorszam",
        "order_fields": {
            "vehicle_make": "Jarmu.Marka",
            "vehicle_model": "Jarmu.Tipus",
            "vehicle_vin": "Jarmu.Alvazszam",
            "description": "Leiras",
            "due_date": "Hatarido",
            "project_type": "Projekttipus",
            "assignee": "Felelos"
        },
        "vehicle_plate_from_title": "[A-Z]{3}-[0-9]{3}",
        "partner_fields": {"eu_tax_number": "EuVatNumber", "country": "CountryId", "notes": "Comment"},
        "countries": {"Deutschland": "DE"},
        "project_types": {"Hutofelepitmeny": "refrigerated_body"},
        "assignees": {"77": assignee_email}
    })
}

fn mapping(dir: &Path, assignee_email: &str) -> Mapping {
    let path = dir.join("mapping.json");
    write(path.clone(), &mapping_value(assignee_email));
    Mapping::load(&path).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn load_is_complete_idempotent_and_reconcilable(pool: PgPool) {
    let state = common::state(pool.clone());
    let dir = fixture();
    let assignee = common::user(&pool, autocrm::domain::role::Role::Designer).await;
    let mapping = mapping(&dir, &assignee.email);
    let raw = dir.join("raw");

    // --dry-run resolves every field and writes nothing (V7.2).
    let dry = load::run(&state, &mapping, &raw, &dir, true).await.unwrap();
    // Both order-category projects are counted, including 503 whose contact was never
    // extracted: the dry run answers "what would the mapping find", not "what loads".
    assert_eq!(dry.orders, 2);
    assert_eq!(dry.order_field_coverage.get("vehicle_plate"), Some(&1));
    assert_eq!(dry.order_field_coverage.get("project_type_id"), Some(&1));
    let written: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(written, 0, "a dry run writes nothing");

    let first = load::run(&state, &mapping, &raw, &dir, false)
        .await
        .unwrap();
    assert_eq!(first.notes, 2, "MiniCRM to-dos become order_notes");
    assert_eq!(
        (
            first.partners,
            first.contacts,
            first.orders,
            first.leads,
            first.images
        ),
        (2, 1, 1, 1, 1)
    );
    assert_eq!(first.skipped_projects, 2);
    assert!(
        first
            .problems
            .iter()
            .any(|p| p.contains("category Some(99)")),
        "{:?}",
        first.problems
    );
    assert!(
        first.problems.iter().any(|p| p.contains("project 503")),
        "{:?}",
        first.problems
    );
    assert!(
        first.problems.iter().any(|p| p.contains("nem-email")),
        "{:?}",
        first.problems
    );

    // Re-running changes nothing and creates no duplicates.
    let second = load::run(&state, &mapping, &raw, &dir, false)
        .await
        .unwrap();
    assert_eq!(second.images, 0);
    let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    let stages: i64 = sqlx::query_scalar("SELECT count(*) FROM order_stages")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((orders, stages), (1, 1));

    let order = sqlx::query!(
        r#"SELECT o.id, o.number, o.valuation_date, o.raw_import, p.name AS partner, c.name AS "contact?",
                  o.vehicle_make, o.vehicle_model, o.vehicle_plate, o.vehicle_vin, o.description,
                  o.due_date, o.assigned_to, t.key AS "project_type?",
                  (SELECT stage_key FROM order_stages s WHERE s.order_id = o.id) AS "stage!",
                  (SELECT unit_price FROM order_items i WHERE i.order_id = o.id) AS "unit_price!"
           FROM orders o JOIN partners p ON p.id = o.partner_id LEFT JOIN contacts c ON c.id = o.contact_id
           LEFT JOIN project_types t ON t.id = o.project_type_id
           WHERE o.minicrm_id = 500"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // V7.1: the number comes from the mapping's order_number_field, not MC-{id}.
    assert_eq!(order.number, "2019/0042");
    // V1.1: every order column with a source now arrives.
    assert_eq!(order.vehicle_make.as_deref(), Some("Iveco"));
    assert_eq!(order.vehicle_model.as_deref(), Some("Daily"));
    assert_eq!(order.vehicle_plate.as_deref(), Some("ABC-123"));
    assert_eq!(order.vehicle_vin.as_deref(), Some("WJMM1VNT0 0C123456"));
    assert_eq!(
        order.description.as_deref(),
        Some("Hűtős felépítmény gyártása.")
    );
    assert_eq!(order.due_date, NaiveDate::from_ymd_opt(2019, 8, 1));
    assert_eq!(order.assigned_to, Some(assignee.user_id));
    assert_eq!(order.project_type.as_deref(), Some("refrigerated_body"));
    assert_eq!(order.partner, "Müller Kühlung GmbH");
    assert_eq!(order.contact.as_deref(), Some("Müller Michael"));
    assert_eq!(order.stage, "production");
    assert_eq!(order.unit_price, 123_456_750);
    assert_eq!(
        order.valuation_date,
        NaiveDate::from_ymd_opt(2019, 5, 10).unwrap()
    );
    assert_eq!(
        order.raw_import.unwrap()["Egyedi_mezo"],
        "megmarad",
        "unmapped fields are preserved"
    );

    let image = sqlx::query!("SELECT category::text AS \"category!\", immutable, source_ref FROM images WHERE order_id = $1", order.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(image.category, "intake");
    assert!(image.immutable);
    assert!(image.source_ref.unwrap().ends_with("IMG_1.jpg"));
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = 'process_image'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 1);

    // V1.2: partner columns and the AddressList file that nothing used to open.
    let partner = sqlx::query!(
        "SELECT eu_tax_number, country, postal_code, city, address_line, notes
           FROM partners WHERE minicrm_id = 100"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(partner.eu_tax_number.as_deref(), Some("DE123456789"));
    assert_eq!(partner.country, "DE");
    assert_eq!(partner.postal_code.as_deref(), Some("80331"));
    assert_eq!(partner.city.as_deref(), Some("München"));
    assert_eq!(partner.address_line.as_deref(), Some("Bahnhofstrasse 1"));
    assert_eq!(partner.notes.as_deref(), Some("Nagy ügyfél"));

    // V1.3: the to-do history, on the order, in order.
    let notes = sqlx::query!(
        "SELECT author_name, body FROM order_notes WHERE order_id = $1 ORDER BY occurred_at",
        order.id
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].author_name.as_deref(), Some("Nagy Béla"));
    assert_eq!(notes[0].body, "Ügyfél jóváhagyta a tervet.");

    let lead_stage: String = sqlx::query_scalar("SELECT ls.stage_key FROM lead_stages ls JOIN leads l ON l.id = ls.lead_id WHERE l.minicrm_id = 501")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(lead_stage, "contacted");

    let report = reconcile::run(&state, &mapping, &raw, &dir).await.unwrap();
    assert!(report.contains("| Orders | 2 | 1 | ⚠️ |"), "{report}");
    assert!(report.contains("MiniCRM project 503"), "{report}");
    assert!(
        report.contains("| Partners (businesses + standalone people) | 2 | 2 | ✅ |"),
        "{report}"
    );
    assert!(
        report.contains("Projects in unmapped categories: **1**"),
        "{report}"
    );
    // V1.6: coverage and a field-level sample, not just counts.
    assert!(report.contains("## Field coverage"), "{report}");
    assert!(
        report.contains("| orders | vehicle_plate | 1 | 100.0% |"),
        "{report}"
    );
    assert!(
        report.contains("| partners | city | 1 | 50.0% |"),
        "{report}"
    );
    assert!(
        report.contains("Imported activity entries (`order_notes`): **2**"),
        "{report}"
    );
    assert!(report.contains("## Field-level sample"), "{report}");
    assert!(report.contains("**Orders: no differences.**"), "{report}");
    // V1.4: the report says outright that historical stage durations are unavailable.
    assert!(
        report.contains("Historical stage durations are not available"),
        "{report}"
    );

    let _ = std::fs::remove_dir_all(dir);
}

/// V1.1: a MiniCRM value the mapping does not cover must stop the load, not default
/// silently. The previous behaviour — no project type at all — is what made
/// `volume_by_project_type` bucket every historical order as "(nincs megadva)".
#[sqlx::test(migrations = "./migrations")]
async fn an_unmapped_project_type_stops_the_load(pool: PgPool) {
    let state = common::state(pool.clone());
    let dir = fixture();
    let assignee = common::user(&pool, autocrm::domain::role::Role::Designer).await;
    let mut value = mapping_value(&assignee.email);
    value["project_types"] = json!({});
    let path = dir.join("mapping-gap.json");
    write(path.clone(), &value);
    let mapping = Mapping::load(&path).unwrap();

    let error = load::run(&state, &mapping, &dir.join("raw"), &dir, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("Hutofelepitmeny"), "{error}");
    assert!(error.contains("project type"), "{error}");

    let written: i64 = sqlx::query_scalar("SELECT count(*) FROM orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(written, 0, "nothing is written before the check passes");

    let _ = std::fs::remove_dir_all(dir);
}
