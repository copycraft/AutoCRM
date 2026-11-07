//! The build questionnaire (0065): its configuration (settings), and the answers on a lead
//! and on an order.
//!
//! Every signed-in user reads the configuration: the forms on both clients are drawn from
//! it. Changing it is `ManageConfiguration`. Nothing is ever deleted — answers are stored
//! under question keys, so a question is switched off rather than removed, and its key and
//! kind never change.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth};
use super::{optional, patch as patch_field, required};
use crate::AppState;
use crate::domain::questionnaire::{Answers, KINDS, Questionnaire};
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::repo::questionnaire::{self as repo, QuestionInput};
use crate::service::questionnaire::{self as spec, LeadSpec, SpecAnswers};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_questionnaire))
        .routes(routes!(create_section))
        .routes(routes!(update_section))
        .routes(routes!(create_question))
        .routes(routes!(update_question))
        .routes(routes!(put_lead_spec))
        .routes(routes!(put_order_spec))
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    key.len() <= 50
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

async fn load(state: &AppState) -> AppResult<Questionnaire> {
    let mut conn = state.db.acquire().await?;
    Ok(repo::load(&mut conn).await?)
}

/// The sections and their questions, in display order, switched-off ones included.
#[utoipa::path(
    get, path = "/config/questionnaire", tag = "configuration",
    responses((status = 200, body = Questionnaire))
)]
async fn get_questionnaire(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Questionnaire>> {
    Ok(Json(load(&state).await?))
}

/// The question a section is to wait on: a live yes/no question in another section, and
/// not one that waits (through any chain) on this section itself.
fn trigger_id(
    qn: &Questionnaire,
    section_id: Option<i64>,
    key: Option<&str>,
) -> AppResult<Option<i64>> {
    let Some(key) = key else {
        return Ok(None);
    };
    let (mut current, q) = qn
        .find(key)
        .ok_or_else(|| AppError::validation("shown_when: no such active question"))?;
    if q.kind != "yes_no" {
        return Err(AppError::validation(
            "shown_when: only a yes/no question can open a section",
        ));
    }
    let id = q.id;
    let Some(section_id) = section_id else {
        return Ok(Some(id));
    };
    let mut seen = std::collections::HashSet::new();
    loop {
        if current.id == section_id {
            return Err(AppError::validation(
                "shown_when: the section would wait on itself",
            ));
        }
        if !seen.insert(current.id) {
            return Ok(Some(id));
        }
        match current.shown_when.as_deref().and_then(|k| qn.find_any(k)) {
            Some((next, _)) => current = next,
            None => return Ok(Some(id)),
        }
    }
}

#[derive(Deserialize, ToSchema)]
struct CreateSection {
    key: String,
    title: String,
    position: Option<i32>,
    /// The key of the yes/no question whose "yes" opens the section; omitted: always asked.
    shown_when: Option<String>,
    /// The project types the section is asked for; omitted or empty: every type.
    #[serde(default)]
    project_type_ids: Vec<i64>,
}

#[utoipa::path(
    post, path = "/config/questionnaire/sections", tag = "configuration",
    request_body = CreateSection,
    responses((status = 201, body = Questionnaire))
)]
async fn create_section(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateSection>,
) -> AppResult<(StatusCode, Json<Questionnaire>)> {
    me.require(Capability::ManageConfiguration)?;
    if !valid_key(&b.key) {
        return Err(AppError::validation(
            "key must be lowercase letters, digits and underscores, starting with a letter",
        ));
    }
    let title = required("title", &b.title)?;
    let mut tx = state.db.begin().await?;
    let qn = repo::load(&mut tx).await?;
    let trigger = trigger_id(&qn, None, optional(b.shown_when).as_deref())?;
    let position = b
        .position
        .unwrap_or_else(|| qn.sections.iter().map(|s| s.position).max().unwrap_or(0) + 10);
    let id = repo::insert_section(&mut *tx, &b.key, &title, position, trigger).await?;
    repo::set_section_types(&mut tx, id, &b.project_type_ids).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "spec_section",
        id,
        "create",
        json!({ "key": b.key, "title": title }),
    )
    .await?;
    let qn = repo::load(&mut tx).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(qn)))
}

#[derive(Deserialize, ToSchema)]
struct PatchSection {
    title: Option<String>,
    position: Option<i32>,
    is_active: Option<bool>,
    /// Absent keeps it; null: always asked.
    #[serde(default, deserialize_with = "patch_field")]
    shown_when: Option<Option<String>>,
    /// Replaces the list; empty: every type.
    project_type_ids: Option<Vec<i64>>,
}

#[utoipa::path(
    patch, path = "/config/questionnaire/sections/{id}", tag = "configuration",
    params(("id" = i64, Path)),
    request_body = PatchSection,
    responses((status = 200, body = Questionnaire))
)]
async fn update_section(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchSection>,
) -> AppResult<Json<Questionnaire>> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let qn = repo::load(&mut tx).await?;
    let current = qn
        .sections
        .iter()
        .find(|s| s.id == id)
        .ok_or(AppError::NotFound("section"))?;
    let title = required("title", &p.title.unwrap_or_else(|| current.title.clone()))?;
    let shown_when = match p.shown_when {
        Some(v) => optional(v),
        None => current.shown_when.clone(),
    };
    let trigger = trigger_id(&qn, Some(id), shown_when.as_deref())?;
    repo::update_section(
        &mut *tx,
        id,
        &title,
        p.position.unwrap_or(current.position),
        p.is_active.unwrap_or(current.is_active),
        trigger,
    )
    .await?;
    if let Some(types) = &p.project_type_ids {
        repo::set_section_types(&mut tx, id, types).await?;
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "spec_section",
        id,
        "update",
        audit::diff(&[
            ("title", json!(current.title), json!(title)),
            ("shown_when", json!(current.shown_when), json!(shown_when)),
            (
                "project_type_ids",
                json!(current.project_type_ids),
                json!(
                    p.project_type_ids
                        .clone()
                        .unwrap_or(current.project_type_ids.clone())
                ),
            ),
        ]),
    )
    .await?;
    let qn = repo::load(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(qn))
}

/// Trimmed, non-empty, each once; required for the choice kinds and refused for the rest.
fn clean_options(kind: &str, options: Vec<String>) -> AppResult<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for o in options {
        let o = o.trim().to_string();
        if o.chars().count() > 200 {
            return Err(AppError::validation("options: at most 200 characters each"));
        }
        if !o.is_empty() && !out.contains(&o) {
            out.push(o);
        }
    }
    let choice = matches!(kind, "choice" | "multi_choice");
    if choice && out.is_empty() {
        return Err(AppError::validation(
            "options: a choice needs at least one option",
        ));
    }
    if !choice && !out.is_empty() {
        return Err(AppError::validation("options: only a choice has options"));
    }
    Ok(out)
}

fn check_range(kind: &str, min: Option<f64>, max: Option<f64>) -> AppResult<()> {
    if kind != "number" && (min.is_some() || max.is_some()) {
        return Err(AppError::validation("min_value: only a number has a range"));
    }
    if [min, max].into_iter().flatten().any(|v| !v.is_finite()) {
        return Err(AppError::validation("min_value: must be a number"));
    }
    if let (Some(min), Some(max)) = (min, max)
        && min > max
    {
        return Err(AppError::validation(
            "min_value: must not be above max_value",
        ));
    }
    Ok(())
}

#[derive(Deserialize, ToSchema)]
struct CreateQuestion {
    section_id: i64,
    /// The answer's name; lowercase letters, digits and underscores. Fixed once created.
    key: String,
    label: String,
    help: Option<String>,
    /// `yes_no` | `number` | `text` | `long_text` | `choice` | `multi_choice`. Fixed once
    /// created.
    kind: String,
    unit: Option<String>,
    #[serde(default)]
    options: Vec<String>,
    min_value: Option<f64>,
    max_value: Option<f64>,
    #[serde(default)]
    required: bool,
    position: Option<i32>,
}

#[utoipa::path(
    post, path = "/config/questionnaire/questions", tag = "configuration",
    request_body = CreateQuestion,
    responses((status = 201, body = Questionnaire))
)]
async fn create_question(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateQuestion>,
) -> AppResult<(StatusCode, Json<Questionnaire>)> {
    me.require(Capability::ManageConfiguration)?;
    if !valid_key(&b.key) {
        return Err(AppError::validation(
            "key must be lowercase letters, digits and underscores, starting with a letter",
        ));
    }
    if !KINDS.contains(&b.kind.as_str()) {
        return Err(AppError::validation(format!(
            "kind must be one of {}",
            KINDS.join(", ")
        )));
    }
    let label = required("label", &b.label)?;
    let options = clean_options(&b.kind, b.options)?;
    check_range(&b.kind, b.min_value, b.max_value)?;
    let mut tx = state.db.begin().await?;
    let qn = repo::load(&mut tx).await?;
    let section = qn
        .sections
        .iter()
        .find(|s| s.id == b.section_id)
        .ok_or_else(|| AppError::validation("section_id: no such section"))?;
    let position = b.position.unwrap_or_else(|| {
        section
            .questions
            .iter()
            .map(|q| q.position)
            .max()
            .unwrap_or(0)
            + 10
    });
    let help = optional(b.help);
    let unit = optional(b.unit);
    let id = repo::insert_question(
        &mut *tx,
        &b.key,
        &b.kind,
        &QuestionInput {
            section_id: b.section_id,
            label: &label,
            help: help.as_deref(),
            unit: unit.as_deref(),
            options: &options,
            min_value: b.min_value,
            max_value: b.max_value,
            required: b.required,
            position,
            is_active: true,
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "spec_question",
        id,
        "create",
        json!({ "key": b.key, "kind": b.kind, "label": label }),
    )
    .await?;
    let qn = repo::load(&mut tx).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(qn)))
}

#[derive(Deserialize, ToSchema)]
struct PatchQuestion {
    /// Moves the question to another section.
    section_id: Option<i64>,
    label: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    help: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    unit: Option<Option<String>>,
    /// Replaces the choices. Answers already given keep their old choice.
    options: Option<Vec<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    min_value: Option<Option<f64>>,
    #[serde(default, deserialize_with = "patch_field")]
    max_value: Option<Option<f64>>,
    required: Option<bool>,
    position: Option<i32>,
    is_active: Option<bool>,
}

#[utoipa::path(
    patch, path = "/config/questionnaire/questions/{id}", tag = "configuration",
    params(("id" = i64, Path)),
    request_body = PatchQuestion,
    responses((status = 200, body = Questionnaire))
)]
async fn update_question(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(p): ApiJson<PatchQuestion>,
) -> AppResult<Json<Questionnaire>> {
    me.require(Capability::ManageConfiguration)?;
    let mut tx = state.db.begin().await?;
    let qn = repo::load(&mut tx).await?;
    let current = qn
        .sections
        .iter()
        .flat_map(|s| s.questions.iter())
        .find(|q| q.id == id)
        .ok_or(AppError::NotFound("question"))?
        .clone();
    let section_id = p.section_id.unwrap_or(current.section_id);
    if !qn.sections.iter().any(|s| s.id == section_id) {
        return Err(AppError::validation("section_id: no such section"));
    }
    let is_active = p.is_active.unwrap_or(current.is_active);
    // Sections that wait on this question: switching it off would close them for good, and
    // moving it could make one wait on itself.
    for waiting in qn
        .sections
        .iter()
        .filter(|s| s.is_active && s.shown_when.as_deref() == Some(current.key.as_str()))
    {
        if !is_active {
            return Err(AppError::validation(format!(
                "is_active: the section \"{}\" opens on this question; change that first",
                waiting.title
            )));
        }
        if section_id == waiting.id {
            return Err(AppError::validation(
                "section_id: the section would wait on itself",
            ));
        }
    }
    let label = required("label", &p.label.unwrap_or_else(|| current.label.clone()))?;
    let options = clean_options(&current.kind, p.options.unwrap_or(current.options.clone()))?;
    let min_value = p.min_value.unwrap_or(current.min_value);
    let max_value = p.max_value.unwrap_or(current.max_value);
    check_range(&current.kind, min_value, max_value)?;
    let help = match p.help {
        Some(v) => optional(v),
        None => current.help.clone(),
    };
    let unit = match p.unit {
        Some(v) => optional(v),
        None => current.unit.clone(),
    };
    repo::update_question(
        &mut *tx,
        id,
        &QuestionInput {
            section_id,
            label: &label,
            help: help.as_deref(),
            unit: unit.as_deref(),
            options: &options,
            min_value,
            max_value,
            required: p.required.unwrap_or(current.required),
            position: p.position.unwrap_or(current.position),
            is_active,
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "spec_question",
        id,
        "update",
        audit::diff(&[
            ("label", json!(current.label), json!(label)),
            ("section_id", json!(current.section_id), json!(section_id)),
            ("options", json!(current.options), json!(options)),
            (
                "required",
                json!(current.required),
                json!(p.required.unwrap_or(current.required)),
            ),
            ("is_active", json!(current.is_active), json!(is_active)),
        ]),
    )
    .await?;
    let qn = repo::load(&mut tx).await?;
    tx.commit().await?;
    Ok(Json(qn))
}

#[derive(Deserialize, ToSchema)]
struct LeadSpecBody {
    /// The project type the questions are asked for; null: not chosen yet.
    project_type_id: Option<i64>,
    /// Question key → answer. Replaces the lead's answers; answers to questions not asked
    /// for this type and these answers are left out.
    #[serde(default)]
    #[schema(value_type = Object)]
    answers: Answers,
}

/// A lead's project type and questionnaire answers, filled while selling. Saving never
/// demands completeness; converting the lead does.
#[utoipa::path(
    put, path = "/leads/{id}/spec", tag = "leads",
    params(("id" = i64, Path)),
    request_body = LeadSpecBody,
    responses((status = 200, body = LeadSpec))
)]
async fn put_lead_spec(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<LeadSpecBody>,
) -> AppResult<Json<LeadSpec>> {
    me.require(Capability::EditLeads)?;
    let mut tx = state.db.begin().await?;
    let saved = spec::save_lead(&mut tx, me.user_id, id, b.project_type_id, &b.answers).await?;
    tx.commit().await?;
    Ok(Json(saved))
}

/// Replaces an order's questionnaire answers, asked for the order's project type.
#[utoipa::path(
    put, path = "/orders/{id}/spec", tag = "orders",
    params(("id" = i64, Path)),
    request_body = SpecAnswers,
    responses((status = 200, body = SpecAnswers))
)]
async fn put_order_spec(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<SpecAnswers>,
) -> AppResult<Json<SpecAnswers>> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let answers = spec::save_order(&mut tx, me.user_id, id, &b.answers).await?;
    tx.commit().await?;
    Ok(Json(SpecAnswers { answers }))
}
