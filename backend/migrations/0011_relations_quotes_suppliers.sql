-- Order relationships, quotation fields on leads, the supplier flag, and repeat conversion.
-- All additive, all cheap now and expensive after 1000 orders are migrated.

-- V2.2: warranty, rework and repeat jobs point at the job they repair. No UI beyond a link
-- on the order detail — the column is cheap, the data is not, and nobody will reconstruct
-- 1000 orders of warranty links retroactively.
ALTER TABLE orders
    ADD COLUMN related_order_id BIGINT REFERENCES orders(id),
    ADD COLUMN relation TEXT CHECK (relation IS NULL OR relation IN ('warranty', 'rework', 'repeat')),
    -- Either both or neither: a relation with no target says nothing.
    ADD CONSTRAINT orders_relation_complete CHECK (num_nonnulls(related_order_id, relation) <> 1),
    ADD CONSTRAINT orders_relation_not_self CHECK (related_order_id IS NULL OR related_order_id <> id);
CREATE INDEX orders_related_idx ON orders (related_order_id) WHERE related_order_id IS NOT NULL;

-- V2.3: the quotation. Not a `quotes` table with versions — at 60 leads a month that is
-- more machinery than the business justifies. A value, a currency and a validity date on
-- the lead, with changes to the value logged in audit_log (entity = 'lead'), is enough to
-- answer "who did we quote, for how much, and when does it expire".
ALTER TABLE leads
    ADD COLUMN quoted_value_minor BIGINT,
    ADD COLUMN currency CHAR(3) CHECK (currency IS NULL OR currency IN ('HUF', 'EUR')),
    ADD COLUMN quote_valid_until DATE,
    -- A number with no currency is not a price.
    ADD CONSTRAINT leads_quote_has_currency CHECK (quoted_value_minor IS NULL OR currency IS NOT NULL);
CREATE INDEX leads_quote_expiry_idx ON leads (quote_valid_until) WHERE quote_valid_until IS NOT NULL;

-- V2.6: suppliers and customers share the partners table, so the paint shop turns up in the
-- customer picker. One nullable column rather than a boolean pair: a partner can be both,
-- and 'both' is a real answer here.
ALTER TABLE partners
    ADD COLUMN role TEXT CHECK (role IS NULL OR role IN ('customer', 'supplier', 'both'));
COMMENT ON COLUMN partners.role IS
    'NULL = not yet classified; treated as a customer by the pickers. Set deliberately.';

-- V2.7: one enquiry for three identical Sprinters becomes three orders, and before this
-- only the first kept its origin — the other two were orphans with no record of where they
-- came from. Drop the uniqueness, keep the index.
ALTER TABLE orders DROP CONSTRAINT orders_lead_id_key;
CREATE INDEX orders_lead_idx ON orders (lead_id) WHERE lead_id IS NOT NULL;
