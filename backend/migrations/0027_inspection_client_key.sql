-- Idempotent inspection create (logic audit INSP-L10).
--
-- The phone creates the server inspection during an offline-first sync. If the
-- response to `POST /inspections` is lost (or the app dies before storing the new
-- id), the retry must return the row it already created instead of making a second
-- one, or, for a check-out, failing forever on `inspections_one_draft_checkout`
-- against its own orphan. The client sends a stable key (the draft's local UUID).
-- Nullable: rows created before this migration, and callers that send no key,
-- keep the old behaviour.
ALTER TABLE inspections
    ADD COLUMN client_key TEXT
        CHECK (client_key IS NULL OR char_length(client_key) BETWEEN 1 AND 100);

CREATE UNIQUE INDEX inspections_client_key ON inspections (client_key)
    WHERE client_key IS NOT NULL;
