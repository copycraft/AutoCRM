//! What a person's calendar feed shows (0049): their open tasks with a due date, the due
//! dates of open orders assigned to them, and their own leave.

use chrono::NaiveDate;
use sqlx::PgExecutor;

pub struct FeedRow {
    pub uid: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub summary: String,
    /// Path under the locale, e.g. `orders/12`.
    pub path: Option<String>,
}

pub async fn feed(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<Vec<FeedRow>> {
    sqlx::query_as!(
        FeedRow,
        r#"SELECT 'task-' || t.id AS "uid!", t.due_date AS "start_date!", t.due_date AS "end_date!",
                  'Feladat: ' || t.title AS "summary!",
                  CASE t.entity_type WHEN 'order' THEN 'orders/' || t.entity_id
                                     WHEN 'lead' THEN 'leads/' || t.entity_id
                                     WHEN 'partner' THEN 'partners/' || t.entity_id
                                     ELSE 'hr' END AS path
             FROM tasks t
            WHERE t.assigned_to = $1 AND t.done_at IS NULL AND t.due_date IS NOT NULL
              AND t.due_date > current_date - 90
           UNION ALL
           SELECT 'order-' || o.id, o.due_date, o.due_date,
                  'Határidő: ' || o.number || ' – ' || o.title,
                  'orders/' || o.id
             FROM orders o
             JOIN order_current_stage cs ON cs.order_id = o.id
             JOIN stage_definitions sd ON sd.entity = 'order' AND sd.key = cs.stage_key
            WHERE o.assigned_to = $1 AND o.due_date IS NOT NULL AND NOT sd.is_terminal
              AND o.due_date > current_date - 90
           UNION ALL
           SELECT 'absence-' || a.id, a.start_date, a.end_date,
                  CASE a.kind WHEN 'annual' THEN 'Szabadság'
                              WHEN 'sick' THEN 'Betegszabadság'
                              WHEN 'unpaid' THEN 'Fizetés nélküli szabadság'
                              ELSE 'Távollét' END,
                  NULL
             FROM absences a
             JOIN employees e ON e.id = a.employee_id
            WHERE e.user_id = $1 AND a.end_date > current_date - 90
           ORDER BY 2"#,
        user_id
    )
    .fetch_all(db)
    .await
}
