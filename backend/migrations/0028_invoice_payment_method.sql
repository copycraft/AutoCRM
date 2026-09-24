-- The payment method is part of the invoice, not of the send: cash and transfer
-- invoices share one numbering series, but each document records how it is paid.
--
-- Backfill: everything reported so far went out with the old default, TRANSFER —
-- the submit path defaulted an absent method to it, and the sidecar only accepts the
-- closed set below. A row whose letter named something else would already be rejected.
ALTER TABLE invoices
    ADD COLUMN payment_method TEXT NOT NULL DEFAULT 'TRANSFER'
        CHECK (payment_method IN ('TRANSFER', 'CASH', 'CARD', 'VOUCHER', 'OTHER'));
