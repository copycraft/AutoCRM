//! Onboarding by invitation (0066).
//!
//! An admin enters a colleague's email, name and role; the colleague gets a letter with a
//! one-time link, picks how the app looks, sets their own password and is signed in. Nobody
//! makes up a temporary password or reads one out over the phone. The same letter sent
//! again is how an admin helps with a forgotten password.
//!
//! The link's token lives only in the letter; the database keeps its SHA-256, like a
//! session's. A link works once, for [`INVITE_DAYS`] days, and a newer letter, an admin's
//! password reset or the person's own password change retires it.

use chrono::{DateTime, TimeDelta, Utc};
use serde_json::json;
use sqlx::{PgConnection, PgExecutor, PgPool};

use crate::AppState;
use crate::domain::template::{self, TemplateValues};
use crate::error::{AppError, AppResult};
use crate::repo::emails::{self, NewEmail};
use crate::repo::users::{self, User};
use crate::repo::{audit, jobs, sessions, templates};
use crate::service::auth::{
    generate_token, hash_password_async, token_hash, validate_new_password,
};
use crate::service::email::{format_from, triggers};

/// How long a link works.
pub const INVITE_DAYS: i64 = 7;

/// How the app can look: follow the device, or always light / always dark. The CHECK in
/// 0066 holds the same list.
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

pub fn check_theme(theme: &str) -> AppResult<()> {
    if THEMES.contains(&theme) {
        Ok(())
    } else {
        Err(AppError::validation(
            "theme: expected system, light or dark",
        ))
    }
}

/// The account behind a link that can still be used.
pub struct OpenInvite {
    pub invite_id: i64,
    pub user_id: i64,
    pub email: String,
    pub display_name: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct InviteRow {
    id: i64,
    user_id: i64,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    email: String,
    display_name: String,
    is_active: bool,
}

/// Retires every open link of a user: a newer letter went out, or the password was set
/// some other way. Returns how many.
pub async fn revoke_open(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<u64> {
    let r = sqlx::query(
        "UPDATE user_invites SET revoked_at = now()
         WHERE user_id = $1 AND used_at IS NULL AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

/// Sends `user` a fresh letter (retiring older ones) and returns when its link expires.
/// `by` is the admin who asked: the letter names them, and goes out as theirs — at once,
/// not held for the customer send window or by the automatic-mail switch.
pub async fn send_invite(
    conn: &mut PgConnection,
    state: &AppState,
    user: &User,
    by: (i64, &str),
) -> AppResult<DateTime<Utc>> {
    if !user.is_active {
        return Err(AppError::validation(
            "the account is inactive: activate it before sending an invitation",
        ));
    }
    let token = generate_token();
    let expires_at = Utc::now() + TimeDelta::days(INVITE_DAYS);
    revoke_open(&mut *conn, user.id).await?;
    let invite_id: i64 = sqlx::query_scalar(
        "INSERT INTO user_invites (user_id, token_hash, created_by, expires_at)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user.id)
    .bind(token_hash(&token))
    .bind(by.0)
    .bind(expires_at)
    .fetch_one(&mut *conn)
    .await?;

    let tpl = templates::find_by_key(&mut *conn, "user_invite")
        .await?
        .ok_or_else(|| AppError::internal("email template 'user_invite' is missing"))?;
    let mut values = TemplateValues::new();
    values.insert("invite.name", user.display_name.clone());
    values.insert("invite.email", user.email.clone());
    values.insert("invite.by", by.1.to_string());
    values.insert(
        "invite.url",
        format!(
            "{}/hu/welcome/{token}",
            state.config.public_base_url.trim_end_matches('/')
        ),
    );
    values.insert(
        "invite.expires",
        expires_at
            .with_timezone(&state.config.business_tz)
            .format("%Y.%m.%d. %H:%M")
            .to_string(),
    );
    let subject = template::single_line(&template::render(&tpl.subject, &values).output);
    let text = template::render(&tpl.body, &values).output;
    // Through Markdown, so the bare link is a link in every mail app.
    let html = template::markdown_to_html(&template::render_markdown(&tpl.body, &values).output);
    let from = format_from(
        &state.config.email.from_name,
        &state.config.email.from_automatic,
    )?;
    let key = format!("user_invite:{invite_id}");
    let email_id = emails::insert(
        &mut *conn,
        &NewEmail {
            order_id: None,
            lead_id: None,
            partner_id: None,
            blocker_id: None,
            template_key: Some("user_invite"),
            trigger: triggers::USER_INVITE,
            sent_by: Some(by.0),
            idempotency_key: Some(&key),
            to_address: &user.email,
            cc: &[],
            bcc: &[],
            from_address: &from,
            reply_to: Some(&state.config.email.reply_to_default),
            subject: &subject,
            body_html: &html,
            body_text: &text,
            attachments: json!([]),
            send_after: None,
        },
    )
    .await?;
    if let Some(id) = email_id {
        jobs::enqueue(
            &mut *conn,
            "send_email",
            json!({ "email_id": id }),
            None,
            Some(&format!("send_email:{id}")),
        )
        .await?;
    }
    audit::record(
        &mut *conn,
        Some(by.0),
        "user",
        user.id,
        "invite_sent",
        json!({ "expires_at": expires_at }),
    )
    .await?;
    Ok(expires_at)
}

async fn find(db: impl PgExecutor<'_>, token: &str, lock: bool) -> AppResult<InviteRow> {
    // A token is 43 characters; anything far off is not one of ours.
    if token.is_empty() || token.len() > 128 {
        return Err(AppError::NotFound("invite"));
    }
    let sql = format!(
        "SELECT i.id, i.user_id, i.expires_at, i.used_at, i.revoked_at,
                u.email, u.display_name, u.is_active
           FROM user_invites i JOIN users u ON u.id = i.user_id
          WHERE i.token_hash = $1{}",
        if lock { " FOR UPDATE OF i" } else { "" }
    );
    let row: Option<InviteRow> = sqlx::query_as(&sql)
        .bind(token_hash(token))
        .fetch_optional(db)
        .await?;
    // A retired link, or one for an account switched off since, reads like no link at all.
    let row = row
        .filter(|r| r.revoked_at.is_none() && r.is_active)
        .ok_or(AppError::NotFound("invite"))?;
    if row.used_at.is_some() {
        return Err(AppError::rule(
            "invite_used",
            "this invitation link was already used",
        ));
    }
    if row.expires_at <= Utc::now() {
        return Err(AppError::rule(
            "invite_expired",
            "this invitation link has expired",
        ));
    }
    Ok(row)
}

/// The account a link sets up, while the link can still be used.
pub async fn open_invite(db: &PgPool, token: &str) -> AppResult<OpenInvite> {
    let r = find(db, token, false).await?;
    Ok(OpenInvite {
        invite_id: r.id,
        user_id: r.user_id,
        email: r.email,
        display_name: r.display_name,
        expires_at: r.expires_at,
    })
}

/// Sets the password and the look from the link's page, and spends the link. Every other
/// session of the account ends: if this was a forgotten password, whoever had it is out.
/// Returns the user, for the caller to sign in.
pub async fn accept(db: &PgPool, token: &str, password: String, theme: &str) -> AppResult<i64> {
    check_theme(theme)?;
    validate_new_password(&password)?;
    // Before the slow hash: a dead link should say so at once.
    find(db, token, false).await?;
    let hash = hash_password_async(password).await?;

    let mut tx = db.begin().await?;
    let invite = find(&mut *tx, token, true).await?;
    users::set_password(&mut *tx, invite.user_id, &hash, false).await?;
    sqlx::query("UPDATE user_invites SET used_at = now() WHERE id = $1")
        .bind(invite.id)
        .execute(&mut *tx)
        .await?;
    revoke_open(&mut *tx, invite.user_id).await?;
    sqlx::query(
        "INSERT INTO user_settings (user_id, theme) VALUES ($1, $2)
         ON CONFLICT (user_id) DO UPDATE SET theme = $2",
    )
    .bind(invite.user_id)
    .bind(theme)
    .execute(&mut *tx)
    .await?;
    sessions::revoke_all(&mut *tx, invite.user_id, None).await?;
    audit::record(
        &mut *tx,
        Some(invite.user_id),
        "user",
        invite.user_id,
        "invite_accepted",
        json!({ "theme": theme }),
    )
    .await?;
    tx.commit().await?;
    Ok(invite.user_id)
}
