//! The CRM assistant: a small local language model (Qwen2.5-0.5B-Instruct by default,
//! served by llama.cpp next to the app) that answers questions about the data through
//! read-only tools, and builds list filters the user can open or save as a view.
//!
//! Nothing here writes: every tool reads, and `create_filter` only returns a filter for the
//! browser. A half-billion-parameter model is fast on a CPU but easily confused, so the
//! tools take names rather than ids (a tag by its label, a stage by key or label), every
//! argument is checked here, and a bad call comes back to the model as a plain error it
//! can correct.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::config::AiConfig;
use crate::domain::search_terms::fold;
use crate::domain::stage::StageEntity;
use crate::error::{AppError, AppResult};
use crate::repo::orders::OrderFilter;
use crate::repo::timeline::TimelineEntity;
use crate::repo::{
    config, incoming_invoices, invoices, lead_tags, leads, like_pattern, newsletter_tags, orders,
    partners, search, timeline,
};

/// Today in Budapest, where the office works.
fn local_today() -> chrono::NaiveDate {
    chrono::Utc::now()
        .with_timezone(&chrono_tz::Europe::Budapest)
        .date_naive()
}

/// Tries at a tool call: the first, and one more with the reason the first was refused.
const MAX_ROUNDS: usize = 2;
const ROWS: i64 = 10;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct ChatMessage {
    /// `user` or `assistant`.
    pub role: String,
    pub content: String,
}

/// A list filter the model built: open it, or save it as a view of that list.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
pub struct FilterSuggestion {
    /// leads | orders | partners | incoming_invoices | subscribers
    pub list: String,
    /// The list's own URL parameters (what a saved view stores).
    pub params: BTreeMap<String, String>,
    /// Where it opens, without the locale: `/leads?stage=quoted&open=1`.
    pub path: String,
    /// A short human description.
    pub label: String,
}

/// One tool the model used, for showing its work.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ToolStep {
    pub tool: String,
    #[schema(value_type = Object)]
    pub arguments: Value,
    /// False when the call was refused (bad arguments); the model was told why.
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AssistantReply {
    pub reply: String,
    pub filters: Vec<FilterSuggestion>,
    pub steps: Vec<ToolStep>,
}

fn tools() -> Value {
    let f = |name: &str, description: &str, properties: Value, required: &[&str]| {
        json!({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": { "type": "object", "properties": properties, "required": required }
            }
        })
    };
    json!([
        f(
            "search",
            "Keresés mindenben: megrendelés, partner, lead (név, szám, rendszám, e-mail).",
            json!({ "query": { "type": "string" } }),
            &["query"]
        ),
        f(
            "list_leads",
            "Leadek listája szűrve (legfeljebb 10, legújabb elöl).",
            json!({
                "query": { "type": "string", "description": "szöveg a címben, névben, e-mailben" },
                "stage": { "type": "string", "description": "fázis kulcsa vagy neve" },
                "tag": { "type": "string", "description": "címke neve, pl. JEGELVE" },
                "open_only": { "type": "boolean", "description": "csak nyitott leadek" }
            }),
            &[]
        ),
        f(
            "list_orders",
            "Megrendelések listája szűrve (legfeljebb 10, legújabb elöl).",
            json!({
                "query": { "type": "string" },
                "stage": { "type": "string", "description": "fázis kulcsa vagy neve" },
                "open_only": { "type": "boolean" }
            }),
            &[]
        ),
        f(
            "get_record",
            "Egy lead, megrendelés vagy partner adatai azonosító alapján.",
            json!({
                "kind": { "type": "string", "enum": ["lead", "order", "partner"] },
                "id": { "type": "integer" }
            }),
            &["kind", "id"]
        ),
        f(
            "overview",
            "Összesítő: leadek és megrendelések fázisonként, számlák és bejövő számlák listánként, hírlevél-feliratkozók.",
            json!({}),
            &[]
        ),
        f(
            "create_filter",
            "Szűrőt készít egy listához, amit a felhasználó megnyithat vagy elmenthet. Használd, ha szűrt listát kérnek.",
            json!({
                "list": { "type": "string", "enum": ["leads", "orders", "partners", "incoming_invoices", "subscribers"] },
                "query": { "type": "string" },
                "stage": { "type": "string", "description": "leads/orders: fázis kulcsa vagy neve" },
                "tag": { "type": "string", "description": "leads: lead címke; subscribers: hírlevél lista neve" },
                "open_only": { "type": "boolean", "description": "leads/orders" },
                "bucket": { "type": "string", "description": "incoming_invoices: open_invoice, open_proforma, transferred, cash, partial, cash_receipt, booking_only" },
                "status": { "type": "string", "description": "subscribers: active, pending, unsubscribed" },
                "name": { "type": "string", "description": "a szűrő rövid neve" }
            }),
            &["list"]
        ),
        f(
            "list_incoming_invoices",
            "Bejövő (beszállítói) számlák listája (legfeljebb 10).",
            json!({
                "query": { "type": "string", "description": "beszállító vagy számlaszám" },
                "bucket": { "type": "string", "description": "open_invoice, open_proforma, transferred, cash, partial, cash_receipt, booking_only" }
            }),
            &[]
        ),
        f(
            "expiring_quotes",
            "Hamarosan lejáró (vagy nemrég lejárt) árajánlatok nyitott leadeken.",
            json!({ "days": { "type": "integer", "description": "hány napon belül (alapból 7)" } }),
            &[]
        ),
        f(
            "overdue_invoices",
            "Lejárt fizetési határidejű, ki nem fizetett számlák, opcionálisan egy ügyfélé.",
            json!({ "customer": { "type": "string", "description": "a partner neve vagy része" } }),
            &[]
        ),
        f(
            "recent_history",
            "Mi történt / mi változott egy rekordon mostanában (előzmények).",
            json!({
                "kind": { "type": "string", "enum": ["lead", "order", "partner"] },
                "id": { "type": "integer" }
            }),
            &["kind", "id"]
        ),
        f(
            "reply",
            "Csak köszönésre vagy nem a CRM adataira vonatkozó kérdésre: rövid válasz.",
            json!({ "text": { "type": "string" } }),
            &["text"]
        ),
    ])
}

async fn system_prompt(db: &PgPool, today: &str) -> AppResult<String> {
    let lead_stages = config::stage_definitions(db, StageEntity::Lead).await?;
    let order_stages = config::stage_definitions(db, StageEntity::Order).await?;
    let list = |defs: &[crate::domain::stage::StageDefinition]| {
        defs.iter()
            .map(|d| format!("{} ({})", d.key, d.label_hu))
            .collect::<Vec<_>>()
            .join(", ")
    };
    // English instructions: a half-billion-parameter model follows them far better than
    // Hungarian ones. The examples are the whole job: pick one tool and its arguments.
    Ok(format!(
        "You route questions about a Hungarian CRM (Autotherm) to exactly one tool. Today is {today}.\n\
         Always call a tool. Never invent data. Copy names (tags, stages, people, companies) from the question.\n\
         Lead stages (key = label): {}.\nOrder stages (key = label): {}.\n\
         Incoming invoice lists: open_invoice, open_proforma, transferred, cash, partial, cash_receipt, booking_only.\n\
         Examples:\n\
         \"Hány nyitott lead van fázisonként?\" -> overview {{}}\n\
         \"Összesítő a számlákról\" -> overview {{}}\n\
         \"Mutasd a JEGELVE címkés leadeket\" -> list_leads {{\"tag\": \"JEGELVE\"}}\n\
         \"Árajánlat fázisú leadek\" -> list_leads {{\"stage\": \"quoted\"}}\n\
         \"Melyik megrendelés van gyártásban?\" -> list_orders {{\"stage\": \"production\"}}\n\
         \"Szűrő: nyitott megrendelések gyártásban\" -> create_filter {{\"list\": \"orders\", \"stage\": \"production\", \"open_only\": true}}\n\
         \"Készíts szűrőt a Pékségek hírlevél listára\" -> create_filter {{\"list\": \"subscribers\", \"tag\": \"Pékségek\"}}\n\
         \"Melyik bejövő számla vár még fizetésre?\" -> list_incoming_invoices {{\"bucket\": \"open_invoice\"}}\n\
         \"Ki az a Kovács Péter?\" -> search {{\"query\": \"Kovács Péter\"}}\n\
         \"Mi van a 12-es leaddel?\" -> get_record {{\"kind\": \"lead\", \"id\": 12}}\n\
         \"Szia, mit tudsz?\" -> reply {{\"text\": \"Szia! Kérdezz a leadekről, megrendelésekről, számlákról, vagy kérj szűrt listát.\"}}",
        list(&lead_stages),
        list(&order_stages)
    ))
}

// ---------------------------------------------------------------- the model

#[derive(Debug, Deserialize)]
struct Completion {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ModelMessage,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ModelMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tool_calls: Vec<ModelToolCall>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ModelToolCall {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default = "function_type")]
    kind: String,
    function: ModelFunction,
}

fn function_type() -> String {
    "function".into()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ModelFunction {
    name: String,
    /// A JSON string, per the OpenAI format; some servers send an object.
    #[serde(default)]
    arguments: Value,
}

/// Asks for exactly one tool call: `tool_choice: required` makes the server constrain the
/// output to a well-formed call, which a small model cannot be trusted to produce freely.
async fn complete(ai: &AiConfig, messages: &[Value]) -> AppResult<ModelMessage> {
    let body = json!({
        "model": ai.model,
        "messages": messages,
        "temperature": 0.0,
        "max_tokens": 300,
        "tools": tools(),
        "tool_choice": "required",
        "parallel_tool_calls": false,
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(ai.timeout_secs))
        .build()
        .map_err(|e| AppError::internal(format!("assistant client: {e}")))?;
    let response = client
        .post(format!("{}/v1/chat/completions", ai.url))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "assistant model unreachable");
            AppError::rule(
                "assistant_unavailable",
                "the assistant model is not reachable",
            )
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        tracing::warn!(%status, body = %text.chars().take(500).collect::<String>(), "assistant model error");
        return Err(AppError::rule(
            "assistant_unavailable",
            "the assistant model answered with an error",
        ));
    }
    let completion: Completion = response
        .json()
        .await
        .map_err(|e| AppError::internal(format!("assistant model reply: {e}")))?;
    completion
        .choices
        .into_iter()
        .next()
        .map(|c| c.message)
        .ok_or_else(|| AppError::internal("assistant model returned no choice"))
}

/// Tool calls a small model wrote into its text instead of the structured field:
/// `<tool_call>{"name": ..., "arguments": {...}}</tool_call>` (Qwen's format), or one
/// bare JSON object of that shape.
fn calls_in_text(content: &str) -> Vec<ModelToolCall> {
    let mut out = Vec::new();
    let mut parse = |s: &str| {
        if let Ok(v) = serde_json::from_str::<Value>(s.trim())
            && let Some(name) = v.get("name").and_then(Value::as_str)
        {
            out.push(ModelToolCall {
                id: format!("text-{}", out.len()),
                kind: function_type(),
                function: ModelFunction {
                    name: name.to_string(),
                    arguments: v.get("arguments").cloned().unwrap_or(json!({})),
                },
            });
        }
    };
    if content.contains("<tool_call>") {
        for part in content.split("<tool_call>").skip(1) {
            parse(part.split("</tool_call>").next().unwrap_or(""));
        }
    } else if content.trim_start().starts_with('{') {
        parse(content);
    }
    out
}

fn arguments(f: &ModelFunction) -> Value {
    match &f.arguments {
        Value::String(s) => serde_json::from_str(s).unwrap_or(json!({})),
        Value::Null => json!({}),
        v => v.clone(),
    }
}

// ---------------------------------------------------------------- tools

fn s<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn b(args: &Value, key: &str) -> bool {
    match args.get(key) {
        Some(Value::Bool(v)) => *v,
        Some(Value::String(v)) => matches!(v.as_str(), "true" | "igen" | "1"),
        _ => false,
    }
}

/// A stage by key or label, or a message listing what exists.
async fn stage_key(db: &PgPool, entity: StageEntity, wanted: &str) -> Result<String, String> {
    let defs = config::stage_definitions(db, entity)
        .await
        .map_err(|e| e.to_string())?;
    let w = fold(wanted);
    defs.iter()
        .find(|d| d.key == wanted || fold(&d.key) == w || fold(&d.label_hu) == w)
        .or_else(|| defs.iter().find(|d| fold(&d.label_hu).contains(&w)))
        .map(|d| d.key.clone())
        .ok_or_else(|| {
            format!(
                "nincs ilyen fázis: {wanted}. Létezők: {}",
                defs.iter()
                    .map(|d| d.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// A lead tag by label (accents and case do not matter), as (id, "HU · label").
/// The same label exists in every market (JEGELVE, Online ajánlatkérők...): a market named
/// in `market` wins, then the tag most leads carry, then the Hungarian list.
async fn lead_tag(
    db: &PgPool,
    wanted: &str,
    market: Option<&str>,
) -> Result<(i64, String), String> {
    let mut tags = lead_tags::list(db, false)
        .await
        .map_err(|e| e.to_string())?;
    tags.sort_by_key(|t| {
        (
            Some(t.market.as_str()) != market,
            std::cmp::Reverse(t.total_leads),
            t.market != "hu",
            t.id,
        )
    });
    let w = fold(wanted);
    tags.iter()
        .find(|t| fold(&t.label) == w)
        .or_else(|| tags.iter().find(|t| fold(&t.label).contains(&w)))
        .map(|t| (t.id, format!("{} · {}", t.market.to_uppercase(), t.label)))
        .ok_or_else(|| format!("nincs ilyen lead címke: {wanted}"))
}

async fn newsletter_tag(db: &PgPool, wanted: &str) -> Result<(i64, String), String> {
    let tags = newsletter_tags::list(db, false)
        .await
        .map_err(|e| e.to_string())?;
    let w = fold(wanted);
    tags.iter()
        .find(|t| fold(&t.label) == w)
        .or_else(|| tags.iter().find(|t| fold(&t.label).contains(&w)))
        .map(|t| (t.id, t.label.clone()))
        .ok_or_else(|| format!("nincs ilyen hírlevél lista: {wanted}"))
}

/// Turns the model's arguments into a filter the browser understands. Pure apart from
/// name lookups, which the caller resolves first.
pub fn filter_path(list: &str, params: &BTreeMap<String, String>) -> Option<String> {
    let base = match list {
        "leads" => "/leads",
        "orders" => "/orders",
        "partners" => "/partners/business",
        "incoming_invoices" => "/incoming-invoices",
        "subscribers" => "/marketing",
        _ => return None,
    };
    if params.is_empty() {
        return Some(base.to_string());
    }
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    Some(format!("{base}?{query}"))
}

fn urlencode(v: &str) -> String {
    v.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn create_filter(db: &PgPool, args: &Value) -> Result<FilterSuggestion, String> {
    let list = s(args, "list").ok_or("a list kötelező")?;
    let mut params = BTreeMap::new();
    let mut parts = Vec::new();
    if let Some(q) = s(args, "query") {
        params.insert("q".into(), q.to_string());
        parts.push(format!("„{q}”"));
    }
    match list {
        "leads" | "orders" => {
            let entity = if list == "leads" {
                StageEntity::Lead
            } else {
                StageEntity::Order
            };
            if let Some(stage) = s(args, "stage") {
                let key = stage_key(db, entity, stage).await?;
                parts.push(format!("fázis: {key}"));
                params.insert("stage".into(), key);
            }
            if b(args, "open_only") {
                params.insert("open".into(), "1".into());
                parts.push("csak nyitottak".into());
            }
            if list == "leads"
                && let Some(tag) = s(args, "tag")
            {
                let (id, label) = lead_tag(db, tag, s(args, "market")).await?;
                params.insert("tag".into(), id.to_string());
                parts.push(format!("címke: {label}"));
            }
        }
        "incoming_invoices" => {
            if let Some(bucket) = s(args, "bucket") {
                if !incoming_invoices::BUCKETS.contains(&bucket) {
                    return Err(format!(
                        "ismeretlen bucket: {bucket}. Létezők: {}",
                        incoming_invoices::BUCKETS.join(", ")
                    ));
                }
                params.insert("bucket".into(), bucket.to_string());
                parts.push(bucket.to_string());
            }
        }
        "subscribers" => {
            if let Some(status) = s(args, "status") {
                if !["active", "pending", "unsubscribed"].contains(&status) {
                    return Err("a status active, pending vagy unsubscribed".into());
                }
                params.insert("status".into(), status.to_string());
                parts.push(status.to_string());
            }
            if let Some(tag) = s(args, "tag") {
                let (id, label) = newsletter_tag(db, tag).await?;
                params.insert("tag".into(), id.to_string());
                parts.push(format!("lista: {label}"));
            }
        }
        "partners" => {}
        other => return Err(format!("ismeretlen lista: {other}")),
    }
    let path = filter_path(list, &params).ok_or("ismeretlen lista")?;
    let list_name = match list {
        "leads" => "Leadek",
        "orders" => "Megrendelések",
        "partners" => "Partnerek",
        "incoming_invoices" => "Bejövő számlák",
        _ => "Feliratkozók",
    };
    let label = s(args, "name").map(str::to_string).unwrap_or_else(|| {
        if parts.is_empty() {
            list_name.to_string()
        } else {
            format!("{list_name}: {}", parts.join(", "))
        }
    });
    Ok(FilterSuggestion {
        list: list.to_string(),
        params,
        path,
        label,
    })
}

/// Drops nulls and bulky import payloads so a record fits the model's context.
fn compact(mut v: Value) -> Value {
    if let Value::Object(map) = &mut v {
        map.retain(|k, val| !val.is_null() && k != "raw_import" && k != "nav_messages");
    }
    v
}

async fn run_tool(
    db: &PgPool,
    name: &str,
    args: &Value,
    filters: &mut Vec<FilterSuggestion>,
) -> Result<Value, String> {
    let err = |e: sqlx::Error| e.to_string();
    match name {
        "search" => {
            let q = s(args, "query").ok_or("a query kötelező")?;
            Ok(json!({
                "orders": search::orders(db, q, 5).await.map_err(err)?,
                "partners": search::partners(db, q, 5).await.map_err(err)?,
                "leads": search::leads(db, q, 5).await.map_err(err)?,
            }))
        }
        "list_leads" => {
            let pattern = s(args, "query").and_then(like_pattern);
            let stage = match s(args, "stage") {
                Some(st) => Some(stage_key(db, StageEntity::Lead, st).await?),
                None => None,
            };
            let tag = match s(args, "tag") {
                Some(t) => Some(lead_tag(db, t, s(args, "market")).await?.0),
                None => None,
            };
            let rows = leads::search(
                db,
                pattern.as_deref(),
                None,
                stage.as_deref(),
                None,
                tag,
                b(args, "open_only"),
                leads::DEFAULT_SORT,
                ROWS,
                0,
            )
            .await
            .map_err(err)?;
            Ok(json!(rows.iter().map(|l| json!({
                "id": l.id, "title": l.title, "partner": l.partner_name, "contact": l.contact_name,
                "stage": l.stage_label, "assigned": l.assigned_name, "created": l.created_at.date_naive(),
            })).collect::<Vec<_>>()))
        }
        "list_orders" => {
            let stage = match s(args, "stage") {
                Some(st) => Some(stage_key(db, StageEntity::Order, st).await?),
                None => None,
            };
            let filter = OrderFilter {
                pattern: s(args, "query").and_then(like_pattern),
                plate_pattern: String::new(),
                stage_key: stage,
                partner_id: None,
                project_type_id: None,
                assigned_to: None,
                open_only: b(args, "open_only"),
            };
            let rows = orders::search(db, &filter, orders::DEFAULT_SORT, ROWS, 0)
                .await
                .map_err(err)?;
            Ok(json!(rows.iter().map(|o| json!({
                "id": o.id, "number": o.number, "title": o.title, "partner": o.partner_name,
                "stage": o.stage_label, "total": format!("{} {}", o.total_minor / 100, o.currency),
                "due": o.due_date, "plate": o.vehicle_plate,
            })).collect::<Vec<_>>()))
        }
        "get_record" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                .ok_or("az id kötelező szám")?;
            match s(args, "kind") {
                Some("lead") => {
                    let lead = leads::find(db, id)
                        .await
                        .map_err(err)?
                        .ok_or("nincs ilyen lead")?;
                    let stage: Option<String> = sqlx::query_scalar(
                        "SELECT sd.label_hu FROM lead_current_stage cs
                         JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
                         WHERE cs.lead_id = $1",
                    )
                    .bind(id)
                    .fetch_optional(db)
                    .await
                    .map_err(err)?;
                    let tags: Vec<String> = lead_tags::for_lead(db, id)
                        .await
                        .map_err(err)?
                        .into_iter()
                        .map(|t| t.label)
                        .collect();
                    let mut v = compact(json!(lead));
                    v["stage"] = json!(stage);
                    v["tags"] = json!(tags);
                    Ok(v)
                }
                Some("order") => {
                    let order = orders::find(db, id)
                        .await
                        .map_err(err)?
                        .ok_or("nincs ilyen megrendelés")?;
                    let stage: Option<String> = sqlx::query_scalar(
                        "SELECT sd.label_hu FROM order_current_stage cs
                         JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
                         WHERE cs.order_id = $1",
                    )
                    .bind(id)
                    .fetch_optional(db)
                    .await
                    .map_err(err)?;
                    let mut v = compact(json!(order));
                    v["stage"] = json!(stage);
                    Ok(v)
                }
                Some("partner") => Ok(compact(json!(
                    partners::find(db, id)
                        .await
                        .map_err(err)?
                        .ok_or("nincs ilyen partner")?
                ))),
                _ => Err("a kind lead, order vagy partner".into()),
            }
        }
        "overview" => {
            let per_stage = |view: &str, entity: &str| {
                format!(
                    "SELECT sd.label_hu, count(*) FROM {view} cs
                 JOIN stage_definitions sd ON sd.entity = '{entity}' AND sd.key = cs.stage_key
                 GROUP BY sd.label_hu, sd.position ORDER BY sd.position"
                )
            };
            let lead_stages: Vec<(String, i64)> =
                sqlx::query_as(&per_stage("lead_current_stage", "lead"))
                    .fetch_all(db)
                    .await
                    .map_err(err)?;
            let order_stages: Vec<(String, i64)> =
                sqlx::query_as(&per_stage("order_current_stage", "order"))
                    .fetch_all(db)
                    .await
                    .map_err(err)?;
            let pairs = |rows: Vec<(String, i64)>| rows.into_iter().collect::<BTreeMap<_, _>>();
            Ok(json!({
                "leads_by_stage": pairs(lead_stages),
                "orders_by_stage": pairs(order_stages),
                "invoices_by_list": invoices::bucket_counts(db).await.map_err(err)?
                    .into_iter().map(|c| (c.bucket, c.count)).collect::<BTreeMap<_, _>>(),
                "incoming_invoices_by_list": incoming_invoices::bucket_counts(db).await.map_err(err)?
                    .into_iter().map(|c| (c.bucket, c.count)).collect::<BTreeMap<_, _>>(),
                "newsletter": newsletter_tags::counts(db).await.map_err(err)?,
            }))
        }
        "create_filter" => {
            let filter = create_filter(db, args).await?;
            let v = json!({ "created": filter.label, "path": filter.path });
            if !filters.contains(&filter) {
                filters.push(filter);
            }
            Ok(v)
        }
        "list_incoming_invoices" => {
            let bucket = s(args, "bucket");
            if let Some(b) = bucket
                && !incoming_invoices::BUCKETS.contains(&b)
            {
                return Err(format!(
                    "ismeretlen lista: {b}. Létezők: {}",
                    incoming_invoices::BUCKETS.join(", ")
                ));
            }
            let pattern = s(args, "query").and_then(like_pattern);
            let rows = incoming_invoices::search(db, pattern.as_deref(), bucket, ROWS, 0)
                .await
                .map_err(err)?;
            Ok(json!(
                rows.iter()
                    .map(|i| json!({
                        "id": i.id, "supplier": i.supplier_name, "number": i.invoice_number,
                        "gross": i.gross_amount.map(|g| format!("{} {}", g / 100, i.currency)),
                        "due": i.due_date, "list": i.bucket, "file": i.file_name,
                    }))
                    .collect::<Vec<_>>()
            ))
        }
        "reply" => Ok(json!({ "text": s(args, "text").unwrap_or("") })),
        "expiring_quotes" => {
            let days = args
                .get("days")
                .and_then(Value::as_i64)
                .unwrap_or(7)
                .clamp(1, 90);
            let rows = crate::service::reminders::expiring_quotes(db, local_today(), days, None)
                .await
                .map_err(err)?;
            Ok(json!(rows.iter().map(|q| json!({
                "id": q.lead_id, "title": q.title,
                "partner": q.partner_name.as_deref().or(q.contact_name.as_deref()),
                "assigned": q.assigned_name, "valid_until": q.valid_until, "days_left": q.days_left,
            })).collect::<Vec<_>>()))
        }
        "overdue_invoices" => {
            let partner_id = match s(args, "customer").and_then(like_pattern) {
                Some(pattern) => Some(
                    sqlx::query_scalar::<_, i64>(
                        "SELECT id FROM partners WHERE name ILIKE $1 ORDER BY length(name) LIMIT 1",
                    )
                    .bind(pattern)
                    .fetch_optional(db)
                    .await
                    .map_err(err)?
                    .ok_or("nincs ilyen nevű partner")?,
                ),
                None => None,
            };
            let rows = invoices::overdue(db, local_today(), partner_id, 50)
                .await
                .map_err(err)?;
            Ok(json!(rows.iter().map(|i| json!({
                "id": i.id, "number": i.number, "partner": i.partner_name,
                "gross": format!("{} {}", i.gross_amount / 100, i.currency),
                "due": i.payment_date, "days_overdue": i.days_overdue, "reminders": i.reminders_sent,
            })).collect::<Vec<_>>()))
        }
        "recent_history" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                .ok_or("az id kötelező szám")?;
            let entity = match s(args, "kind") {
                Some("lead") => TimelineEntity::Lead,
                Some("order") => TimelineEntity::Order,
                Some("partner") => TimelineEntity::Partner,
                _ => return Err("a kind lead, order vagy partner".into()),
            };
            let rows = timeline::list(db, entity, id, ROWS).await.map_err(err)?;
            Ok(json!(rows.iter().map(|e| json!({
                "at": e.at.with_timezone(&chrono_tz::Europe::Budapest).format("%Y.%m.%d. %H:%M").to_string(),
                "kind": e.kind, "action": e.action, "who": e.user_name,
                "text": e.text.as_deref().or(e.file_name.as_deref()),
                "changes": e.changes.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()),
            })).collect::<Vec<_>>()))
        }
        other => Err(format!("ismeretlen eszköz: {other}")),
    }
}

// ---------------------------------------------------------------- repair and answers

/// Which list the question is about, from its words.
fn list_in(q: &str) -> Option<&'static str> {
    if q.contains("bejovo") || q.contains("beszallito") {
        Some("incoming_invoices")
    } else if q.contains("feliratkoz") || q.contains("hirlevel") {
        Some("subscribers")
    } else if q.contains("megrendel") || q.contains("rendeles") {
        Some("orders")
    } else if q.contains("lead") || q.contains("erdeklod") || q.contains("ajanlatker") {
        Some("leads")
    } else if q.contains("partner") || q.contains("ugyfel") {
        Some("partners")
    } else {
        None
    }
}

/// An incoming-invoice list named in the question.
fn bucket_in(q: &str) -> Option<&'static str> {
    if q.contains("csakkonyveles") || q.contains("konyvelesben") {
        Some("booking_only")
    } else if q.contains("penztarbizonylat") {
        Some("cash_receipt")
    } else if q.contains("reszlet") || q.contains("reszben") {
        Some("partial")
    } else if q.contains("keszpenz") {
        Some("cash")
    } else if q.contains("dijbeker") {
        Some("open_proforma")
    } else if q.contains("fizetesre")
        || q.contains("nyitott")
        || q.contains("kifizetetlen")
        || q.contains("nemfizet")
        || q.contains("tartoz")
    {
        Some("open_invoice")
    } else if q.contains("kifizetett") || q.contains("atutal") || q.contains("fizetett") {
        Some("transferred")
    } else {
        None
    }
}

fn status_in(q: &str) -> Option<&'static str> {
    if q.contains("leiratkoz") {
        Some("unsubscribed")
    } else if q.contains("megerosit") || q.contains("fuggo") {
        Some("pending")
    } else if q.contains("aktiv") {
        Some("active")
    } else {
        None
    }
}

/// A market (language list) named in the question.
fn market_in(q: &str) -> Option<&'static str> {
    if q.contains("magyar") {
        Some("hu")
    } else if q.contains("roman") {
        Some("ro")
    } else if q.contains("nemet") || q.contains("osztrak") {
        Some("de")
    } else if q.contains("olasz") {
        Some("it")
    } else {
        None
    }
}

/// The first whole number in the question: "a 8-as lead", "#12".
fn number_in(question: &str) -> Option<i64> {
    question
        .split(|c: char| !c.is_ascii_digit())
        .find(|w| !w.is_empty() && w.len() <= 9)
        .and_then(|w| w.parse().ok())
}

const HELP: &str = "Szia! Kérdezz a leadekről, megrendelésekről, bejövő számlákról, vagy kérj szűrt listát, \
    pl. „Mutasd a JEGELVE címkés leadeket”, „Mely ajánlatok járnak le?”, „Ki tartozik?”, „Mi változott a 8-as leaden?” \
    vagy „Szűrő: nyitott megrendelések gyártásban”.";

/// The stage whose label (or key) the question names, longest match first.
async fn stage_in(db: &PgPool, entity: StageEntity, q: &str) -> Option<String> {
    let defs = config::stage_definitions(db, entity).await.ok()?;
    defs.iter()
        .filter(|d| {
            let label = fold(&d.label_hu);
            (label.len() >= 3 && q.contains(&label)) || (d.key.len() >= 4 && q.contains(&d.key))
        })
        .max_by_key(|d| d.label_hu.len())
        .map(|d| d.key.clone())
}

/// The tag whose label the question names, longest match first.
async fn tag_in(db: &PgPool, newsletter: bool, q: &str) -> Option<String> {
    let labels: Vec<String> = if newsletter {
        newsletter_tags::list(db, false)
            .await
            .ok()?
            .into_iter()
            .map(|t| t.label)
            .collect()
    } else {
        lead_tags::list(db, false)
            .await
            .ok()?
            .into_iter()
            .map(|t| t.label)
            .collect()
    };
    labels
        .into_iter()
        .filter(|l| fold(l).len() >= 4 && q.contains(&fold(l)))
        .max_by_key(|l| l.len())
}

/// Words that ask for a list or filter rather than name something to look for.
const FILLER: &[&str] = &[
    "mutasd",
    "mutass",
    "listazd",
    "keresd",
    "keress",
    "meg",
    "a",
    "az",
    "es",
    "is",
    "milyen",
    "melyik",
    "mely",
    "vannak",
    "van",
    "lead",
    "leadek",
    "leadeket",
    "megrendeles",
    "megrendelesek",
    "partner",
    "partnert",
    "partnerek",
    "szuro",
    "szurot",
    "szurd",
    "csinalj",
    "keszits",
    "cimkes",
    "cimke",
    "fazis",
    "fazisban",
    "nyitott",
    "osszes",
    "kerem",
    "nekem",
    "kapcsolatos",
    "ki",
    "mi",
    "hol",
];

/// Keeps only the arguments the question supports and fills in what it plainly says.
/// A small model copies its examples ("JEGELVE" where nobody said it) and garbles
/// Hungarian endings ("gyártásban"); the question itself is the ground truth. `fold`
/// strips spaces and accents, so every check below is on squeezed lower-case letters.
async fn ground(db: &PgPool, tool: &str, args: &mut Value, question: &str) {
    if !args.is_object() {
        *args = json!({});
    }
    let q = fold(question);
    if tool == "create_filter"
        && let Some(list) = list_in(&q)
    {
        args["list"] = json!(list);
    }
    let list = match tool {
        "list_leads" => Some("leads".to_string()),
        "list_orders" => Some("orders".to_string()),
        "list_incoming_invoices" => Some("incoming_invoices".to_string()),
        _ => s(args, "list").map(str::to_string),
    };
    let mut put = |k: &str, v: Option<Value>| {
        let map = args.as_object_mut().expect("an object");
        match v {
            Some(v) => {
                map.insert(k.to_string(), v);
            }
            None => {
                map.remove(k);
            }
        }
    };
    match list.as_deref() {
        Some("leads") => {
            put(
                "stage",
                stage_in(db, StageEntity::Lead, &q).await.map(Value::from),
            );
            put("tag", tag_in(db, false, &q).await.map(Value::from));
            put("market", market_in(&q).map(Value::from));
            put("open_only", q.contains("nyitott").then_some(json!(true)));
        }
        Some("orders") => {
            put(
                "stage",
                stage_in(db, StageEntity::Order, &q).await.map(Value::from),
            );
            put("open_only", q.contains("nyitott").then_some(json!(true)));
        }
        Some("incoming_invoices") => put("bucket", bucket_in(&q).map(Value::from)),
        Some("subscribers") => {
            put("tag", tag_in(db, true, &q).await.map(Value::from));
            put("status", status_in(&q).map(Value::from));
        }
        _ => {}
    }
    // Search text must come from the question, and must not merely repeat the filters.
    if let Some(query) = s(args, "query").map(fold) {
        let named = |k: &str| s(args, k).map(fold).unwrap_or_default();
        let (stage, tag) = (named("stage"), named("tag"));
        let echoes = (!tag.is_empty() && (tag.contains(&query) || query.contains(&tag)))
            || (!stage.is_empty() && query.contains(&stage));
        if query.is_empty() || !q.contains(&query) || FILLER.contains(&query.as_str()) || echoes {
            args.as_object_mut().expect("an object").remove("query");
        }
    }
}

/// The tool for a question whose words leave no doubt, without asking the model at all.
async fn route(db: &PgPool, question: &str) -> Option<(String, Value)> {
    let q = fold(question);
    let list = list_in(&q);
    let greetings = [
        "szia", "hello", "hallo", "jonapot", "udv", "hey", "hi", "koszonom", "koszi",
    ];
    if q.len() <= 12 && greetings.iter().any(|g| q.starts_with(g))
        || q.contains("mittudsz")
        || q.contains("segits")
    {
        return Some(("reply".to_string(), json!({ "text": HELP })));
    }
    let record_kind = |l: Option<&str>| match l {
        Some("leads") => Some("lead"),
        Some("orders") => Some("order"),
        Some("partners") => Some("partner"),
        _ => None,
    };
    // "Mi változott a 8-as leaden?": the record's recent history.
    if ["valtoz", "elozmeny", "tortent", "modosit"]
        .iter()
        .any(|w| q.contains(w))
        && let (Some(id), Some(kind)) = (number_in(question), record_kind(list))
    {
        return Some((
            "recent_history".to_string(),
            json!({ "kind": kind, "id": id }),
        ));
    }
    // "Mely ajánlatok járnak le?"
    if q.contains("ajanlat") && (q.contains("lejar") || q.contains("ervenyes")) {
        return Some(("expiring_quotes".to_string(), json!({})));
    }
    // "Ki tartozik?", "lejárt számlák", "ki nem fizetett számlák".
    if [
        "tartoz",
        "kintlev",
        "fizetetlen",
        "nemfizet",
        "kifizetetlen",
        "kesedelm",
    ]
    .iter()
    .any(|w| q.contains(w))
        || (q.contains("szaml") && (q.contains("lejart") || q.contains("kesik")))
    {
        return Some(("overdue_invoices".to_string(), json!({})));
    }
    // "Mi van a 8-as leaddel?", "#12 megrendelés": one record, by its number.
    if let (Some(id), Some(kind)) = (number_in(question), list) {
        let kind = match kind {
            "leads" => Some("lead"),
            "orders" => Some("order"),
            "partners" => Some("partner"),
            _ => None,
        };
        if let Some(kind) = kind
            && !q.contains("szuro")
        {
            return Some(("get_record".to_string(), json!({ "kind": kind, "id": id })));
        }
    }
    if q.starts_with("szuro")
        || q.contains("szurot")
        || q.contains("szurd")
        || q.contains("nezetet")
    {
        return list.map(|l| ("create_filter".to_string(), json!({ "list": l })));
    }
    if q.contains("fazisonkent")
        || q.contains("osszesit")
        || q.contains("attekint")
        || q.contains("statisztik")
    {
        return Some(("overview".to_string(), json!({})));
    }
    match list {
        Some("incoming_invoices") => Some(("list_incoming_invoices".to_string(), json!({}))),
        Some("leads")
            if stage_in(db, StageEntity::Lead, &q).await.is_some()
                || tag_in(db, false, &q).await.is_some() =>
        {
            Some(("list_leads".to_string(), json!({})))
        }
        Some("orders") if stage_in(db, StageEntity::Order, &q).await.is_some() => {
            Some(("list_orders".to_string(), json!({})))
        }
        _ => None,
    }
}

fn rows_of(v: &Value) -> &[Value] {
    v.as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn txt(v: &Value) -> String {
    match v {
        Value::Null => "—".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The Hungarian name of an invoice list key; stage labels pass through unchanged.
fn bucket_label(key: &str) -> &str {
    match key {
        "to_issue" => "Kiállítandó",
        "issued" => "Kiállítva",
        "paid" => "Fizetve",
        "archived" => "Archiválva",
        "stornoed" => "Sztornózva",
        "storno" => "Sztornó",
        "open_invoice" => "Nyitott számla",
        "open_proforma" => "Nyitott díjbekérő",
        "transferred" => "Átutalva",
        "cash" => "Készpénzes",
        "partial" => "Részteljesítés",
        "cash_receipt" => "Pénztárbizonylat",
        "booking_only" => "Csak könyvelésben létező",
        other => other,
    }
}

/// The answer, written from the tool's real result: the model chose what to look up,
/// it does not get to describe what it found.
fn render(tool: &str, result: &Value) -> String {
    match tool {
        "reply" => txt(&result["text"]),
        "overview" => {
            let line = |title: &str, v: &Value| {
                let parts: Vec<String> = v
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, n)| format!("{}: {n}", bucket_label(k)))
                            .collect()
                    })
                    .unwrap_or_default();
                if parts.is_empty() {
                    format!("{title}: nincs adat")
                } else {
                    format!("{title}: {}", parts.join(", "))
                }
            };
            let nl = &result["newsletter"];
            [
                line("Leadek fázisonként", &result["leads_by_stage"]),
                line("Megrendelések fázisonként", &result["orders_by_stage"]),
                line("Számlák listánként", &result["invoices_by_list"]),
                line(
                    "Bejövő számlák listánként",
                    &result["incoming_invoices_by_list"],
                ),
                format!(
                    "Hírlevél: {} aktív feliratkozó, összesen {}",
                    txt(&nl["active"]),
                    txt(&nl["total"])
                ),
            ]
            .join("\n")
        }
        "search" => {
            let mut out = Vec::new();
            for (key, title) in [
                ("orders", "Megrendelések"),
                ("partners", "Partnerek"),
                ("leads", "Leadek"),
            ] {
                let rows = rows_of(&result[key]);
                if rows.is_empty() {
                    continue;
                }
                out.push(format!("{title}:"));
                for r in rows {
                    let name = r
                        .get("number")
                        .map(|n| format!("{} {}", txt(n), txt(&r["title"])))
                        .or_else(|| r.get("name").map(txt))
                        .unwrap_or_else(|| txt(&r["title"]));
                    out.push(format!("• {name}"));
                }
            }
            if out.is_empty() {
                "Nincs találat.".into()
            } else {
                out.join("\n")
            }
        }
        "list_leads" | "list_orders" | "list_incoming_invoices" => {
            let rows = rows_of(result);
            if rows.is_empty() {
                return "Nincs ilyen tétel.".into();
            }
            let mut out = vec![format!(
                "{} találat{}:",
                rows.len(),
                if rows.len() == 10 {
                    " (az első 10)"
                } else {
                    ""
                }
            )];
            for r in rows {
                out.push(match tool {
                    "list_leads" => format!(
                        "• #{} {} — {} ({})",
                        txt(&r["id"]),
                        txt(&r["title"]),
                        txt(&r["stage"]),
                        r["partner"]
                            .as_str()
                            .or(r["contact"].as_str())
                            .unwrap_or("—")
                    ),
                    "list_orders" => format!(
                        "• {} {} — {} ({}, {})",
                        txt(&r["number"]),
                        txt(&r["title"]),
                        txt(&r["stage"]),
                        txt(&r["partner"]),
                        txt(&r["total"])
                    ),
                    _ => format!(
                        "• {} {} — {}{}",
                        txt(&r["supplier"]),
                        r["number"].as_str().unwrap_or(""),
                        r["gross"].as_str().unwrap_or("összeg nélkül"),
                        r["due"]
                            .as_str()
                            .map(|d| format!(", esedékes {d}"))
                            .unwrap_or_default()
                    ),
                });
            }
            out.join("\n")
        }
        "get_record" => {
            let keys = [
                ("title", "Cím"),
                ("name", "Név"),
                ("number", "Szám"),
                ("stage", "Fázis"),
                ("tags", "Címkék"),
                ("contact_name", "Kapcsolattartó"),
                ("contact_email", "E-mail"),
                ("email", "E-mail"),
                ("contact_phone", "Telefon"),
                ("phone", "Telefon"),
                ("source", "Forrás"),
                ("city", "Város"),
                ("due_date", "Határidő"),
                ("created_at", "Létrehozva"),
            ];
            let lines: Vec<String> = keys
                .iter()
                .filter_map(|(k, label)| {
                    result.get(*k).filter(|v| !v.is_null()).map(|v| {
                        let v = match v {
                            Value::Array(a) => a.iter().map(txt).collect::<Vec<_>>().join(", "),
                            v => txt(v),
                        };
                        format!("{label}: {v}")
                    })
                })
                .collect();
            if lines.is_empty() {
                "Nincs adat.".into()
            } else {
                lines.join("\n")
            }
        }
        "expiring_quotes" => {
            let rows = rows_of(result);
            if rows.is_empty() {
                return "A következő napokban nem jár le árajánlat.".into();
            }
            let mut out = vec![format!("{} árajánlat jár le hamarosan:", rows.len())];
            for r in rows {
                let left = r["days_left"].as_i64().unwrap_or(0);
                let when = match left {
                    0 => "ma lejár".to_string(),
                    d if d < 0 => format!("{} napja lejárt", -d),
                    d => format!("{d} nap múlva"),
                };
                out.push(format!(
                    "• #{} {} — {} ({}, felelős: {})",
                    txt(&r["id"]),
                    txt(&r["title"]),
                    when,
                    r["partner"].as_str().unwrap_or("—"),
                    r["assigned"].as_str().unwrap_or("nincs")
                ));
            }
            out.join("\n")
        }
        "overdue_invoices" => {
            let rows = rows_of(result);
            if rows.is_empty() {
                return "Nincs lejárt, kifizetetlen számla.".into();
            }
            let mut out = vec![format!("{} lejárt, kifizetetlen számla:", rows.len())];
            for r in rows {
                out.push(format!(
                    "• {} — {}, {} ({} napja lejárt, {} emlékeztető)",
                    txt(&r["number"]),
                    txt(&r["partner"]),
                    txt(&r["gross"]),
                    txt(&r["days_overdue"]),
                    txt(&r["reminders"])
                ));
            }
            out.join("\n")
        }
        "recent_history" => {
            let rows = rows_of(result);
            if rows.is_empty() {
                return "Nincs még előzmény ezen a rekordon.".into();
            }
            let mut out = vec!["Legutóbbi események:".to_string()];
            for r in rows {
                let what = match (r["kind"].as_str(), r["action"].as_str()) {
                    (Some("change"), _) => {
                        let fields = r["changes"]
                            .as_array()
                            .map(|a| a.iter().map(txt).collect::<Vec<_>>().join(", "));
                        format!("módosítás: {}", fields.unwrap_or_default())
                    }
                    (Some("stage"), _) => format!("fázis: {}", txt(&r["text"])),
                    (Some("email"), Some("received")) => {
                        format!("beérkező levél: {}", txt(&r["text"]))
                    }
                    (Some("email"), _) => format!("e-mail: {}", txt(&r["text"])),
                    (Some("file"), _) => format!("fájl: {}", txt(&r["text"])),
                    (Some("task"), _) => format!("feladat: {}", txt(&r["text"])),
                    (Some("create"), _) => "létrehozva".to_string(),
                    (_, Some(action)) => r["text"]
                        .as_str()
                        .map(|t| format!("{action}: {t}"))
                        .unwrap_or_else(|| action.to_string()),
                    _ => "esemény".to_string(),
                };
                let who = r["who"]
                    .as_str()
                    .map(|w| format!(" ({w})"))
                    .unwrap_or_default();
                out.push(format!("• {} — {what}{who}", txt(&r["at"])));
            }
            out.join("\n")
        }
        "create_filter" => format!("Elkészítettem a szűrőt: {}.", txt(&result["created"])),
        _ => "Kész.".into(),
    }
}

/// What is left of the question once the list words are gone: something to search for.
fn search_words(question: &str) -> Option<String> {
    let words: Vec<&str> = question
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '@' && c != '.')
        .filter(|w| !w.is_empty() && !FILLER.contains(&fold(w).as_str()))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Runs one grounded tool call and writes the answer; a list answer also brings the
/// filter that opens the same rows. The error is the tool's refusal, in words.
async fn answer(
    db: &PgPool,
    name: &str,
    args: &Value,
    steps: &mut Vec<ToolStep>,
    filters: &mut Vec<FilterSuggestion>,
) -> Result<String, String> {
    let result = run_tool(db, name, args, filters).await;
    steps.push(ToolStep {
        tool: name.to_string(),
        arguments: args.clone(),
        ok: result.is_ok(),
    });
    let value = result?;
    if let Some(f) = filter_args_for(name, args)
        && let Ok(filter) = create_filter(db, &f).await
        && !filters.contains(&filter)
    {
        filters.push(filter);
    }
    Ok(render(name, &value))
}

/// The filter that shows the same rows as a list tool, so every list answer can be opened.
fn filter_args_for(tool: &str, args: &Value) -> Option<Value> {
    let list = match tool {
        "list_leads" => "leads",
        "list_orders" => "orders",
        "list_incoming_invoices" => "incoming_invoices",
        _ => return None,
    };
    let mut f = args.clone();
    f["list"] = json!(list);
    Some(f)
}

pub async fn chat(
    db: &PgPool,
    ai: &AiConfig,
    history: &[ChatMessage],
    today: &str,
) -> AppResult<AssistantReply> {
    let question = history
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.chars().take(2000).collect::<String>())
        .unwrap_or_default();
    let mut messages =
        vec![json!({ "role": "system", "content": system_prompt(db, today).await? })];
    // The last few turns, so "and the open ones?" can lean on the previous question.
    for m in history.iter().rev().take(6).rev() {
        let role = if m.role == "assistant" {
            "assistant"
        } else {
            "user"
        };
        let content: String = m.content.chars().take(1000).collect();
        messages.push(json!({ "role": role, "content": content }));
    }
    let mut steps = Vec::new();
    let mut filters = Vec::new();
    let mut last_error = None;

    // Plain questions skip the model: the right tool is obvious from the words.
    if let Some((name, mut args)) = route(db, &question).await {
        ground(db, &name, &mut args, &question).await;
        let reply = answer(db, &name, &args, &mut steps, &mut filters)
            .await
            .unwrap_or_else(|e| format!("Ezt nem sikerült lekérdeznem: {e}"));
        return Ok(AssistantReply {
            reply,
            filters,
            steps,
        });
    }

    for round in 0..MAX_ROUNDS {
        let message = complete(ai, &messages).await?;
        let content = message.content.clone().unwrap_or_default();
        let mut calls = message.tool_calls.clone();
        if calls.is_empty() {
            calls = calls_in_text(&content);
        }
        // Not even a parseable call: look the question up as plain search instead.
        let Some(mut call) = calls.into_iter().next() else {
            if round == 0 {
                let args =
                    json!({ "query": search_words(&question).unwrap_or_else(|| question.clone()) });
                if let Ok(reply) = answer(db, "search", &args, &mut steps, &mut filters).await {
                    return Ok(AssistantReply {
                        reply,
                        filters,
                        steps,
                    });
                }
            }
            break;
        };
        if call.id.is_empty() {
            call.id = format!("call-{round}");
        }
        let mut args = arguments(&call.function);
        let name = call.function.name.clone();
        ground(db, &name, &mut args, &question).await;
        let result = run_tool(db, &name, &args, &mut filters).await;
        steps.push(ToolStep {
            tool: name.clone(),
            arguments: args.clone(),
            ok: result.is_ok(),
        });
        match result {
            Ok(v) => {
                // A list answer comes with the filter that opens the same rows.
                if let Some(f) = filter_args_for(&name, &args)
                    && let Ok(filter) = create_filter(db, &f).await
                    && !filters.contains(&filter)
                {
                    filters.push(filter);
                }
                return Ok(AssistantReply {
                    reply: render(&name, &v),
                    filters,
                    steps,
                });
            }
            Err(e) => {
                // Once more, with the reason: the model gets to correct its arguments.
                messages
                    .push(json!({ "role": "assistant", "content": content, "tool_calls": [call] }));
                messages.push(json!({ "role": "tool", "tool_call_id": call.id, "content": format!("HIBA: {e}") }));
                last_error = Some(e);
            }
        }
    }
    Ok(AssistantReply {
        reply: match last_error {
            Some(e) => format!("Ezt nem sikerült lekérdeznem: {e}"),
            None => "Erre most nem tudok válaszolni. Kérdezzen a leadekről, megrendelésekről, számlákról, vagy kérjen szűrt listát.".into(),
        },
        filters,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_calls_written_as_text_are_recovered() {
        let calls = calls_in_text(
            "Megnézem.\n<tool_call>\n{\"name\": \"list_leads\", \"arguments\": {\"stage\": \"quoted\"}}\n</tool_call>",
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "list_leads");
        assert_eq!(arguments(&calls[0].function)["stage"], "quoted");
        let bare = calls_in_text("{\"name\": \"overview\", \"arguments\": {}}");
        assert_eq!(bare[0].function.name, "overview");
        assert!(calls_in_text("Nincs ilyen lead.").is_empty());
    }

    #[test]
    fn arguments_come_as_a_string_or_an_object() {
        let f = |a: Value| ModelFunction {
            name: "x".into(),
            arguments: a,
        };
        assert_eq!(arguments(&f(json!("{\"id\": 5}")))["id"], 5);
        assert_eq!(arguments(&f(json!({"id": 5})))["id"], 5);
        assert_eq!(arguments(&f(json!("nem json"))), json!({}));
    }

    #[test]
    fn filters_open_the_right_list_with_encoded_values() {
        let mut p = BTreeMap::new();
        p.insert("q".to_string(), "hűtős furgon".to_string());
        p.insert("open".to_string(), "1".to_string());
        assert_eq!(
            filter_path("leads", &p).unwrap(),
            "/leads?open=1&q=h%C5%B1t%C5%91s%20furgon"
        );
        assert_eq!(
            filter_path("subscribers", &BTreeMap::new()).unwrap(),
            "/marketing"
        );
        assert_eq!(filter_path("spaceships", &BTreeMap::new()), None);
    }
}
