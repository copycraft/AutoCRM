//! Creating orders and changing their line items.

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgConnection;

use crate::domain::money::{Currency, Money};
use crate::domain::order::validate_line_item;
use crate::domain::stage::{StageEntity, initial_stage};
use crate::error::{AppError, AppResult};
use crate::repo::orders::{Order, OrderFields};
use crate::repo::{audit, config, contacts, order_items, orders, partners, stages};

#[derive(Debug, Clone)]
pub struct NewItem {
    pub description: String,
    pub quantity: Decimal,
    pub unit_price: i64,
}

/// Checks that referenced records exist and belong together. Foreign keys would catch
/// a missing row too, but not "contact belongs to a different partner".
pub async fn validate_references(conn: &mut PgConnection, f: &OrderFields) -> AppResult<()> {
    let partner = partners::find(&mut *conn, f.partner_id)
        .await?
        .ok_or_else(|| AppError::validation("partner does not exist"))?;
    if partner.archived_at.is_some() {
        return Err(AppError::validation("partner is archived"));
    }
    if let Some(contact_id) = f.contact_id {
        let contact = contacts::find(&mut *conn, contact_id)
            .await?
            .ok_or_else(|| AppError::validation("contact does not exist"))?;
        if contact.partner_id != f.partner_id {
            return Err(AppError::validation(
                "contact belongs to a different partner",
            ));
        }
    }
    Ok(())
}

/// Creates an order with its items and initial stage inside the caller's transaction.
pub async fn create_in_tx(
    conn: &mut PgConnection,
    user_id: i64,
    today: NaiveDate,
    fields: OrderFields,
    items: Vec<NewItem>,
    lead_id: Option<i64>,
) -> AppResult<Order> {
    let currency: Currency = fields
        .currency
        .parse()
        .map_err(|e| AppError::validation(format!("{e}")))?;
    for item in &items {
        validate_line_item(
            &item.description,
            item.quantity,
            Money::new(item.unit_price, currency),
            currency,
        )
        .map_err(|e| AppError::validation(e.to_string()))?;
    }
    validate_references(conn, &fields).await?;

    let number = orders::next_number(conn, today.year()).await?;
    let order = orders::insert(&mut *conn, &number, &fields, lead_id, user_id).await?;

    for (i, item) in items.iter().enumerate() {
        let position = (i32::try_from(i).unwrap_or(i32::MAX / 10) + 1) * 10;
        order_items::insert(
            &mut *conn,
            order.id,
            position,
            item.description.trim(),
            item.quantity,
            item.unit_price,
            currency.code(),
        )
        .await?;
    }

    let definitions = config::stage_definitions(&mut *conn, StageEntity::Order).await?;
    let initial = initial_stage(&definitions)
        .ok_or_else(|| AppError::internal("no active initial order stage is configured"))?;
    stages::insert_order_stage(&mut *conn, order.id, &initial.key, Some(user_id), None).await?;

    audit::record(
        &mut *conn,
        Some(user_id),
        "order",
        order.id,
        "create",
        json!({ "number": order.number, "lead_id": lead_id, "stage": initial.key, "items": items.len() }),
    )
    .await?;
    Ok(order)
}

pub fn order_currency(order: &Order) -> AppResult<Currency> {
    order
        .currency
        .parse()
        .map_err(|e| AppError::internal(format!("order {} has invalid currency: {e}", order.id)))
}

/// Sums line totals through the Money type, so a mixed-currency order is an error, not a number.
pub fn items_total(currency: Currency, items: &[order_items::OrderItem]) -> AppResult<Money> {
    let mut total = Money::zero(currency);
    for item in items {
        let item_currency: Currency = item
            .currency
            .parse()
            .map_err(|e| AppError::internal(format!("{e}")))?;
        let line = Money::new(item.unit_price, item_currency)
            .times_quantity(item.quantity)
            .map_err(|e| AppError::internal(e.to_string()))?;
        total = total
            .checked_add(line)
            .map_err(|e| AppError::internal(e.to_string()))?;
    }
    Ok(total)
}
