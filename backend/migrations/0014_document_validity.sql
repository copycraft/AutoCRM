-- Documents get a validity period and an issuer (V2.5).
--
-- An ATP certificate belongs to the vehicle for its life and expires. Without these
-- columns it is a PDF filed under one job with kind = 'other', and "which certificates
-- expire next quarter" has no answer at any layer. For refrigerated bodies exported to
-- Austria and Bavaria that question is not optional.
ALTER TABLE documents
    ADD COLUMN issuer      TEXT,
    ADD COLUMN valid_from  DATE,
    ADD COLUMN valid_until DATE,
    ADD CONSTRAINT documents_validity_ordered
        CHECK (valid_from IS NULL OR valid_until IS NULL OR valid_from <= valid_until);

-- Drives GET /documents?kind=certificate&expiring_before=…, the first cross-order document
-- query in the system.
CREATE INDEX documents_valid_until_idx ON documents (valid_until)
    WHERE deleted_at IS NULL AND valid_until IS NOT NULL;
