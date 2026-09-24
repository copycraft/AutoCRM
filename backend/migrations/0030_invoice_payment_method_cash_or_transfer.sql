-- Cash or bank transfer, nothing else. The 0028 CHECK admitted the sidecar's wider
-- set, but the office invoices cash or transfer only: narrowing the constraint keeps a
-- typo ("CHEQUE", "card " with a stray space is fine — parsing trims) from becoming a
-- stored method no letter, screen or report knows how to name.
ALTER TABLE invoices DROP CONSTRAINT invoices_payment_method_check;
ALTER TABLE invoices
    ADD CONSTRAINT invoices_payment_method_check CHECK (payment_method IN ('TRANSFER', 'CASH'));
