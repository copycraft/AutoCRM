-- A 0% invoice to an EU business rests on the buyer's EU VAT number being valid: the VIES
-- check that showed it (or, while VIES was down and overrides are allowed, the reason the
-- user gave for issuing anyway) is kept with the invoice.
ALTER TABLE invoices
    ADD COLUMN vat_check_id        BIGINT REFERENCES vat_checks (id),
    ADD COLUMN vat_override_reason TEXT;
