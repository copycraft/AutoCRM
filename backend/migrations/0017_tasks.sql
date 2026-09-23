-- Follow-up tasks (Feladatok): lightweight reminders pinned to an order, lead or
-- partner. Not a workflow engine: title, optional due date, optional assignee,
-- done flag. The dashboard widget reads the open ones; record pages read their own.
CREATE TABLE tasks (
    id            BIGSERIAL PRIMARY KEY,
    entity_type   TEXT NOT NULL CHECK (entity_type IN ('order', 'lead', 'partner')),
    entity_id     BIGINT NOT NULL,
    title         TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 200),
    due_date      DATE,
    done_at       TIMESTAMPTZ,
    assigned_to   BIGINT REFERENCES users(id) ON DELETE SET NULL,
    created_by    BIGINT NOT NULL REFERENCES users(id),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER tasks_touch BEFORE UPDATE ON tasks FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE INDEX tasks_entity_idx ON tasks (entity_type, entity_id);
CREATE INDEX tasks_open_idx ON tasks (assigned_to, due_date) WHERE done_at IS NULL;
