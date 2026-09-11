//! Moving leads and orders between stages. The domain decides; this applies it atomically.

use std::collections::HashMap;

use serde::Serialize;
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;
use crate::domain::stage::{StageEntity, TransitionKind, check_transition, find, keys};
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
    // Customers hear about progress, not about rework or cancellation.
    if kind == TransitionKind::Forward {
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
    if let Some((_, number)) = orders::find_by_lead(&mut *tx, lead_id).await? {
        return Err(AppError::rule(
            "lead_converted",
            format!("this lead was converted to order {number}; work on the order instead"),
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
