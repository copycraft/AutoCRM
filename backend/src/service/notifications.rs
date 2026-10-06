//! Raising notifications. The feed itself lives in `repo::notifications`.

use sqlx::PgPool;

use crate::error::AppResult;
use crate::repo::leads::Lead;
use crate::repo::notifications;

/// Notifications older than this are deleted whenever a new one is raised.
const KEEP_DAYS: i32 = 60;

/// Who is told about a new lead: everyone who can edit leads (admins and office).
const LEAD_AUDIENCE: &[&str] = &["admin", "office"];

fn short(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    format!("{}…", cut.trim_end())
}

/// A lead came in from the website: tell the people who work leads.
pub async fn lead_arrived(db: &PgPool, lead: &Lead) -> AppResult<u64> {
    let who = lead.contact_name.as_deref().unwrap_or("Ismeretlen");
    let body = match lead
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        Some(text) => format!("{who}: {}", short(text, 120)),
        None => who.to_string(),
    };
    let n = notifications::broadcast(
        db,
        LEAD_AUDIENCE,
        "lead",
        "Új érdeklődés a weboldalról",
        Some(&body),
        Some(&format!("/leads/{}", lead.id)),
    )
    .await?;
    notifications::purge_older_than_days(db, KEEP_DAYS).await?;
    Ok(n)
}

/// Someone applied through a job listing's form: tell the people who work in HR.
pub async fn application_arrived(
    db: &PgPool,
    posting_id: i64,
    job_title: &str,
    applicant: &str,
) -> AppResult<u64> {
    let n = notifications::broadcast_hr(
        db,
        "application",
        "Új jelentkezés",
        Some(&format!("{applicant}: {}", short(job_title, 120))),
        Some(&format!("/hr?tab=recruitment&job={posting_id}")),
    )
    .await?;
    notifications::purge_older_than_days(db, KEEP_DAYS).await?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_text_is_cut_on_a_character_boundary() {
        assert_eq!(short("  rövid  ", 10), "rövid");
        assert_eq!(short(&"é".repeat(30), 10), format!("{}…", "é".repeat(10)));
    }
}
