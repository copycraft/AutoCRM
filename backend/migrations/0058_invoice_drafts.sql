-- An invoice being prepared in the Számlázó before it is issued.
--
-- The order screen no longer issues on the spot: "Számlázóba" files a draft here, and the
-- billing desk looks it over against a preview first. The draft has no lines of its own:
-- they are the order's items, so a line corrected on the invoice is corrected on the order
-- too. This row holds only what belongs to the invoice alone; NULL means the default the
-- issue would take (today, the configured rate, transfer). Issuing removes it.
CREATE TABLE invoice_drafts (
    order_id       BIGINT PRIMARY KEY REFERENCES orders(id),
    vat_rate       NUMERIC(5,4) CHECK (vat_rate IS NULL OR (vat_rate >= 0 AND vat_rate < 1)),
    issue_date     DATE,
    delivery_date  DATE,
    payment_date   DATE,
    payment_method TEXT,
    created_by     BIGINT REFERENCES users(id),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER invoice_drafts_touch BEFORE UPDATE ON invoice_drafts
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
