//! The build questionnaire (műszaki kérdőív, 0065): which questions a job is asked, given its
//! project type and the answers so far, which of them are still missing, and whether an
//! answer is well-formed.
//!
//! Sections of questions are configuration. A section belongs to some project types (none
//! listed: every type) and is either always asked, or only once a yes/no question is answered
//! yes: "heats" opens the heating section, "cools" the cooling one, "freezes" (inside
//! cooling) the freezer one. A section waiting on a question is asked only while that
//! question's own section is asked, so switching cooling off also closes the freezer.
//!
//! Answers are one flat JSON object keyed by question key. Both clients mirror
//! [`Questionnaire::visible_sections`] and [`Questionnaire::missing`] to draw the form and
//! the convert dialog; the server is the one that decides.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{Map, Number, Value};

pub type Answers = Map<String, Value>;

/// Every question kind, as stored in `spec_questions.kind`.
pub const KINDS: &[&str] = &[
    "yes_no",
    "number",
    "text",
    "long_text",
    "choice",
    "multi_choice",
];

/// Longest answers the free-text kinds take.
pub const TEXT_MAX: usize = 500;
pub const LONG_TEXT_MAX: usize = 5000;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Question {
    pub id: i64,
    pub section_id: i64,
    /// The answer's name in the answers object. Never changes.
    pub key: String,
    pub label: String,
    /// One line under the field: an example, or what to measure.
    pub help: Option<String>,
    /// `yes_no` | `number` | `text` | `long_text` | `choice` | `multi_choice`.
    pub kind: String,
    /// Shown next to a number: °C, mm, kW.
    pub unit: Option<String>,
    /// The choices of a `choice` / `multi_choice`, as shown and as stored.
    pub options: Vec<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    /// Has to be answered before the lead may become an order.
    pub required: bool,
    pub position: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Section {
    pub id: i64,
    pub key: String,
    pub title: String,
    pub position: i32,
    pub is_active: bool,
    /// The key of the yes/no question whose "yes" opens this section; null: always asked.
    pub shown_when: Option<String>,
    /// The project types this section is asked for; empty: every type.
    pub project_type_ids: Vec<i64>,
    /// In display order, inactive ones included (settings shows them; forms skip them).
    pub questions: Vec<Question>,
}

/// The whole configuration, sections in display order.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct Questionnaire {
    pub sections: Vec<Section>,
}

/// Why an answer was refused: the question's key and what is wrong with the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerError {
    pub key: String,
    pub message: String,
}

impl std::fmt::Display for AnswerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "answers.{}: {}", self.key, self.message)
    }
}

fn err(q: &Question, message: impl Into<String>) -> AnswerError {
    AnswerError {
        key: q.key.clone(),
        message: message.into(),
    }
}

/// Whether a stored value counts as an answer. `false` is an answer; an empty string, an
/// empty list or null is not.
pub fn is_answered(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    }
}

impl Questionnaire {
    /// An active question by key, with its section.
    pub fn find(&self, key: &str) -> Option<(&Section, &Question)> {
        self.sections.iter().find_map(|s| {
            s.questions
                .iter()
                .find(|q| q.key == key && q.is_active)
                .map(|q| (s, q))
        })
    }

    /// Any question by key, active or not.
    pub fn find_any(&self, key: &str) -> Option<(&Section, &Question)> {
        self.sections
            .iter()
            .find_map(|s| s.questions.iter().find(|q| q.key == key).map(|q| (s, q)))
    }

    fn applies(section: &Section, project_type_id: Option<i64>) -> bool {
        section.is_active
            && (section.project_type_ids.is_empty()
                || project_type_id.is_some_and(|t| section.project_type_ids.contains(&t)))
    }

    /// Whether this section is asked for a job of this type with these answers.
    pub fn is_visible(
        &self,
        section: &Section,
        project_type_id: Option<i64>,
        answers: &Answers,
    ) -> bool {
        self.visible_inner(section, project_type_id, answers, &mut HashSet::new())
    }

    fn visible_inner(
        &self,
        section: &Section,
        project_type_id: Option<i64>,
        answers: &Answers,
        seen: &mut HashSet<i64>,
    ) -> bool {
        // A chain that comes back to itself is never asked rather than looping.
        if !Self::applies(section, project_type_id) || !seen.insert(section.id) {
            return false;
        }
        match &section.shown_when {
            None => true,
            Some(key) => match self.find(key) {
                Some((trigger_section, q)) if q.kind == "yes_no" => {
                    answers.get(key) == Some(&Value::Bool(true))
                        && self.visible_inner(trigger_section, project_type_id, answers, seen)
                }
                _ => false,
            },
        }
    }

    /// The sections asked, in display order.
    pub fn visible_sections(&self, project_type_id: Option<i64>, answers: &Answers) -> Vec<&Section> {
        self.sections
            .iter()
            .filter(|s| self.is_visible(s, project_type_id, answers))
            .collect()
    }

    /// The required questions still unanswered among those asked, in display order.
    pub fn missing(&self, project_type_id: Option<i64>, answers: &Answers) -> Vec<&Question> {
        self.visible_sections(project_type_id, answers)
            .into_iter()
            .flat_map(|s| s.questions.iter())
            .filter(|q| q.is_active && q.required && !is_answered(answers.get(&q.key)))
            .collect()
    }

    /// The answers as they are kept: every value checked against its question and
    /// normalised, empty ones dropped, and answers to questions not asked (another type's
    /// section, a closed section, a switched-off or unknown question) left out. `previous`
    /// is what was stored before: a choice the office has since renamed stays acceptable on
    /// the record that already had it.
    pub fn clean(
        &self,
        project_type_id: Option<i64>,
        input: &Answers,
        previous: &Answers,
    ) -> Result<Answers, AnswerError> {
        // The yes/no answers first: they decide which sections are asked.
        let mut switches = Answers::new();
        for (key, value) in input {
            if let Some((_, q)) = self.find(key)
                && q.kind == "yes_no"
                && let Some(v) = yes_no(q, value)?
            {
                switches.insert(key.clone(), Value::Bool(v));
            }
        }
        let mut out = Answers::new();
        for section in self.visible_sections(project_type_id, &switches) {
            for q in section.questions.iter().filter(|q| q.is_active) {
                let Some(value) = input.get(&q.key) else {
                    continue;
                };
                if let Some(v) = normalize(q, value, previous.get(&q.key))? {
                    out.insert(q.key.clone(), v);
                }
            }
        }
        Ok(out)
    }
}

fn yes_no(q: &Question, value: &Value) -> Result<Option<bool>, AnswerError> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        Value::String(s) if s.trim().is_empty() => Ok(None),
        _ => Err(err(q, "must be true or false")),
    }
}

/// A finite number from a JSON number or a typed string ("12,5" from a Hungarian keyboard).
fn number(q: &Question, value: &Value) -> Result<Option<f64>, AnswerError> {
    let n = match value {
        Value::Null => return Ok(None),
        Value::Number(n) => n.as_f64(),
        Value::String(s) if s.trim().is_empty() => return Ok(None),
        Value::String(s) => s.trim().replace(',', ".").parse::<f64>().ok(),
        _ => None,
    };
    match n {
        Some(n) if n.is_finite() => Ok(Some(n)),
        _ => Err(err(q, "must be a number")),
    }
}

/// Whole numbers are kept as integers, so 1200 does not come back as 1200.0.
fn number_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        Number::from_f64(n).map(Value::Number).unwrap_or(Value::Null)
    }
}

fn text(q: &Question, value: &Value, max: usize) -> Result<Option<String>, AnswerError> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                Ok(None)
            } else if s.chars().count() > max {
                Err(err(q, format!("at most {max} characters")))
            } else {
                Ok(Some(s.to_string()))
            }
        }
        _ => Err(err(q, "must be text")),
    }
}

fn allowed(q: &Question, option: &str, previous: Option<&Value>) -> bool {
    q.options.iter().any(|o| o == option)
        || match previous {
            Some(Value::String(p)) => p == option,
            Some(Value::Array(a)) => a.iter().any(|p| p.as_str() == Some(option)),
            _ => false,
        }
}

fn normalize(
    q: &Question,
    value: &Value,
    previous: Option<&Value>,
) -> Result<Option<Value>, AnswerError> {
    Ok(match q.kind.as_str() {
        "yes_no" => yes_no(q, value)?.map(Value::Bool),
        "number" => match number(q, value)? {
            None => None,
            Some(n) => {
                let below = q.min_value.is_some_and(|min| n < min);
                let above = q.max_value.is_some_and(|max| n > max);
                if below || above {
                    let range = match (q.min_value, q.max_value) {
                        (Some(min), Some(max)) => format!("must be between {min} and {max}"),
                        (Some(min), None) => format!("must be at least {min}"),
                        (None, Some(max)) => format!("must be at most {max}"),
                        (None, None) => unreachable!(),
                    };
                    return Err(err(q, range));
                }
                Some(number_value(n))
            }
        },
        "text" => text(q, value, TEXT_MAX)?.map(Value::String),
        "long_text" => text(q, value, LONG_TEXT_MAX)?.map(Value::String),
        "choice" => match text(q, value, TEXT_MAX)? {
            None => None,
            Some(s) if allowed(q, &s, previous) => Some(Value::String(s)),
            Some(_) => {
                return Err(err(
                    q,
                    format!("must be one of: {}", q.options.join(", ")),
                ));
            }
        },
        "multi_choice" => match value {
            Value::Null => None,
            Value::Array(items) => {
                let mut picked: Vec<String> = Vec::new();
                for item in items {
                    let Some(s) = item.as_str().map(str::trim) else {
                        return Err(err(q, "must be a list of choices"));
                    };
                    if s.is_empty() || picked.iter().any(|p| p == s) {
                        continue;
                    }
                    if !allowed(q, s, previous) {
                        return Err(err(
                            q,
                            format!("each must be one of: {}", q.options.join(", ")),
                        ));
                    }
                    picked.push(s.to_string());
                }
                (!picked.is_empty())
                    .then(|| Value::Array(picked.into_iter().map(Value::String).collect()))
            }
            _ => return Err(err(q, "must be a list of choices")),
        },
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const BOX: i64 = 1;
    const VAN: i64 = 2;

    fn q(id: i64, section_id: i64, key: &str, kind: &str, required: bool) -> Question {
        Question {
            id,
            section_id,
            key: key.into(),
            label: key.into(),
            help: None,
            kind: kind.into(),
            unit: None,
            options: if kind.contains("choice") {
                vec!["A".into(), "B".into()]
            } else {
                vec![]
            },
            min_value: None,
            max_value: None,
            required,
            position: id as i32,
            is_active: true,
        }
    }

    fn section(id: i64, key: &str, shown_when: Option<&str>, types: &[i64], questions: Vec<Question>) -> Section {
        Section {
            id,
            key: key.into(),
            title: key.into(),
            position: id as i32,
            is_active: true,
            shown_when: shown_when.map(Into::into),
            project_type_ids: types.to_vec(),
            questions,
        }
    }

    /// The seeded shape, cut down: general switches, a box-only section, and cooling →
    /// freezer, heating behind their switches.
    fn sample() -> Questionnaire {
        let mut temp = q(30, 3, "cool_temp_c", "number", true);
        temp.min_value = Some(-40.0);
        temp.max_value = Some(30.0);
        Questionnaire {
            sections: vec![
                section(1, "general", None, &[], vec![
                    q(10, 1, "heats", "yes_no", true),
                    q(11, 1, "cools", "yes_no", true),
                    q(12, 1, "notes", "long_text", false),
                ]),
                section(2, "box", None, &[BOX], vec![q(20, 2, "box_length_mm", "number", true)]),
                section(3, "cooling", Some("cools"), &[], vec![
                    temp,
                    q(31, 3, "freezes", "yes_no", true),
                    q(32, 3, "defrost", "choice", false),
                ]),
                section(4, "freezer", Some("freezes"), &[], vec![q(40, 4, "freeze_temp_c", "number", true)]),
                section(5, "heating", Some("heats"), &[], vec![
                    q(50, 5, "heat_temp_c", "number", true),
                    q(51, 5, "extras", "multi_choice", false),
                ]),
            ],
        }
    }

    fn answers(v: Value) -> Answers {
        v.as_object().cloned().unwrap()
    }

    fn keys(sections: Vec<&Section>) -> Vec<&str> {
        sections.into_iter().map(|s| s.key.as_str()).collect()
    }

    fn missing_keys(qn: &Questionnaire, pt: Option<i64>, a: &Answers) -> Vec<String> {
        qn.missing(pt, a).into_iter().map(|q| q.key.clone()).collect()
    }

    #[test]
    fn switches_open_their_sections_and_both_can_be_on() {
        let qn = sample();
        let none = answers(json!({}));
        assert_eq!(keys(qn.visible_sections(Some(BOX), &none)), ["general", "box"]);
        let both = answers(json!({"heats": true, "cools": true}));
        assert_eq!(
            keys(qn.visible_sections(Some(BOX), &both)),
            ["general", "box", "cooling", "heating"]
        );
        let freezer = answers(json!({"cools": true, "freezes": true}));
        assert_eq!(
            keys(qn.visible_sections(Some(VAN), &freezer)),
            ["general", "cooling", "freezer"]
        );
    }

    #[test]
    fn closing_cooling_closes_the_freezer_behind_it() {
        let qn = sample();
        let a = answers(json!({"cools": false, "freezes": true}));
        assert_eq!(keys(qn.visible_sections(Some(BOX), &a)), ["general", "box"]);
    }

    #[test]
    fn type_sections_wait_for_a_type() {
        let qn = sample();
        assert_eq!(keys(qn.visible_sections(None, &Answers::new())), ["general"]);
    }

    #[test]
    fn missing_lists_only_required_questions_that_are_asked() {
        let qn = sample();
        assert_eq!(
            missing_keys(&qn, Some(BOX), &Answers::new()),
            ["heats", "cools", "box_length_mm"]
        );
        let a = answers(json!({"heats": false, "cools": true, "box_length_mm": 4200}));
        assert_eq!(missing_keys(&qn, Some(BOX), &a), ["cool_temp_c", "freezes"]);
        let a = answers(json!({
            "heats": false, "cools": true, "box_length_mm": 4200,
            "cool_temp_c": 2, "freezes": true
        }));
        assert_eq!(missing_keys(&qn, Some(BOX), &a), ["freeze_temp_c"]);
    }

    #[test]
    fn no_is_an_answer_and_blank_is_not() {
        let qn = sample();
        let a = answers(json!({"heats": false, "cools": false, "box_length_mm": ""}));
        assert_eq!(missing_keys(&qn, Some(BOX), &a), ["box_length_mm"]);
    }

    #[test]
    fn clean_drops_hidden_unknown_and_empty_answers() {
        let qn = sample();
        let input = answers(json!({
            "heats": false,
            "cools": true,
            "heat_temp_c": 20,
            "box_length_mm": "4200",
            "nonsense": 1,
            "notes": "  ",
            "cool_temp_c": "-18,5",
        }));
        let out = qn.clean(Some(VAN), &input, &Answers::new()).unwrap();
        assert_eq!(
            Value::Object(out),
            json!({"heats": false, "cools": true, "cool_temp_c": -18.5})
        );
    }

    #[test]
    fn whole_numbers_stay_integers() {
        let qn = sample();
        let out = qn
            .clean(Some(BOX), &answers(json!({"box_length_mm": "4200,0"})), &Answers::new())
            .unwrap();
        assert_eq!(out.get("box_length_mm"), Some(&json!(4200)));
    }

    #[test]
    fn values_are_checked_against_their_question() {
        let qn = sample();
        let none = Answers::new();
        let e = qn
            .clean(None, &answers(json!({"cools": true, "cool_temp_c": -50})), &none)
            .unwrap_err();
        assert_eq!(e.key, "cool_temp_c");
        assert!(e.message.contains("between -40 and 30"), "{}", e.message);
        assert_eq!(
            qn.clean(None, &answers(json!({"heats": "yes"})), &none).unwrap_err().key,
            "heats"
        );
        assert_eq!(
            qn.clean(None, &answers(json!({"cools": true, "defrost": "C"})), &none)
                .unwrap_err()
                .key,
            "defrost"
        );
        assert_eq!(
            qn.clean(None, &answers(json!({"cools": true, "cool_temp_c": "hideg"})), &none)
                .unwrap_err()
                .key,
            "cool_temp_c"
        );
    }

    #[test]
    fn a_renamed_choice_survives_on_the_record_that_had_it() {
        let qn = sample();
        let previous = answers(json!({"defrost": "Régi"}));
        let input = answers(json!({"cools": true, "defrost": "Régi"}));
        let out = qn.clean(None, &input, &previous).unwrap();
        assert_eq!(out.get("defrost"), Some(&json!("Régi")));
    }

    #[test]
    fn multi_choice_keeps_distinct_known_options() {
        let qn = sample();
        let input = answers(json!({"heats": true, "extras": ["A", "B", "A", ""]}));
        let out = qn.clean(None, &input, &Answers::new()).unwrap();
        assert_eq!(out.get("extras"), Some(&json!(["A", "B"])));
        let input = answers(json!({"heats": true, "extras": ["Z"]}));
        assert!(qn.clean(None, &input, &Answers::new()).is_err());
    }

    #[test]
    fn a_hidden_section_is_not_validated() {
        // A stale bad value in a closed section must not block saving the rest.
        let qn = sample();
        let input = answers(json!({"cools": false, "cool_temp_c": "hideg"}));
        let out = qn.clean(None, &input, &Answers::new()).unwrap();
        assert_eq!(Value::Object(out), json!({"cools": false}));
    }

    #[test]
    fn a_section_waiting_on_itself_is_never_asked() {
        let mut qn = sample();
        qn.sections[3].shown_when = Some("freeze_temp_c".into()); // not a yes/no
        let a = answers(json!({"cools": true, "freezes": true}));
        assert_eq!(keys(qn.visible_sections(None, &a)), ["general", "cooling"]);
        let mut qn = sample();
        qn.sections[2].shown_when = Some("freezes".into()); // cooling behind its own question
        assert_eq!(keys(qn.visible_sections(None, &a)), ["general"]);
    }

    #[test]
    fn switched_off_questions_and_sections_are_not_asked() {
        let mut qn = sample();
        qn.sections[1].questions[0].is_active = false;
        qn.sections[4].is_active = false;
        let a = answers(json!({"heats": true, "cools": false}));
        assert_eq!(missing_keys(&qn, Some(BOX), &a), Vec::<String>::new());
        assert_eq!(keys(qn.visible_sections(Some(BOX), &a)), ["general", "box"]);
    }
}
