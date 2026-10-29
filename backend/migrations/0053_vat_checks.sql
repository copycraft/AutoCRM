-- EU VAT numbers checked against VIES (and later NAV). vat_checks is the audit trail: every
-- check, with VIES's consultation number when we sent our own VAT number, so we can prove
-- what we knew and when. Append-only; the trigger below holds even against a manual UPDATE.

CREATE TABLE vat_checks (
    id                     BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    subject_type           TEXT CHECK (subject_type IN ('partner', 'invoice')),
    subject_id             BIGINT,
    source                 TEXT NOT NULL CHECK (source IN ('VIES', 'NAV')),
    input_raw              TEXT NOT NULL,
    country_code           CHAR(2) NOT NULL,
    vat_number             TEXT NOT NULL,
    hu_domestic_tax_number TEXT CHECK (hu_domestic_tax_number ~ '^[0-9]{11}$'),
    -- FORMAT_INVALID is never stored: it never reaches VIES.
    status                 TEXT NOT NULL CHECK (status IN ('VALID', 'INVALID', 'UNAVAILABLE', 'ERROR')),
    error_code             TEXT,
    name                   TEXT,
    address                TEXT,
    request_identifier     TEXT,
    request_date           TIMESTAMPTZ,
    requester              TEXT,
    attempts               INT NOT NULL,
    duration_ms            INT NOT NULL,
    raw_response           JSONB,
    hints                  TEXT[] NOT NULL DEFAULT '{}',
    -- A user id, 'system' or 'migration'.
    created_by             TEXT NOT NULL,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((subject_type IS NULL) = (subject_id IS NULL))
);
CREATE INDEX vat_checks_number_idx ON vat_checks (country_code, vat_number, created_at DESC);
CREATE INDEX vat_checks_subject_idx ON vat_checks (subject_type, subject_id, created_at DESC);

CREATE FUNCTION protect_vat_checks() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'vat_checks is append-only' USING ERRCODE = 'AC001';
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER vat_checks_append_only BEFORE UPDATE OR DELETE ON vat_checks
    FOR EACH ROW EXECUTE FUNCTION protect_vat_checks();

-- The latest verdict on the partner, for lists and filters; vat_checks stays the source of
-- truth. The number itself already lives in partners.eu_tax_number / tax_number;
-- vat_formatted records which number the status is about, so an edited number reads as
-- unchecked rather than inheriting the old verdict.
ALTER TABLE partners
    ADD COLUMN vat_formatted      TEXT,
    ADD COLUMN vat_status         TEXT CHECK (vat_status IN ('VALID', 'INVALID', 'UNAVAILABLE', 'ERROR')),
    ADD COLUMN vat_checked_at     TIMESTAMPTZ,
    ADD COLUMN vat_last_check_id  BIGINT REFERENCES vat_checks (id);
