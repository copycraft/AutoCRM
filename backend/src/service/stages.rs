//! Moving leads and orders between stages. The domain decides; this applies it atomically.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;
use crate::domain::media::ImageCategory;
use crate::domain::stage::{
    StageDefinition, StageEntity, TransitionError, TransitionKind, check_transition, find, keys,
};
use crate::error::{AppError, AppResult};
use crate::repo::{audit, config, images, leads, orders, stages};
use crate::service::auth::AuthUser;
use crate::service::automation;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StageChange {
    pub from: String,
    pub to: String,
    pub kind: TransitionKind,
}

/// One manual stage target, with everything the UI needs to present it
/// without reimplementing the transition rules: whether a hand move there is
/// allowed at all, whether it needs a note, and whether its image gates pass.
/// Computed from the same `check_transition` the move itself goes through.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TransitionOption {
    pub stage_key: String,
    pub label_hu: String,
    /// False for targets unreachable by a manual move (lead `won`: conversion only).
    pub manual: bool,
    /// True when the move needs a note (backward or reopen).
    pub requires_note: bool,
    /// False when an image gate between current and target is unmet.
    pub gates_met: bool,
}

fn transition_options(
    entity: StageEntity,
    definitions: &[StageDefinition],
    current_key: &str,
    image_counts: &HashMap<ImageCategory, i64>,
    mileage_in: Option<i32>,
) -> Vec<TransitionOption> {
    definitions
        .iter()
        .filter(|d| {
            d.key != current_key
                && (d.is_active || (entity == StageEntity::Lead && d.key == keys::LEAD_WON))
        })
        .map(|d| {
            let manual = !(entity == StageEntity::Lead && d.key == keys::LEAD_WON);
            let requires_note = matches!(
                check_transition(definitions, current_key, &d.key, None, image_counts),
                Err(TransitionError::NoteRequired)
            );
            let gates_met =
                check_transition(definitions, current_key, &d.key, Some("note"), image_counts)
                    .is_ok()
                    && (entity != StageEntity::Order
                        || !intake_gate_blocks(current_key, d, mileage_in));
            TransitionOption {
                stage_key: d.key.clone(),
                label_hu: d.label_hu.clone(),
                manual,
                requires_note,
                gates_met,
            }
        })
        .collect()
}

/// Whether leaving the current stage is blocked for want of the intake slip (ORD-44).
///
/// In `intake`, moving anywhere but staying requires a recorded mileage — except to an
/// exit stage: cancelling needs no odometer reading, and exit stages skip gates by
/// definition (migration 0002). Shared by the move itself and `/transitions`, so the UI
/// never offers what the move refuses (Q-ORD-3) and cancellation never needs a slip
/// (Q-ORD-4).
fn intake_gate_blocks(current_key: &str, to: &StageDefinition, mileage_in: Option<i32>) -> bool {
    current_key == keys::ORDER_INTAKE
        && to.key != keys::ORDER_INTAKE
        && !to.is_exit
        && mileage_in.is_none()
}

/// Manual targets for an order, evaluated against its image counts.
pub async fn order_transitions(
    state: &AppState,
    order_id: i64,
) -> AppResult<Vec<TransitionOption>> {
    let order = orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let definitions = config::stage_definitions(&state.db, StageEntity::Order).await?;
    let current = stages::current_order_stage(&state.db, order_id)
        .await?
        .ok_or_else(|| AppError::internal(format!("order {order_id} has no stage history")))?;
    let image_counts = images::count_by_category(&state.db, order_id).await?;
    Ok(transition_options(
        StageEntity::Order,
        &definitions,
        &current.stage_key,
        &image_counts,
        order.mileage_in,
    ))
}

/// Manual targets for a lead. `won` is listed with `manual: false`.
pub async fn lead_transitions(db: &PgPool, lead_id: i64) -> AppResult<Vec<TransitionOption>> {
    leads::find(db, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let definitions = config::stage_definitions(db, StageEntity::Lead).await?;
    let current = stages::current_lead_stage(db, lead_id)
        .await?
        .ok_or_else(|| AppError::internal(format!("lead {lead_id} has no stage history")))?;
    Ok(transition_options(
        StageEntity::Lead,
        &definitions,
        &current.stage_key,
        &HashMap::new(),
        None,
    ))
}

pub async fn change_order_stage(
    state: &AppState,
    user: &AuthUser,
    order_id: i64,
    to: &str,
    note: Option<&str>,
) -> AppResult<StageChange> {
    let mut tx = state.db.begin().await?;
    orders::lock(&mut *tx, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;

    let definitions = config::stage_definitions(&mut *tx, StageEntity::Order).await?;
    let current = stages::current_order_stage(&mut *tx, order_id)
        .await?
        .ok_or_else(|| AppError::internal(format!("order {order_id} has no stage history")))?;
    let image_counts = images::count_by_category(&mut *tx, order_id).await?;

    let kind = check_transition(&definitions, &current.stage_key, to, note, &image_counts)?;
    // Leaving intake closes the intake slip: without a recorded mileage the MEO review
    // and the job sheet have no starting point. Checked here (not in domain) so the
    // pure transition rules stay signature-stable. Runs before the insert, so a refusal
    // writes nothing.
    if intake_gate_blocks(
        &current.stage_key,
        find(&definitions, to).ok_or_else(|| {
            AppError::internal(format!("order stage definition missing for {to}"))
        })?,
        orders::find(&mut *tx, order_id)
            .await?
            .and_then(|o| o.mileage_in),
    ) {
        return Err(AppError::rule(
            "intake_slip_missing",
            "leaving intake requires the intake slip (mileage_in); record it on the order first",
        ));
    }
    let stage_row_id =
        stages::insert_order_stage(&mut *tx, order_id, to, Some(user.user_id), note).await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        order_id,
        "stage_change",
        json!({ "from": current.stage_key, "to": to, "kind": kind, "note": note }),
    )
    .await?;
    // Customers hear about progress, not about rework or cancellation. Entering completed
    // sends the pickup letter instead of the generic stage mail: one letter, not two.
    if to == keys::ORDER_COMPLETED {
        automation::notify_ready_for_pickup(&mut tx, state, order_id, stage_row_id).await?;
    } else if kind == TransitionKind::Forward {
        automation::notify_stage_changed(&mut tx, state, order_id, stage_row_id).await?;
    }
    tx.commit().await?;

    tracing::info!(order_id, from = %current.stage_key, to, ?kind, "order stage changed");
    Ok(StageChange {
        from: current.stage_key,
        to: to.to_string(),
        kind,
    })
}

pub async fn change_lead_stage(
    db: &PgPool,
    user: &AuthUser,
    lead_id: i64,
    to: &str,
    note: Option<&str>,
) -> AppResult<StageChange> {
    if to == keys::LEAD_WON {
        return Err(AppError::rule(
            "use_conversion",
            "a lead is won by converting it to an order",
        ));
    }
    let mut tx = db.begin().await?;
    leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let converted = orders::find_by_lead(&mut *tx, lead_id).await?;
    if !converted.is_empty() {
        let numbers: Vec<&str> = converted.iter().map(|(_, n)| n.as_str()).collect();
        return Err(AppError::rule(
            "lead_converted",
            format!(
                "this lead was converted to order {}; work on the order instead",
                numbers.join(", ")
            ),
        ));
    }

    let definitions = config::stage_definitions(&mut *tx, StageEntity::Lead).await?;
    let current = stages::current_lead_stage(&mut *tx, lead_id)
        .await?
        .ok_or_else(|| AppError::internal(format!("lead {lead_id} has no stage history")))?;
    if find(&definitions, to).is_none_or(|s| s.entity != StageEntity::Lead.as_str()) {
        return Err(AppError::rule(
            "invalid_transition",
            format!("unknown stage '{to}'"),
        ));
    }

    let kind = check_transition(&definitions, &current.stage_key, to, note, &HashMap::new())?;
    stages::insert_lead_stage(&mut *tx, lead_id, to, Some(user.user_id), note).await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead_id,
        "stage_change",
        json!({ "from": current.stage_key, "to": to, "kind": kind, "note": note }),
    )
    .await?;
    tx.commit().await?;
    Ok(StageChange {
        from: current.stage_key,
        to: to.to_string(),
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lead_def(key: &str, position: i32, terminal: bool, exit: bool) -> StageDefinition {
        StageDefinition {
            id: position.into(),
            entity: StageEntity::Lead.as_str().into(),
            key: key.into(),
            label_hu: key.into(),
            position,
            min_images: 0,
            required_image_category: None,
            is_terminal: terminal,
            is_exit: exit,
            stall_after_days: None,
            is_active: true,
        }
    }

    fn lead_pipeline() -> Vec<StageDefinition> {
        vec![
            lead_def("new", 10, false, false),
            lead_def("contacted", 20, false, false),
            lead_def("quoted", 30, false, false),
            lead_def("won", 40, true, false),
            lead_def("lost", 50, true, true),
        ]
    }

    #[test]
    fn won_is_listed_but_never_a_manual_target() {
        let defs = lead_pipeline();
        let opts = transition_options(StageEntity::Lead, &defs, "new", &HashMap::new(), None);
        let won = opts.iter().find(|o| o.stage_key == "won").unwrap();
        assert!(!won.manual);
        assert!(
            opts.iter()
                .filter(|o| o.manual)
                .all(|o| o.stage_key != "won")
        );
    }

    #[test]
    fn forward_needs_no_note_backward_does() {
        let defs = lead_pipeline();
        let from_new = transition_options(StageEntity::Lead, &defs, "new", &HashMap::new(), None);
        assert!(
            from_new.iter().all(|o| !o.requires_note),
            "every move out of the first stage is forward"
        );
        let from_quoted =
            transition_options(StageEntity::Lead, &defs, "quoted", &HashMap::new(), None);
        let back = from_quoted
            .iter()
            .find(|o| o.stage_key == "contacted")
            .unwrap();
        assert!(back.requires_note);
        assert!(back.gates_met);
    }

    fn order_def(key: &str, position: i32, terminal: bool, exit: bool) -> StageDefinition {
        StageDefinition {
            entity: StageEntity::Order.as_str().into(),
            ..lead_def(key, position, terminal, exit)
        }
    }

    // Q-ORD-4: cancelling from intake needs no odometer reading — exit stages skip
    // gates by definition. Q-ORD-3: `/transitions` shares the rule, so the UI never
    // offers what the move refuses.
    #[test]
    fn the_intake_slip_blocks_work_but_never_cancellation() {
        let design = order_def("design", 20, false, false);
        let cancelled = order_def("cancelled", 60, true, true);
        let intake = order_def("intake", 10, false, false);

        assert!(intake_gate_blocks("intake", &design, None));
        assert!(!intake_gate_blocks("intake", &design, Some(120000)));
        assert!(
            !intake_gate_blocks("intake", &cancelled, None),
            "cancelling must not need a mileage reading"
        );
        assert!(!intake_gate_blocks("intake", &intake, None));
        assert!(!intake_gate_blocks("design", &design, None));
    }

    #[test]
    fn transitions_hide_gated_targets_but_offer_cancellation() {
        let defs = vec![
            order_def("intake", 10, false, false),
            order_def("design", 20, false, false),
            order_def("cancelled", 60, true, true),
        ];
        let without_slip =
            transition_options(StageEntity::Order, &defs, "intake", &HashMap::new(), None);
        let design = without_slip
            .iter()
            .find(|o| o.stage_key == "design")
            .unwrap();
        let cancelled = without_slip
            .iter()
            .find(|o| o.stage_key == "cancelled")
            .unwrap();
        assert!(!design.gates_met);
        assert!(cancelled.gates_met);

        let with_slip = transition_options(
            StageEntity::Order,
            &defs,
            "intake",
            &HashMap::new(),
            Some(1),
        );
        let design = with_slip.iter().find(|o| o.stage_key == "design").unwrap();
        assert!(design.gates_met);
    }
}
