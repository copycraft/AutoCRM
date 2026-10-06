-- Invoice status lists, worked out from the data rather than set by hand, and incoming
-- (supplier) invoices.
--
-- Outgoing. The MiniCRM lists were Kiállítandó, Kiállítva / Fizetve / Archiválva,
-- Sztornózva, Sztornó. All but one follow from what NAV said; the missing fact is whether
-- the customer paid, so `paid_at` is added. A cash invoice is paid when it is issued, so
-- the trigger sets it then; a transfer is marked paid by the office.
ALTER TABLE invoices ADD COLUMN paid_at TIMESTAMPTZ;

CREATE FUNCTION invoices_cash_paid() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status = 'issued' AND NEW.kind = 'invoice' AND NEW.payment_method = 'CASH'
       AND NEW.paid_at IS NULL
       AND (TG_OP = 'INSERT' OR OLD.status IS DISTINCT FROM 'issued') THEN
        NEW.paid_at := coalesce(NEW.issued_at, now());
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER invoices_cash_paid BEFORE INSERT OR UPDATE ON invoices
    FOR EACH ROW EXECUTE FUNCTION invoices_cash_paid();

UPDATE invoices SET paid_at = coalesce(issued_at, created_at)
 WHERE status = 'issued' AND kind = 'invoice' AND payment_method = 'CASH';

-- One list per invoice:
--   to_issue  Kiállítandó  being reported, or rejected and not yet issued again
--   issued    Kiállítva    valid at NAV, not paid
--   paid      Fizetve      valid at NAV, paid
--   archived  Archiválva   technically annulled, or a rejected attempt a later one replaced
--   stornoed  Sztornózva   cancelled by a storno
--   storno    Sztornó      the storno document itself
CREATE VIEW invoice_buckets AS
SELECT i.id,
       CASE
           WHEN i.status = 'submitting' THEN 'to_issue'
           WHEN i.status = 'rejected' THEN
               CASE WHEN EXISTS (SELECT 1 FROM invoices j
                                  WHERE j.order_id = i.order_id AND j.kind = i.kind
                                    AND j.id > i.id AND j.status <> 'rejected')
                    THEN 'archived' ELSE 'to_issue' END
           WHEN i.kind = 'storno' THEN 'storno'
           WHEN i.status = 'stornoed' THEN 'stornoed'
           WHEN i.status = 'annulled' THEN 'archived'
           WHEN i.paid_at IS NOT NULL THEN 'paid'
           ELSE 'issued'
       END AS bucket
FROM invoices i;

-- Incoming: the bills suppliers send. The office drops the PDF (or photo) in and fills
-- in what it says; the list it lands on follows from the figures:
--   open_invoice   Nyitott számla        an invoice not yet paid
--   open_proforma  Nyitott díjbekérő     a proforma not yet paid
--   transferred    Átutalva              paid in full by transfer (or card)
--   cash           Készpénzes            paid in full in cash
--   partial        Részteljesítés        paid in part
--   cash_receipt   Pénztárbizonylat      a cash register receipt
--   booking_only   Csak könyvelésben létező  exists only in the books
CREATE TABLE incoming_invoices (
    id                  BIGSERIAL PRIMARY KEY,
    kind                TEXT NOT NULL DEFAULT 'invoice'
                            CHECK (kind IN ('invoice', 'proforma', 'receipt')),
    supplier_name       TEXT NOT NULL DEFAULT '',
    supplier_tax_number TEXT,
    partner_id          BIGINT REFERENCES partners(id),
    invoice_number      TEXT,
    issue_date          DATE,
    due_date            DATE,
    currency            CHAR(3) NOT NULL DEFAULT 'HUF' CHECK (currency IN ('HUF', 'EUR')),
    -- Minor units. Null until someone has read them off the document.
    net_amount          BIGINT,
    vat_amount          BIGINT,
    gross_amount        BIGINT CHECK (gross_amount IS NULL OR gross_amount >= 0),
    payment_method      TEXT NOT NULL DEFAULT 'TRANSFER'
                            CHECK (payment_method IN ('TRANSFER', 'CASH', 'CARD')),
    paid_amount         BIGINT NOT NULL DEFAULT 0 CHECK (paid_amount >= 0),
    paid_on             DATE,
    booking_only        BOOLEAN NOT NULL DEFAULT false,
    notes               TEXT,
    -- The uploaded document in object storage.
    file_key            TEXT,
    file_name           TEXT,
    file_type           TEXT,
    file_size           BIGINT,
    file_hash           BYTEA CHECK (file_hash IS NULL OR length(file_hash) = 32),
    created_by          BIGINT REFERENCES users(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
);
CREATE TRIGGER incoming_invoices_touch BEFORE UPDATE ON incoming_invoices
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
-- The same file twice, or the same supplier's number twice, is a double entry.
CREATE UNIQUE INDEX incoming_invoices_file_idx ON incoming_invoices (file_hash)
    WHERE deleted_at IS NULL AND file_hash IS NOT NULL;
CREATE UNIQUE INDEX incoming_invoices_number_idx
    ON incoming_invoices (lower(supplier_name), lower(invoice_number))
    WHERE deleted_at IS NULL AND invoice_number IS NOT NULL AND supplier_name <> '';
CREATE INDEX incoming_invoices_created_idx ON incoming_invoices (created_at DESC) WHERE deleted_at IS NULL;
