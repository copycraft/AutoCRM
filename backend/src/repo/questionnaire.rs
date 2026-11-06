//! The build questionnaire (0065): its configuration, and the answers on leads and orders.
//! The rules (what is asked, what is missing, what a valid answer is) are in
//! `domain::questionnaire`; this is storage only.

use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};

use crate::domain::questionnaire::{Answers, Question, Questionnaire, Section};

/// The whole configuration, inactive parts included: settings shows them, and the rules
/// skip them.
pub async fn load(conn: &mut PgConnection) -> sqlx::Result<Questionnaire> {
    let sections = sqlx::query!(
        r#"SELECT s.id, s.key, s.title, s.position, s.is_active, q.key AS "shown_when?"
             FROM spec_sections s
             LEFT JOIN spec_questions q ON q.id = s.shown_when_question_id
            ORDER BY s.position, s.id"#
    )
    .fetch_all(&mut *conn)
    .await?;
    let links = sqlx::query!(
        "SELECT section_id, project_type_id FROM spec_section_project_types
          ORDER BY section_id, project_type_id"
    )
    .fetch_all(&mut *conn)
    .await?;
    let questions = sqlx::query_as!(
        Question,
        r#"SELECT id, section_id, key, label, help, kind, unit, options,
                  min_value::float8 AS "min_value?", max_value::float8 AS "max_value?",
                  required, position, is_active
             FROM spec_questions
            ORDER BY position, id"#
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(Questionnaire {
        sections: sections
            .into_iter()
            .map(|s| Section {
                project_type_ids: links
                    .iter()
                    .filter(|l| l.section_id == s.id)
                    .map(|l| l.project_type_id)
                    .collect(),
                questions: questions
                    .iter()
                    .filter(|q| q.section_id == s.id)
                    .cloned()
                    .collect(),
                id: s.id,
                key: s.key,
                title: s.title,
                position: s.position,
                is_active: s.is_active,
                shown_when: s.shown_when,
            })
            .collect(),
    })
}

pub async fn insert_section(
    db: impl PgExecutor<'_>,
    key: &str,
    title: &str,
    position: i32,
    shown_when_question_id: Option<i64>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO spec_sections (key, title, position, shown_when_question_id)
         VALUES ($1, $2, $3, $4) RETURNING id",
        key,
        title,
        position,
        shown_when_question_id
    )
    .fetch_one(db)
    .await
}

pub async fn update_section(
    db: impl PgExecutor<'_>,
    id: i64,
    title: &str,
    position: i32,
    is_active: bool,
    shown_when_question_id: Option<i64>,
) -> sqlx::Result<bool> {
    let done = sqlx::query!(
        "UPDATE spec_sections
            SET title = $2, position = $3, is_active = $4, shown_when_question_id = $5
          WHERE id = $1",
        id,
        title,
        position,
        is_active,
        shown_when_question_id
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Replaces the project types a section is asked for; none means every type.
pub async fn set_section_types(
    conn: &mut PgConnection,
    section_id: i64,
    project_type_ids: &[i64],
) -> sqlx::Result<()> {
    sqlx::query!(
        "DELETE FROM spec_section_project_types WHERE section_id = $1",
        section_id
    )
    .execute(&mut *conn)
    .await?;
    sqlx::query!(
        "INSERT INTO spec_section_project_types (section_id, project_type_id)
         SELECT $1, t FROM unnest($2::bigint[]) AS t ON CONFLICT DO NOTHING",
        section_id,
        project_type_ids
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// A question as written by settings. Key and kind are fixed once created: answers are
/// stored under the key, in the shape of the kind.
pub struct QuestionInput<'a> {
    pub section_id: i64,
    pub label: &'a str,
    pub help: Option<&'a str>,
    pub unit: Option<&'a str>,
    pub options: &'a [String],
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub required: bool,
    pub position: i32,
    pub is_active: bool,
}

pub async fn insert_question(
    db: impl PgExecutor<'_>,
    key: &str,
    kind: &str,
    q: &QuestionInput<'_>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "INSERT INTO spec_questions
            (section_id, key, kind, label, help, unit, options, min_value, max_value, required,
             position, is_active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::float8::numeric, $9::float8::numeric, $10,
                 $11, $12)
         RETURNING id",
        q.section_id,
        key,
        kind,
        q.label,
        q.help,
        q.unit,
        q.options,
        q.min_value,
        q.max_value,
        q.required,
        q.position,
        q.is_active
    )
    .fetch_one(db)
    .await
}

pub async fn update_question(
    db: impl PgExecutor<'_>,
    id: i64,
    q: &QuestionInput<'_>,
) -> sqlx::Result<bool> {
    let done = sqlx::query!(
        "UPDATE spec_questions
            SET section_id = $2, label = $3, help = $4, unit = $5, options = $6,
                min_value = $7::float8::numeric, max_value = $8::float8::numeric,
                required = $9, position = $10, is_active = $11
          WHERE id = $1",
        id,
        q.section_id,
        q.label,
        q.help,
        q.unit,
        q.options,
        q.min_value,
        q.max_value,
        q.required,
        q.position,
        q.is_active
    )
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

fn object(value: Value) -> Answers {
    match value {
        Value::Object(map) => map,
        _ => Answers::new(),
    }
}

/// A lead's project type and answers; None when there is no such lead.
pub async fn lead_spec(
    db: impl PgExecutor<'_>,
    lead_id: i64,
) -> sqlx::Result<Option<(Option<i64>, Answers)>> {
    Ok(sqlx::query!(
        "SELECT project_type_id, spec_answers FROM leads WHERE id = $1",
        lead_id
    )
    .fetch_optional(db)
    .await?
    .map(|r| (r.project_type_id, object(r.spec_answers))))
}

pub async fn set_lead_spec(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    project_type_id: Option<i64>,
    answers: &Answers,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE leads SET project_type_id = $2, spec_answers = $3, updated_at = now() WHERE id = $1",
        lead_id,
        project_type_id,
        Value::Object(answers.clone())
    )
    .execute(db)
    .await?;
    Ok(())
}

/// An order's answers; empty when it has none (or there is no such order).
pub async fn order_answers(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Answers> {
    Ok(sqlx::query_scalar!(
        "SELECT spec_answers FROM orders WHERE id = $1",
        order_id
    )
    .fetch_optional(db)
    .await?
    .map(object)
    .unwrap_or_default())
}

pub async fn set_order_answers(
    db: impl PgExecutor<'_>,
    order_id: i64,
    answers: &Answers,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE orders SET spec_answers = $2 WHERE id = $1",
        order_id,
        Value::Object(answers.clone())
    )
    .execute(db)
    .await?;
    Ok(())
}
