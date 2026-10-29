-- An UNAVAILABLE verdict is never final: the worker asks VIES again every 15 minutes for a
-- day, then gives up and flags the partner for a person to re-check by hand.
ALTER TABLE partners
    -- When the current run of UNAVAILABLE answers began; NULL once VIES answered.
    ADD COLUMN vat_unavailable_since TIMESTAMPTZ,
    ADD COLUMN vat_needs_manual_check BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX partners_vat_status_idx ON partners (vat_status) WHERE vat_status IS NOT NULL;
