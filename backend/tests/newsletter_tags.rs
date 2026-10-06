//! Marketing: newsletter tags, tagging subscribers, pasting in a list, and a blast aimed at
//! some tags reaching only their subscribers.

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

async fn call(pool: &PgPool, method: &str, uri: &str, bearer: &str, body: Option<Value>) -> (StatusCode, Value) {
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
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn token(pool: &PgPool, role: Role) -> String {
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

async fn tag_id(pool: &PgPool, label: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM newsletter_tags WHERE label = $1")
        .bind(label)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn an_import_tags_new_and_known_addresses_and_skips_the_opted_out(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let bakeries = tag_id(&pool, "Pékségek").await;
    let romanian = tag_id(&pool, "Román - pékségek").await;

    // One known active subscriber, one who opted out.
    sqlx::query(
        "INSERT INTO newsletter_subscriptions (email, name, source, confirmed_at, unsubscribed_at)
         VALUES ('regi@pekseg.hu', 'Régi', 'office', now(), NULL),
                ('kilepett@pekseg.hu', 'Kilépett', 'office', now(), now())",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (status, result) = call(
        &pool,
        "POST",
        "/api/newsletter/import",
        &office,
        Some(json!({
            "text": "E-mail;Név\nuj@pekseg.hu;Új Pékség\nREGI@pekseg.hu\nkilepett@pekseg.hu\nnem cím\n\nmasik@pekseg.ro,Brutăria",
            "tag_ids": [bakeries]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["added"], 2);
    assert_eq!(result["existing"], 1);
    assert_eq!(result["opted_out"], 1);
    assert_eq!(result["invalid"], json!(["nem cím"]));

    // The opted-out address got no tag; the others did, and the new ones are active.
    let (_, rows) = call(&pool, "GET", &format!("/api/newsletter/subscribers?tag={bakeries}"), &office, None).await;
    let mut emails: Vec<&str> = rows["items"].as_array().unwrap().iter().map(|r| r["email"].as_str().unwrap()).collect();
    emails.sort();
    assert_eq!(emails, vec!["masik@pekseg.ro", "regi@pekseg.hu", "uj@pekseg.hu"]);
    let (_, counts) = call(&pool, "GET", "/api/newsletter/subscribers/counts", &office, None).await;
    assert_eq!(counts["active"], 3);
    assert_eq!(counts["unsubscribed"], 1);
    assert_eq!(counts["untagged"], 1);

    // Search, status and the untagged filter.
    let (_, found) = call(&pool, "GET", "/api/newsletter/subscribers?q=brut", &office, None).await;
    assert_eq!(found["items"][0]["email"], "masik@pekseg.ro");
    let (_, out) = call(&pool, "GET", "/api/newsletter/subscribers?status=unsubscribed", &office, None).await;
    assert_eq!(out["items"].as_array().unwrap().len(), 1);
    let (_, bare) = call(&pool, "GET", "/api/newsletter/subscribers?untagged=true", &office, None).await;
    assert_eq!(bare["items"][0]["email"], "kilepett@pekseg.hu");

    // Bulk: move the Romanian one to its own list.
    let ro_id = found["items"][0]["id"].as_i64().unwrap();
    let (status, bulk) = call(
        &pool,
        "POST",
        "/api/newsletter/subscriptions/tags",
        &office,
        Some(json!({ "subscription_ids": [ro_id], "add": [romanian], "remove": [bakeries] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bulk, json!({ "added": 1, "removed": 1 }));

    // The audience follows the tags; a suppressed address is left out of it.
    let (_, all) = call(&pool, "GET", "/api/newsletter/audience", &office, None).await;
    assert_eq!(all["recipients"], 3);
    let (_, hu) = call(&pool, "GET", &format!("/api/newsletter/audience?tags={bakeries}"), &office, None).await;
    assert_eq!(hu["recipients"], 2);
    sqlx::query("INSERT INTO email_suppressions (email) VALUES ('uj@pekseg.hu')")
        .execute(&pool)
        .await
        .unwrap();
    let (_, hu) = call(&pool, "GET", &format!("/api/newsletter/audience?tags={bakeries},{romanian}"), &office, None).await;
    assert_eq!(hu["recipients"], 2);

    // The tag list counts its subscribers.
    let (_, tags) = call(&pool, "GET", "/api/newsletter/tags", &office, None).await;
    let b = tags["items"].as_array().unwrap().iter().find(|t| t["id"] == bakeries).unwrap().clone();
    assert_eq!(b["active_subscribers"], 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn tags_are_edited_and_set_on_one_subscriber(pool: PgPool) {
    let office = token(&pool, Role::Office).await;
    let (status, tag) = call(
        &pool,
        "POST",
        "/api/newsletter/tags",
        &office,
        Some(json!({ "section": "Listák", "label": "Hűtőházak", "color": "#5B93F5" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{tag}");
    assert_eq!(tag["color"], "#5b93f5");
    let id = tag["id"].as_i64().unwrap();
    // Placed at the end of its section, before the next section.
    let last_list: i32 = sqlx::query_scalar("SELECT position FROM newsletter_tags WHERE label = 'Szerbiai temetkezési'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(tag["position"].as_i64().unwrap() > last_list as i64);
    assert!(tag["position"].as_i64().unwrap() < 1010);

    let (status, _) = call(
        &pool,
        "POST",
        "/api/newsletter/tags",
        &office,
        Some(json!({ "section": "listák", "label": "hűtőházak" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, sub) = call(
        &pool,
        "POST",
        "/api/newsletter/subscriptions",
        &office,
        Some(json!({ "email": "iroda@hutohaz.hu", "name": "Hűtőház", "tag_ids": [id] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{sub}");
    let sub_id = sub["id"].as_i64().unwrap();

    let other = tag_id(&pool, "Fuvarozók").await;
    let (status, ids) = call(
        &pool,
        "PUT",
        &format!("/api/newsletter/subscriptions/{sub_id}/tags"),
        &office,
        Some(json!({ "tag_ids": [other] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids, json!([other]));

    // Archived tags cannot be put on anyone.
    let (status, _) = call(&pool, "PATCH", &format!("/api/newsletter/tags/{id}"), &office, Some(json!({ "archived": true }))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &pool,
        "PUT",
        &format!("/api/newsletter/subscriptions/{sub_id}/tags"),
        &office,
        Some(json!({ "tag_ids": [other, id] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Deleting the subscription takes its tags with it.
    let (status, _) = call(&pool, "DELETE", &format!("/api/newsletter/subscriptions/{sub_id}"), &office, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // A viewer reads but does not change.
    let viewer = token(&pool, Role::Viewer).await;
    let (status, _) = call(&pool, "GET", "/api/newsletter/tags", &viewer, None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&pool, "POST", "/api/newsletter/import", &viewer, Some(json!({ "text": "a@b.hu" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
