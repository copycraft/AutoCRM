//! The client's November 2026 round: invoices prepared in the Számlázó with a preview,
//! technical content split by department with its own rights and notices, and repeat
//! orders copied from an earlier project.

mod common;

use autocrm::domain::role::Role;
use autocrm::repo::{departments, invoice_drafts, order_modules, orders, questionnaire};
use autocrm::service::invoicing::{self, IssueRequest};
use autocrm::service::modules;
use autocrm::service::orders::NewItem;
use autocrm::service::repeat_order::{CopyRequest, copy_order};
use rust_decimal::Decimal;
use sqlx::PgPool;

fn item(description: &str, quantity: &str, unit_price: i64) -> NewItem {
    NewItem {
        description: description.into(),
        quantity: quantity.parse::<Decimal>().unwrap(),
        unit_price,
    }
}

async fn department(pool: &PgPool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM departments WHERE key = $1")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ── Phase 2: the draft and its preview ──────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn the_preview_prices_the_order_items_and_names_what_would_stop_the_issue(pool: PgPool) {
    let state = common::state_with_nav(pool.clone(), "http://127.0.0.1:9", "12345678-2-11");
    let user = common::user(&pool, Role::Office).await;
    // `common::order` files a partner with no address: the issue would be refused.
    let order = common::order(
        &pool,
        &user,
        "HUF",
        vec![
            item("Hűtőgép beépítés", "1", 10_000_000),
            item("Szigetelés", "2", 2_500_000),
        ],
    )
    .await;

    let preview = invoicing::preview_invoice(&state, order.id, &IssueRequest::default())
        .await
        .unwrap();
    assert_eq!(preview.lines.len(), 2);
    assert_eq!(preview.net_minor, 15_000_000);
    assert_eq!(preview.vat_minor, 4_050_000, "27% by default");
    assert_eq!(preview.gross_minor, 19_050_000);
    assert_eq!(preview.payment_method, "TRANSFER");
    assert!(
        preview.problems.iter().any(|p| p.contains("irányítószám")),
        "{:?}",
        preview.problems
    );
    // Nothing was numbered or written.
    let invoices: i64 = sqlx::query_scalar("SELECT count(*) FROM invoices WHERE order_id = $1")
        .bind(order.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(invoices, 0);

    // Terms from the draft show in the preview.
    let req = IssueRequest {
        vat_rate: Some("0.05".parse().unwrap()),
        payment_method: Some("CASH".into()),
        ..IssueRequest::default()
    };
    let preview = invoicing::preview_invoice(&state, order.id, &req)
        .await
        .unwrap();
    assert_eq!(preview.vat_minor, 750_000);
    assert_eq!(preview.payment_method, "CASH");
}

#[sqlx::test(migrations = "./migrations")]
async fn a_draft_keeps_only_the_terms_and_refuses_bad_ones(pool: PgPool) {
    let user = common::user(&pool, Role::Office).await;
    let order = common::order(&pool, &user, "HUF", vec![item("Munka", "1", 100_000)]).await;

    let req = IssueRequest {
        vat_rate: Some("0.27".parse().unwrap()),
        payment_method: Some("CASH".into()),
        ..IssueRequest::default()
    };
    let terms = invoicing::draft_terms(&req).unwrap();
    invoice_drafts::upsert(&pool, order.id, &terms, user.user_id)
        .await
        .unwrap();
    let draft = invoice_drafts::find(&pool, order.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(draft.payment_method.as_deref(), Some("CASH"));
    assert_eq!(invoice_drafts::list(&pool).await.unwrap().len(), 1);
    assert_eq!(
        invoice_drafts::list(&pool).await.unwrap()[0].net_minor,
        100_000
    );

    let bad = IssueRequest {
        payment_method: Some("BITCOIN".into()),
        ..IssueRequest::default()
    };
    assert!(invoicing::draft_terms(&bad).is_err());
    let bad = IssueRequest {
        vat_rate: Some("27".parse().unwrap()),
        ..IssueRequest::default()
    };
    assert!(invoicing::draft_terms(&bad).is_err());

    assert!(invoice_drafts::delete(&pool, order.id).await.unwrap());
    assert!(
        invoice_drafts::find(&pool, order.id)
            .await
            .unwrap()
            .is_none()
    );
}

// ── Phase 3: departments ────────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn the_three_departments_come_in_their_colours(pool: PgPool) {
    let all = departments::list(&pool).await.unwrap();
    let pairs: Vec<(&str, &str)> = all
        .iter()
        .map(|d| (d.name.as_str(), d.color.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("Felépítmény", "#64748b"),
            ("Lakatos", "#dc2626"),
            ("Hűtés", "#2563eb")
        ]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn members_write_their_own_department_and_the_office_writes_all(pool: PgPool) {
    let office = common::user(&pool, Role::Office).await;
    let welder = common::user(&pool, Role::Designer).await;
    let lakatos = department(&pool, "lakatos").await;
    let hutes = department(&pool, "hutes").await;
    let mut conn = pool.acquire().await.unwrap();
    departments::set_for_user(&mut conn, welder.user_id, &[(lakatos, false)])
        .await
        .unwrap();

    assert!(
        modules::can_edit(&mut conn, &welder, lakatos)
            .await
            .unwrap()
    );
    assert!(!modules::can_edit(&mut conn, &welder, hutes).await.unwrap());
    assert!(modules::can_edit(&mut conn, &office, hutes).await.unwrap());
    let viewer = common::user(&pool, Role::Viewer).await;
    assert!(
        !modules::can_edit(&mut conn, &viewer, lakatos)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn releasing_tells_only_the_heads_and_a_change_goes_out_again(pool: PgPool) {
    let state = common::state(pool.clone());
    sqlx::query("UPDATE settings SET automatic_email_enabled = true")
        .execute(&pool)
        .await
        .unwrap();
    let office = common::user(&pool, Role::Office).await;
    let head = common::user(&pool, Role::Designer).await;
    let member = common::user(&pool, Role::Designer).await;
    let hutes = department(&pool, "hutes").await;
    let mut conn = pool.acquire().await.unwrap();
    departments::set_for_user(&mut conn, head.user_id, &[(hutes, true)])
        .await
        .unwrap();
    departments::set_for_user(&mut conn, member.user_id, &[(hutes, false)])
        .await
        .unwrap();
    drop(conn);

    let order = common::order(&pool, &office, "HUF", vec![]).await;
    let id = order_modules::insert(&pool, order.id, hutes, "", office.user_id)
        .await
        .unwrap();
    // Empty: nothing to hand over.
    assert!(modules::release(&state, &office, id).await.is_err());

    order_modules::set_content(&pool, id, "Thermo King V-500, 2 kör", office.user_id)
        .await
        .unwrap();
    let released = modules::release(&state, &office, id).await.unwrap();
    assert_eq!(released.status, "released");
    assert!(!released.changed_since_release());

    let told = |user_id: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM notifications WHERE user_id = $1 AND kind = 'module'",
            )
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(told(head.user_id).await, 1);
    assert_eq!(told(member.user_id).await, 0, "only heads, for now");
    let letters: Vec<String> = sqlx::query_scalar(
        "SELECT to_address FROM email_messages WHERE trigger = 'module_released'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(letters, vec![head.email.clone()]);

    // Released and unchanged: releasing again tells nobody twice.
    modules::release(&state, &office, id).await.unwrap();
    assert_eq!(told(head.user_id).await, 1);

    // Written to after release: flagged, and sent out again as a change.
    order_modules::set_content(&pool, id, "Thermo King V-500, 3 kör", office.user_id)
        .await
        .unwrap();
    let m = order_modules::find(&pool, id).await.unwrap().unwrap();
    assert!(m.changed_since_release());
    let again = modules::release(&state, &office, id).await.unwrap();
    assert!(!again.changed_since_release());
    assert_eq!(told(head.user_id).await, 2);
    let subjects: Vec<String> = sqlx::query_scalar(
        "SELECT subject FROM email_messages WHERE trigger = 'module_released' ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(subjects[0].contains("kiadva"), "{subjects:?}");
    assert!(subjects[1].contains("módosult"), "{subjects:?}");

    // A member of another department may not hand it over.
    let welder = common::user(&pool, Role::Designer).await;
    let lakatos = department(&pool, "lakatos").await;
    let mut conn = pool.acquire().await.unwrap();
    departments::set_for_user(&mut conn, welder.user_id, &[(lakatos, true)])
        .await
        .unwrap();
    assert!(modules::release(&state, &welder, id).await.is_err());
}

// ── Phase 4: repeat orders ──────────────────────────────────────────────────

#[sqlx::test(migrations = "./migrations")]
async fn a_repeat_order_copies_the_job_but_not_the_vehicle(pool: PgPool) {
    let state = common::state(pool.clone());
    let office = common::user(&pool, Role::Office).await;
    let source = common::order(
        &pool,
        &office,
        "HUF",
        vec![item("Hűtőfelépítmény", "1", 420_000_000)],
    )
    .await;
    sqlx::query("UPDATE orders SET vehicle_vin = 'WF0XXXTTGXEY12345', due_date = '2026-12-01' WHERE id = $1")
        .bind(source.id)
        .execute(&pool)
        .await
        .unwrap();
    questionnaire::set_order_answers(
        &pool,
        source.id,
        serde_json::json!({
            "heats": false,
            "cools": true,
            "cooling_unit_make": "Thermo King",
            "insulation_mm": 80,
            "cooling_unit_serial": "TK-998877",
        })
        .as_object()
        .unwrap(),
    )
    .await
    .unwrap();
    let hutes = department(&pool, "hutes").await;
    order_modules::insert(&pool, source.id, hutes, "V-500, 2 kör", office.user_id)
        .await
        .unwrap();

    let copied = copy_order(
        &state,
        &office,
        source.id,
        &CopyRequest {
            title: Some("Hűtőfelépítmény – második".into()),
            items: true,
            spec: true,
            modules: true,
            files: true,
        },
    )
    .await
    .unwrap();
    let new = orders::find(&pool, copied.order.id).await.unwrap().unwrap();
    assert_ne!(new.id, source.id);
    assert_eq!(new.title, "Hűtőfelépítmény – második");
    assert_eq!(new.partner_id, source.partner_id);
    assert_eq!(new.vehicle_make.as_deref(), Some("Iveco"));
    assert_eq!(new.vehicle_model.as_deref(), Some("Daily"));
    assert_eq!(new.vehicle_plate, None);
    assert_eq!(new.vehicle_vin, None);
    assert_eq!(new.due_date, None);
    assert_eq!(new.related_order_id, Some(source.id));
    assert_eq!(new.relation.as_deref(), Some("repeat"));

    let items = autocrm::repo::order_items::list(&pool, new.id)
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].unit_price, 420_000_000, "the old price");

    let spec = questionnaire::order_answers(&pool, new.id).await.unwrap();
    assert_eq!(
        spec.get("cooling_unit_make"),
        Some(&serde_json::json!("Thermo King"))
    );
    assert_eq!(spec.get("insulation_mm"), Some(&serde_json::json!(80)));
    assert_eq!(
        spec.get("cooling_unit_serial"),
        None,
        "a new unit, a new serial"
    );

    let mods = order_modules::list_for_order(&pool, new.id).await.unwrap();
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].content, "V-500, 2 kör");
    assert_eq!(mods[0].status, "draft", "to be released again");
    assert!(copied.failed_files.is_empty());

    // Nothing ticked: only the data row.
    let bare = copy_order(
        &state,
        &office,
        source.id,
        &CopyRequest {
            title: None,
            items: false,
            spec: false,
            modules: false,
            files: false,
        },
    )
    .await
    .unwrap();
    assert!(
        autocrm::repo::order_items::list(&pool, bare.order.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        questionnaire::order_answers(&pool, bare.order.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(bare.order.title, source.title);
}

#[sqlx::test(migrations = "./migrations")]
async fn a_repeat_order_copies_the_drawings_into_its_own_folder(pool: PgPool) {
    let (state, dir) = common::state_fs(pool.clone());
    let office = common::user(&pool, Role::Office).await;
    let source = common::order(&pool, &office, "HUF", vec![]).await;
    let bytes = b"%PDF-1.4 drawing".to_vec();
    let key = format!("test/{}/rajz.pdf", source.id);
    state
        .storage
        .put_bytes(&key, bytes.clone(), "application/pdf")
        .await
        .unwrap();
    let doc_id: i64 = sqlx::query_scalar(
        "INSERT INTO documents (order_id, kind, filename, content_type, storage_key, content_hash, byte_size)
         VALUES ($1, 'design', 'rajz.pdf', 'application/pdf', $2, digest($3::bytea, 'sha256'), $4)
         RETURNING id",
    )
    .bind(source.id)
    .bind(&key)
    .bind(&bytes)
    .bind(bytes.len() as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    let hutes = department(&pool, "hutes").await;
    let module = order_modules::insert(&pool, source.id, hutes, "lásd rajz", office.user_id)
        .await
        .unwrap();
    order_modules::add_file(&pool, module, None, Some(doc_id))
        .await
        .unwrap();

    let copied = copy_order(
        &state,
        &office,
        source.id,
        &CopyRequest {
            title: None,
            items: true,
            spec: true,
            modules: true,
            files: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(copied.copied_documents, 1, "{:?}", copied.failed_files);
    let docs = autocrm::repo::documents::list_for_order(&pool, copied.order.id)
        .await
        .unwrap();
    assert_eq!(docs.len(), 1);
    assert_ne!(docs[0].storage_key, key, "its own file in its own folder");
    assert_eq!(
        state.storage.get_bytes(&docs[0].storage_key).await.unwrap(),
        bytes
    );
    // And the new module's sheet carries the copy, not the original.
    let files = order_modules::files_for_order(&pool, copied.order.id)
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].document_id, Some(docs[0].id));
    let _ = std::fs::remove_dir_all(dir);
}
