use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct FxRate {
    pub day: NaiveDate,
    pub base: String,
    pub quote: String,
    pub rate: Decimal,
    pub source: String,
    pub fetched_at: DateTime<Utc>,
}

/// Published rates don't change; re-fetching a day only corrects it if MNB did.
pub async fn upsert(
    db: impl PgExecutor<'_>,
    day: NaiveDate,
    base: &str,
    quote: &str,
    rate: Decimal,
    source: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO fx_rates (day, base, quote, rate, source) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (day, base, quote) DO UPDATE SET rate = EXCLUDED.rate, source = EXCLUDED.source, fetched_at = now()
         WHERE fx_rates.rate IS DISTINCT FROM EXCLUDED.rate",
        day,
        base,
        quote,
        rate,
        source
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list(
    db: impl PgExecutor<'_>,
    base: &str,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<FxRate>> {
    sqlx::query_as!(
        FxRate,
        "SELECT day, base, quote, rate, source, fetched_at FROM fx_rates
         WHERE base = $1 AND quote = 'HUF' AND day BETWEEN $2 AND $3 ORDER BY day DESC",
        base,
        from,
        to
    )
    .fetch_all(db)
    .await
}

pub async fn latest_day(db: impl PgExecutor<'_>, base: &str) -> sqlx::Result<Option<NaiveDate>> {
    sqlx::query_scalar!(
        "SELECT max(day) FROM fx_rates WHERE base = $1 AND quote = 'HUF'",
        base
    )
    .fetch_one(db)
    .await
}

/// Orders whose valuation date has no usable rate (nothing within the 10-day window).
pub async fn orders_missing_rates(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM order_values WHERE currency <> 'HUF' AND total_huf_minor IS NULL"#)
        .fetch_one(db)
        .await
}
