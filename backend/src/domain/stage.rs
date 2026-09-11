//! Stage transition rules.
//!
//! Stages are configuration (`stage_definitions`); this module decides which moves that
//! configuration allows. Code refers to stage *keys* only — never labels.
//!
//! Rules:
//! - Forward moves may skip stages, but every gate from the current stage up to the
//!   target must be satisfied (so skipping MEO cannot dodge the MEO photo requirement).
//! - Backward moves are allowed (design rework happens) but require a note.
//! - Exit stages (lost, cancelled) are reachable from any open stage without gates.
//! - Leaving a terminal stage is a reopen and requires a note.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::media::ImageCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StageEntity {
    Lead,
    Order,
}

impl StageEntity {
    pub fn as_str(self) -> &'static str {
        match self {
            StageEntity::Lead => "lead",
            StageEntity::Order => "order",
        }
    }
}

/// Stage keys that code depends on. Everything else about stages is configuration.
pub mod keys {
    /// A lead reaches `won` only through conversion to an order.
    pub const LEAD_WON: &str = "won";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct StageDefinition {
    pub id: i64,
    #[schema(value_type = StageEntity)]
    pub entity: String,
    pub key: String,
    pub label_hu: String,
    pub position: i32,
    pub min_images: i32,
    pub required_image_category: Option<ImageCategory>,
    pub is_terminal: bool,
    pub is_exit: bool,
    pub stall_after_days: Option<i32>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Forward,
    Backward,
    Exit,
    Reopen,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransitionError {
    #[error("unknown stage '{0}'")]
    UnknownStage(String),
    #[error("stage '{0}' is not active")]
    InactiveStage(String),
    #[error("already in stage '{0}'")]
    SameStage(String),
    #[error("moving backwards or reopening a finished record requires a note")]
    NoteRequired,
    #[error("stage '{stage}' requires at least {required} '{}' image(s) before moving on; {have} uploaded", category.as_str())]
    GateNotMet {
        stage: String,
        category: ImageCategory,
        required: i32,
        have: i64,
    },
}

pub fn find<'a>(stages: &'a [StageDefinition], key: &str) -> Option<&'a StageDefinition> {
    stages.iter().find(|s| s.key == key)
}

/// Where new records start: the lowest-positioned active, non-exit stage.
pub fn initial_stage(stages: &[StageDefinition]) -> Option<&StageDefinition> {
    stages
        .iter()
        .filter(|s| s.is_active && !s.is_exit && !s.is_terminal)
        .min_by_key(|s| s.position)
}

pub fn check_transition(
    stages: &[StageDefinition],
    from_key: &str,
    to_key: &str,
    note: Option<&str>,
    image_counts: &HashMap<ImageCategory, i64>,
) -> Result<TransitionKind, TransitionError> {
    let to =
        find(stages, to_key).ok_or_else(|| TransitionError::UnknownStage(to_key.to_string()))?;
    if !to.is_active {
        return Err(TransitionError::InactiveStage(to.key.clone()));
    }
    // The current stage may have been deactivated since the record entered it; that's fine.
    let from = find(stages, from_key)
        .ok_or_else(|| TransitionError::UnknownStage(from_key.to_string()))?;
    if from.key == to.key {
        return Err(TransitionError::SameStage(to.key.clone()));
    }

    let has_note = note.is_some_and(|n| !n.trim().is_empty());

    if from.is_terminal {
        return if has_note {
            Ok(TransitionKind::Reopen)
        } else {
            Err(TransitionError::NoteRequired)
        };
    }
    if to.is_exit {
        return Ok(TransitionKind::Exit);
    }
    if to.position < from.position {
        return if has_note {
            Ok(TransitionKind::Backward)
        } else {
            Err(TransitionError::NoteRequired)
        };
    }

    let gates = stages.iter().filter(|s| {
        s.is_active
            && !s.is_exit
            && s.min_images > 0
            && s.position >= from.position
            && s.position < to.position
    });
    for gate in gates {
        let Some(category) = gate.required_image_category else {
            continue;
        };
        let have = image_counts.get(&category).copied().unwrap_or(0);
        if have < i64::from(gate.min_images) {
            return Err(TransitionError::GateNotMet {
                stage: gate.key.clone(),
                category,
                required: gate.min_images,
                have,
            });
        }
    }
    Ok(TransitionKind::Forward)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(key: &str, position: i32) -> StageDefinition {
        StageDefinition {
            id: i64::from(position),
            entity: "order".into(),
            key: key.into(),
            label_hu: key.to_uppercase(),
            position,
            min_images: 0,
            required_image_category: None,
            is_terminal: false,
            is_exit: false,
            stall_after_days: None,
            is_active: true,
        }
    }

    /// The seeded order pipeline: intake · design · production · meo(gate) · completed · cancelled(exit)
    fn order_stages() -> Vec<StageDefinition> {
        let mut meo = stage("meo", 40);
        meo.min_images = 1;
        meo.required_image_category = Some(ImageCategory::Completion);
        let mut completed = stage("completed", 50);
        completed.is_terminal = true;
        let mut cancelled = stage("cancelled", 60);
        cancelled.is_terminal = true;
        cancelled.is_exit = true;
        vec![
            stage("intake", 10),
            stage("design", 20),
            stage("production", 30),
            meo,
            completed,
            cancelled,
        ]
    }

    fn no_images() -> HashMap<ImageCategory, i64> {
        HashMap::new()
    }

    fn with(category: ImageCategory, n: i64) -> HashMap<ImageCategory, i64> {
        HashMap::from([(category, n)])
    }

    #[test]
    fn initial_stage_is_first_open_stage() {
        assert_eq!(initial_stage(&order_stages()).unwrap().key, "intake");
    }

    #[test]
    fn simple_forward_move() {
        let s = order_stages();
        assert_eq!(
            check_transition(&s, "intake", "design", None, &no_images()),
            Ok(TransitionKind::Forward)
        );
    }

    #[test]
    fn meo_gate_blocks_leaving_meo_without_completion_photos() {
        let s = order_stages();
        assert!(matches!(
            check_transition(&s, "meo", "completed", None, &no_images()),
            Err(TransitionError::GateNotMet {
                required: 1,
                have: 0,
                ..
            })
        ));
        // Photos of the wrong category don't count.
        assert!(
            check_transition(
                &s,
                "meo",
                "completed",
                None,
                &with(ImageCategory::Production, 50)
            )
            .is_err()
        );
        assert_eq!(
            check_transition(
                &s,
                "meo",
                "completed",
                None,
                &with(ImageCategory::Completion, 1)
            ),
            Ok(TransitionKind::Forward)
        );
    }

    #[test]
    fn entering_meo_is_not_gated() {
        let s = order_stages();
        assert_eq!(
            check_transition(&s, "production", "meo", None, &no_images()),
            Ok(TransitionKind::Forward)
        );
    }

    #[test]
    fn skipping_meo_does_not_bypass_its_gate() {
        let s = order_stages();
        assert!(matches!(
            check_transition(&s, "production", "completed", None, &no_images()),
            Err(TransitionError::GateNotMet { .. })
        ));
    }

    #[test]
    fn backward_requires_note() {
        let s = order_stages();
        assert_eq!(
            check_transition(&s, "production", "design", None, &no_images()),
            Err(TransitionError::NoteRequired)
        );
        assert_eq!(
            check_transition(&s, "production", "design", Some("  "), &no_images()),
            Err(TransitionError::NoteRequired)
        );
        assert_eq!(
            check_transition(&s, "production", "design", Some("rajz hibás"), &no_images()),
            Ok(TransitionKind::Backward)
        );
    }

    #[test]
    fn exit_is_reachable_from_any_open_stage_without_gates() {
        let s = order_stages();
        assert_eq!(
            check_transition(&s, "meo", "cancelled", None, &no_images()),
            Ok(TransitionKind::Exit)
        );
        assert_eq!(
            check_transition(&s, "intake", "cancelled", None, &no_images()),
            Ok(TransitionKind::Exit)
        );
    }

    #[test]
    fn leaving_terminal_is_a_reopen_needing_a_note() {
        let s = order_stages();
        assert_eq!(
            check_transition(&s, "completed", "production", None, &no_images()),
            Err(TransitionError::NoteRequired)
        );
        assert_eq!(
            check_transition(
                &s,
                "cancelled",
                "intake",
                Some("ügyfél mégis kéri"),
                &no_images()
            ),
            Ok(TransitionKind::Reopen)
        );
    }

    #[test]
    fn same_unknown_and_inactive_targets_are_rejected() {
        let mut s = order_stages();
        assert!(matches!(
            check_transition(&s, "design", "design", None, &no_images()),
            Err(TransitionError::SameStage(_))
        ));
        assert!(matches!(
            check_transition(&s, "design", "nope", None, &no_images()),
            Err(TransitionError::UnknownStage(_))
        ));
        s[1].is_active = false; // design
        assert!(matches!(
            check_transition(&s, "intake", "design", None, &no_images()),
            Err(TransitionError::InactiveStage(_))
        ));
        // ...but a record already sitting in an inactive stage can still move on.
        assert_eq!(
            check_transition(&s, "design", "production", None, &no_images()),
            Ok(TransitionKind::Forward)
        );
    }

    #[test]
    fn deactivated_gate_no_longer_applies() {
        let mut s = order_stages();
        s[3].is_active = false; // meo
        assert_eq!(
            check_transition(&s, "production", "completed", None, &no_images()),
            Ok(TransitionKind::Forward)
        );
    }
}
