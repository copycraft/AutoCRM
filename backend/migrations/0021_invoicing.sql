-- Invoicing: what we reported to NAV, and what NAV said back.
--
-- The line items of an order are not an invoice. An invoice is a snapshot taken at one
-- moment, with the VAT rate that applied then, and it stays that way even when the order
-- is edited afterwards — which is why `invoice_lines` copies the figures rather than
-- referencing `order_items`. It is also where VAT finally lives: `order_items` still
-- carries none (docs/DECISIONS.md), because a rate belongs to a bill, not to a job.
--
-- All amounts are minor units (fillér / eurocent), like `order_items.unit_price`. A
-- storno's amounts are negative: it is the reversal, and reading it as such needs no flag.

CREATE TYPE invoice_kind AS ENUM ('invoice', 'storno');

-- `submitting` is the honest name for the gap between "we asked" and "NAV answered":
-- reporting is asynchronous and can take a moment, so the state exists in the data rather
-- than only in the caller's imagination.
CREATE TYPE invoice_status AS ENUM ('submitting', 'issued', 'rejected', 'stornoed', 'annulled');

CREATE TABLE invoices (
    id                  BIGSERIAL PRIMARY KEY,
    order_id            BIGINT NOT NULL REFERENCES orders(id),
    -- The number NAV knows this document by. Unique forever, per taxpayer: never reused,
    -- not even after a rejection.
    number              TEXT NOT NULL UNIQUE,
    kind                invoice_kind NOT NULL,
    status              invoice_status NOT NULL,
    -- The invoice this one cancels. Set exactly on a storno.
    original_invoice_id BIGINT REFERENCES invoices(id),
    currency            CHAR(3) NOT NULL CHECK (currency IN ('HUF', 'EUR')),
    issue_date          DATE NOT NULL,
    delivery_date       DATE NOT NULL,
    payment_date        DATE,
    net_amount          BIGINT NOT NULL,
    vat_amount          BIGINT NOT NULL,
    gross_amount        BIGINT NOT NULL,

    -- NAV's own words, kept verbatim. `nav_error_code` is the fault code the tax
    -- authority returned (INVOICE_NUMBER_ALREADY_EXISTS and friends); the UI shows it and
    -- support quotes it. Swallowing it and storing "failed" would throw away the only
    -- thing that explains why.
    nav_transaction_id  TEXT,
    nav_status          TEXT,
    nav_error_code      TEXT,
    nav_message         TEXT,
    nav_messages        JSONB NOT NULL DEFAULT '[]'::jsonb,

    -- Technical annulment (a report that should never have been filed), which NAV accepts
    -- and a person then approves in the Online Számla portal.
    annulment_transaction_id TEXT,
    annulment_code      TEXT,
    annulment_reason    TEXT,
    annulled_at         TIMESTAMPTZ,

    -- The rendered PDF, fetched back from the sidecar after NAV stored the report.
    document_id         BIGINT REFERENCES documents(id),

    submitted_at        TIMESTAMPTZ,
    issued_at           TIMESTAMPTZ,
    created_by          BIGINT REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT invoices_storno_has_original
        CHECK ((kind = 'storno') = (original_invoice_id IS NOT NULL)),
    CONSTRAINT invoices_storno_not_self
        CHECK (original_invoice_id IS DISTINCT FROM id)
);
CREATE INDEX invoices_order_idx ON invoices (order_id, id DESC);
-- One live storno per invoice. A rejected attempt does not count: it was never reported,
-- and the next attempt needs a number of its own.
CREATE UNIQUE INDEX invoices_one_storno_per_invoice
    ON invoices (original_invoice_id)
    WHERE original_invoice_id IS NOT NULL AND status <> 'rejected';
CREATE TRIGGER invoices_touch BEFORE UPDATE ON invoices
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TABLE invoice_lines (
    id          BIGSERIAL PRIMARY KEY,
    invoice_id  BIGINT NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
    position    INT NOT NULL,
    description TEXT NOT NULL,
    quantity    NUMERIC(12,3) NOT NULL CHECK (quantity > 0),
    -- One of NAV's units of measure; the sidecar rejects anything else.
    unit        TEXT NOT NULL DEFAULT 'PIECE',
    -- Net unit price in minor units. Negative is allowed: a discount line is a line.
    unit_price  BIGINT NOT NULL,
    -- The fraction, not the percentage: 0.2700 is 27%.
    vat_rate    NUMERIC(5,4) NOT NULL CHECK (vat_rate >= 0 AND vat_rate <= 1),
    net_amount  BIGINT NOT NULL,
    vat_amount  BIGINT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX invoice_lines_invoice_idx ON invoice_lines (invoice_id, position, id);

-- Proformas (díjbekérő) are not invoices and are reported nowhere, so they are not rows in
-- `invoices` and carry no status, no transaction and no chain. A proforma is a rendered
-- document plus the number it was rendered under — kept because the caller is the only one
-- who can map a number back to the document: the renderer keeps nothing.
CREATE TABLE proformas (
    id           BIGSERIAL PRIMARY KEY,
    order_id     BIGINT NOT NULL REFERENCES orders(id),
    number       TEXT NOT NULL UNIQUE,
    currency     CHAR(3) NOT NULL CHECK (currency IN ('HUF', 'EUR')),
    issue_date   DATE NOT NULL,
    payment_date DATE,
    net_amount   BIGINT NOT NULL,
    vat_amount   BIGINT NOT NULL,
    gross_amount BIGINT NOT NULL,
    document_id  BIGINT NOT NULL REFERENCES documents(id),
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX proformas_order_idx ON proformas (order_id, id DESC);
