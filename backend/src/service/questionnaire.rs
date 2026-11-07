//! The build questionnaire's answers on leads and orders (0065): checking and saving them.
//!
//! Saving never demands completeness: a lead is filled in over several calls. Completeness
//! is asked once, when the lead becomes an order ([`require_complete`], from
//! `service::leads::convert`).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;

use crate::domain::questionnaire::{Answers, Questionnaire};
use crate::error::{AppError, AppResult};
use crate::repo::{audit, leads, orders, questionnaire as repo};

/// A lead's questionnaire: the project type it is asked for, and the answers so far.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct LeadSpec {
    pub project_type_id: Option<i64>,
    /// Question key → answer: `true`/`false`, a number, text, or a list of choices.
    #[schema(value_type = Object)]
    pub answers: Answers,
}

/// An order's answers, as sent and as returned.
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SpecAnswers {
    /// Question key → answer. Answers to questions not asked for this job are left out.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub answers: Answers,
}

/// The answers as kept, or a validation error naming the question.
pub fn clean(
    qn: &Questionnaire,
    project_type_id: Option<i64>,
    input: &Answers,
    previous: &Answers,
) -> AppResult<Answers> {
    qn.clean(project_type_id, input, previous)
        .map_err(|e| AppError::validation(e.to_string()))
}

/// Refuses while a required question that is asked has no answer. The message lists the
/// keys; clients work the list out themselves to highlight the fields.
pub fn require_complete(
    qn: &Questionnaire,
    project_type_id: Option<i64>,
    answers: &Answers,
) -> AppResult<()> {
    let missing = qn.missing(project_type_id, answers);
    if missing.is_empty() {
        return Ok(());
    }
    Err(AppError::rule(
        "spec_incomplete",
        format!(
            "required answers missing: {}",
            missing
                .iter()
                .map(|q| q.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ))
}

fn changes(before: &Answers, after: &Answers) -> Value {
    let mut keys: Vec<&String> = before.keys().chain(after.keys()).collect();
    keys.sort();
    keys.dedup();
    let changed: serde_json::Map<String, Value> = keys
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .map(|k| {
            (
                k.clone(),
                json!({ "from": before.get(k), "to": after.get(k) }),
            )
        })
        .collect();
    Value::Object(changed)
}

/// Replaces a lead's project type and answers.
pub async fn save_lead(
    conn: &mut PgConnection,
    user_id: i64,
    lead_id: i64,
    project_type_id: Option<i64>,
    input: &Answers,
) -> AppResult<LeadSpec> {
    leads::lock(&mut *conn, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let (before_type, before) = repo::lead_spec(&mut *conn, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let qn = repo::load(&mut *conn).await?;
    let answers = clean(&qn, project_type_id, input, &before)?;
    repo::set_lead_spec(&mut *conn, lead_id, project_type_id, &answers).await?;
    let mut diff = changes(&before, &answers);
    if before_type != project_type_id
        && let Value::Object(map) = &mut diff
    {
        map.insert(
            "project_type_id".into(),
            json!({ "from": before_type, "to": project_type_id }),
        );
    }
    if diff.as_object().is_some_and(|m| !m.is_empty()) {
        audit::record(&mut *conn, Some(user_id), "lead", lead_id, "spec_set", diff).await?;
    }
    Ok(LeadSpec {
        project_type_id,
        answers,
    })
}

/// Replaces an order's answers, asked for the order's project type.
pub async fn save_order(
    conn: &mut PgConnection,
    user_id: i64,
    order_id: i64,
    input: &Answers,
) -> AppResult<Answers> {
    let order = orders::lock(&mut *conn, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let before = repo::order_answers(&mut *conn, order_id).await?;
    let qn = repo::load(&mut *conn).await?;
    let answers = clean(&qn, order.project_type_id, input, &before)?;
    repo::set_order_answers(&mut *conn, order_id, &answers).await?;
    let diff = changes(&before, &answers);
    if diff.as_object().is_some_and(|m| !m.is_empty()) {
        audit::record(
            &mut *conn,
            Some(user_id),
            "order",
            order_id,
            "spec_set",
            diff,
        )
        .await?;
    }
    Ok(answers)
}

/// Sets one answer on an order, keeping the others: the cooling unit's serial scanned on
/// the phone. Refused when the question is not asked for this order (no cooling).
pub async fn set_order_answer(
    conn: &mut PgConnection,
    user_id: i64,
    order_id: i64,
    key: &str,
    value: Value,
) -> AppResult<Answers> {
    let order = orders::lock(&mut *conn, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let before = repo::order_answers(&mut *conn, order_id).await?;
    let qn = repo::load(&mut *conn).await?;
    let asked = qn
        .find(key)
        .is_some_and(|(section, _)| qn.is_visible(section, order.project_type_id, &before));
    if !asked {
        return Err(AppError::validation(format!(
            "{key}: not asked for this order"
        )));
    }
    // Only this answer is checked: an old value elsewhere that today's limits would refuse
    // must not stop the phone filing a serial number.
    let mut probe: Answers = before
        .iter()
        .filter(|(_, v)| v.is_boolean())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    probe.insert(key.to_string(), value);
    let checked = clean(&qn, order.project_type_id, &probe, &before)?;
    let mut answers = before.clone();
    match checked.get(key) {
        Some(v) => answers.insert(key.to_string(), v.clone()),
        None => answers.remove(key),
    };
    repo::set_order_answers(&mut *conn, order_id, &answers).await?;
    let diff = changes(&before, &answers);
    if diff.as_object().is_some_and(|m| !m.is_empty()) {
        audit::record(
            &mut *conn,
            Some(user_id),
            "order",
            order_id,
            "spec_set",
            diff,
        )
        .await?;
    }
    Ok(answers)
}
