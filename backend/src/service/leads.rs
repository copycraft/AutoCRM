//! Lead creation and lead → order conversion.

use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;

use crate::domain::stage::{StageEntity, initial_stage, keys};
use crate::error::{AppError, AppResult};
use crate::repo::leads::{Lead, LeadInput};
use crate::repo::orders::{Order, OrderFields};
use crate::repo::{audit, config, contacts, leads, orders, partners, stages};
use crate::service::auth::AuthUser;
use crate::service::orders::{NewItem, create_in_tx};

pub async fn create(db: &PgPool, user: &AuthUser, input: LeadInput) -> AppResult<Lead> {
    let mut tx = db.begin().await?;
    if let Some(partner_id) = input.partner_id {
        partners::find(&mut *tx, partner_id)
            .await?
            .ok_or_else(|| AppError::validation("partner does not exist"))?;
    }
    if let Some(contact_id) = input.contact_id {
        let contact = contacts::find(&mut *tx, contact_id)
            .await?
            .ok_or_else(|| AppError::validation("contact does not exist"))?;
        if Some(contact.partner_id) != input.partner_id {
            return Err(AppError::validation(
                "contact belongs to a different partner",
            ));
        }
    }
    let lead = leads::insert(&mut *tx, &input, user.user_id).await?;
    let definitions = config::stage_definitions(&mut *tx, StageEntity::Lead).await?;
    let initial = initial_stage(&definitions)
        .ok_or_else(|| AppError::internal("no active initial lead stage is configured"))?;
    stages::insert_lead_stage(&mut *tx, lead.id, &initial.key, Some(user.user_id), None).await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead.id,
        "create",
        json!({ "title": lead.title }),
    )
    .await?;
    tx.commit().await?;
    Ok(lead)
}

/// What the office fills in when a lead becomes an order. Partner may be omitted if the
/// lead already has one.
pub struct Conversion {
    pub partner_id: Option<i64>,
    pub fields: OrderFields,
    pub items: Vec<NewItem>,
}

pub async fn convert(
    db: &PgPool,
    user: &AuthUser,
    lead_id: i64,
    today: NaiveDate,
    mut conversion: Conversion,
) -> AppResult<Order> {
    let mut tx = db.begin().await?;
    let lead = leads::lock(&mut *tx, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    if let Some((_, number)) = orders::find_by_lead(&mut *tx, lead_id).await? {
        return Err(AppError::conflict(
            "already_converted",
            format!("lead already converted to order {number}"),
        ));
    }

    let partner_id = conversion.partner_id.or(lead.partner_id).ok_or_else(|| {
        AppError::validation("choose or create a partner before converting the lead")
    })?;
    conversion.fields.partner_id = partner_id;
    if conversion.fields.contact_id.is_none() && lead.partner_id == Some(partner_id) {
        conversion.fields.contact_id = lead.contact_id;
    }

    let order = create_in_tx(
        &mut tx,
        user.user_id,
        today,
        conversion.fields,
        conversion.items,
        Some(lead_id),
    )
    .await?;

    if lead.partner_id.is_none() {
        leads::set_partner(&mut *tx, lead_id, partner_id).await?;
    }
    let note = format!("Megrendelés: {}", order.number);
    stages::insert_lead_stage(
        &mut *tx,
        lead_id,
        keys::LEAD_WON,
        Some(user.user_id),
        Some(&note),
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "lead",
        lead_id,
        "convert",
        json!({ "order_id": order.id, "order_number": order.number }),
    )
    .await?;
    tx.commit().await?;

    tracing::info!(lead_id, order_id = order.id, number = %order.number, "lead converted");
    Ok(order)
}
