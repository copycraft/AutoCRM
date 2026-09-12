-- Documents can hang off a lead (V2.4).
--
-- The expensive half of the quotation gap: `documents.order_id` was NOT NULL, so a
-- quotation PDF had nowhere to live and `service/email.rs` rejected attachments outright
-- without an order — a quotation could not be emailed from the system at all. Done now,
-- while the table is small; after the migration it holds a row per migrated file.
--
-- NOT VALID then VALIDATE so the constraint is enforced for new rows immediately and the
-- existing rows are checked without an ACCESS EXCLUSIVE lock for the duration of the scan.

ALTER TABLE documents ADD COLUMN lead_id BIGINT REFERENCES leads(id);
ALTER TABLE documents ALTER COLUMN order_id DROP NOT NULL;

ALTER TABLE documents
    ADD CONSTRAINT documents_one_owner CHECK (num_nonnulls(order_id, lead_id) = 1) NOT VALID;
ALTER TABLE documents VALIDATE CONSTRAINT documents_one_owner;

-- The dedup index has to cope with a null owner on either side. Two partial indexes rather
-- than one over coalesce(order_id, -lead_id): they stay readable, and each is used by the
-- lookup that matches it.
DROP INDEX documents_order_hash_key;
CREATE UNIQUE INDEX documents_order_hash_key ON documents (order_id, content_hash)
    WHERE deleted_at IS NULL AND order_id IS NOT NULL;
CREATE UNIQUE INDEX documents_lead_hash_key ON documents (lead_id, content_hash)
    WHERE deleted_at IS NULL AND lead_id IS NOT NULL;
CREATE INDEX documents_lead_idx ON documents (lead_id, uploaded_at DESC)
    WHERE deleted_at IS NULL AND lead_id IS NOT NULL;
