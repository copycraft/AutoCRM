//! The technical content split by department (0059): who may write a department's module,
//! and handing it to the department.
//!
//! Rights: everyone reads (as with the rest of an order). The office (`EditOrders`) writes
//! every module; a department's members write that department's module and nobody else's.
//! Handing a module over ("Kiadás a részlegnek") tells the department's heads — a bell
//! notification and a letter — and nobody else, for now.

use serde_json::json;
use sqlx::PgConnection;

use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::order_modules::{self, OrderModule};
use crate::repo::{audit, departments, notifications, orders};
use crate::service::auth::AuthUser;
use crate::service::email::{About, AutomaticEmail, queue_automatic, triggers};

/// Whether this person may write the department's module.
pub async fn can_edit(
    conn: &mut PgConnection,
    user: &AuthUser,
    department_id: i64,
) -> AppResult<bool> {
    if user.can(Capability::EditOrders) {
        return Ok(true);
    }
    Ok(departments::of_user(&mut *conn, user.user_id)
        .await?
        .contains(&department_id))
}

pub async fn require_edit(
    conn: &mut PgConnection,
    user: &AuthUser,
    department_id: i64,
) -> AppResult<()> {
    if can_edit(conn, user, department_id).await? {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

/// Hands the module to its department: released now, and every head told. Released before
/// and written to since, it goes out again as a change; released and unchanged, nobody is
/// told twice.
pub async fn release(state: &AppState, user: &AuthUser, module_id: i64) -> AppResult<OrderModule> {
    let mut tx = state.db.begin().await?;
    let module = order_modules::find(&mut *tx, module_id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    require_edit(&mut tx, user, module.department_id).await?;
    if module.content.trim().is_empty() {
        return Err(AppError::validation(
            "the module is empty: write what the department should build before releasing it",
        ));
    }
    if module.status == "released" && !module.changed_since_release() {
        tx.rollback().await?;
        return Ok(module);
    }
    let event = if module.released_at.is_some() {
        "módosult"
    } else {
        "kiadva"
    };
    let order = orders::find(&mut *tx, module.order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    order_modules::mark_released(&mut *tx, module_id, user.user_id).await?;

    let link = format!("/orders/{}?tab=modules", order.id);
    let url = format!(
        "{}/hu{link}",
        state.config.public_base_url.trim_end_matches('/')
    );
    let title = format!(
        "{}: {} – {}",
        module.department_name, order.number, order.title
    );
    let body = if event == "kiadva" {
        "Új műszaki tartalom a részlegnek."
    } else {
        "A részleg műszaki tartalma módosult."
    };
    // Per release, not per second: a change sent right after the first release is a new letter.
    let released_at = chrono::Utc::now().timestamp_micros();
    let heads = departments::heads(&mut *tx, module.department_id).await?;
    for head in &heads {
        notifications::notify_user(
            &mut *tx,
            head.user_id,
            "module",
            &title,
            Some(body),
            Some(&link),
        )
        .await?;
        let mut mail = AutomaticEmail::new(
            "module_released",
            About {
                order_id: Some(order.id),
                ..About::default()
            },
            &head.email,
            triggers::MODULE_RELEASED,
            format!("module_released:{module_id}:{}:{released_at}", head.user_id),
        );
        mail.extra_values
            .insert("module.department", module.department_name.clone());
        mail.extra_values.insert("module.event", event.to_string());
        mail.extra_values
            .insert("module.content", module.content.replace("{{", "{ {"));
        mail.extra_values.insert("module.url", url.clone());
        queue_automatic(&mut tx, &state.config, mail).await?;
    }
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        order.id,
        "module_released",
        json!({
            "module_id": module_id,
            "department": module.department_name,
            "event": event,
            "heads": heads.len(),
        }),
    )
    .await?;
    tx.commit().await?;
    order_modules::find(&state.db, module_id)
        .await?
        .ok_or(AppError::NotFound("module"))
}
