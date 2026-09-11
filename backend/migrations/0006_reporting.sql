-- Reporting: FX rates and plain views. At this data volume every report runs in
-- milliseconds, so there are no materialized views and no refresh job.

-- rate = units of `quote` per 1 unit of `base`, as published. MNB: base EUR, quote HUF.
CREATE TABLE fx_rates (
    day        DATE NOT NULL,
    base       CHAR(3) NOT NULL,
    quote      CHAR(3) NOT NULL,
    rate       NUMERIC(18,8) NOT NULL CHECK (rate > 0),
    source     TEXT NOT NULL DEFAULT 'MNB',
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (day, base, quote)
);

CREATE VIEW order_current_stage AS
SELECT DISTINCT ON (os.order_id)
       os.order_id, os.stage_key, os.entered_at, os.entered_by
FROM order_stages os
ORDER BY os.order_id, os.entered_at DESC, os.id DESC;

CREATE VIEW lead_current_stage AS
SELECT DISTINCT ON (ls.lead_id)
       ls.lead_id, ls.stage_key, ls.entered_at, ls.entered_by
FROM lead_stages ls
ORDER BY ls.lead_id, ls.entered_at DESC, ls.id DESC;

-- One row per stage visit. left_at is null while the order is still in that stage.
CREATE VIEW order_stage_intervals AS
SELECT os.id,
       os.order_id,
       os.stage_key,
       os.entered_at,
       lead(os.entered_at) OVER w AS left_at,
       coalesce(lead(os.entered_at) OVER w, now()) - os.entered_at AS duration
FROM order_stages os
WINDOW w AS (PARTITION BY os.order_id ORDER BY os.entered_at, os.id);

-- Order value in its own currency and normalised to HUF at the valuation date's MNB rate.
-- MNB does not publish on weekends/holidays: the latest rate within 10 days before the
-- valuation date is used. No rate in that window → total_huf_minor is null (a data gap
-- worth seeing, not something to paper over with today's rate).
CREATE VIEW order_values AS
SELECT o.id AS order_id,
       o.currency,
       o.valuation_date,
       t.total_minor,
       fx.day  AS fx_day,
       fx.rate AS fx_rate,
       CASE
           WHEN o.currency = 'HUF' THEN t.total_minor
           WHEN fx.rate IS NOT NULL THEN round(t.total_minor * fx.rate)::bigint
       END AS total_huf_minor
FROM orders o
CROSS JOIN LATERAL (
    SELECT coalesce(sum(round(oi.quantity * oi.unit_price)), 0)::bigint AS total_minor
    FROM order_items oi
    WHERE oi.order_id = o.id
) t
LEFT JOIN LATERAL (
    SELECT r.day, r.rate
    FROM fx_rates r
    WHERE o.currency <> 'HUF'
      AND r.base = o.currency
      AND r.quote = 'HUF'
      AND r.day <= o.valuation_date
      AND r.day >= o.valuation_date - 10
    ORDER BY r.day DESC
    LIMIT 1
) fx ON true;
