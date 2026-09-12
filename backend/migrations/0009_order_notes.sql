-- Imported MiniCRM activity (V1.3).
--
-- MiniCRM's ToDoList is where this company's project management has lived for years:
-- who said what, when, and what was agreed. It is an append-only historical record,
-- not a task system — AutoCRM has no to-do feature and is not gaining one. Notes are
-- shown on the order's Napló tab beside audit_log and are never edited or completed.
CREATE TABLE order_notes (
    id          BIGSERIAL PRIMARY KEY,
    order_id    BIGINT NOT NULL REFERENCES orders(id),
    minicrm_id  BIGINT UNIQUE,              -- migration provenance; re-running the load upserts
    author_name TEXT,                       -- MiniCRM user name as text: those accounts don't exist here
    body        TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    raw_import  JSONB,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX order_notes_order_idx ON order_notes (order_id, occurred_at DESC, id DESC);
