-- Yard layout (0049): a 2D map of the yard. Each item is a placed shape -- a parking
-- spot, a building, a workshop bay -- drawn by hand in the editor. The map is what the
-- board used to be: spots reference a yard_location, vehicles sit at spot coordinates.
-- The 3D view is rendered from this same layout.

CREATE TABLE IF NOT EXISTS yard_layout_items (
    id           BIGSERIAL PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('spot', 'building', 'workshop_bay')),
    -- The yard_location this spot assigns vehicles to (spots and workshop bays only).
    location_id  BIGINT REFERENCES yard_locations(id),
    label        TEXT NOT NULL DEFAULT '',
    number       TEXT NOT NULL DEFAULT '',
    -- Coordinates in the editor's grid (origin top-left, y down). The 3D view
    -- reads the same numbers, scaled: 1 grid unit = 1 metre.
    x            INT NOT NULL DEFAULT 0,
    y            INT NOT NULL DEFAULT 0,
    width        INT NOT NULL DEFAULT 1,
    height       INT NOT NULL DEFAULT 1,
    rotation     INT NOT NULL DEFAULT 0 CHECK (rotation IN (0, 90, 180, 270)),
    -- Workshop bays belong to a building; the building row carries the name.
    parent_id    BIGINT REFERENCES yard_layout_items(id),
    color        TEXT NOT NULL DEFAULT '#6b7280',
    position     INT NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kind <> 'spot' OR location_id IS NOT NULL),
    CHECK (kind <> 'workshop_bay' OR parent_id IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS yard_layout_items_kind_idx ON yard_layout_items (kind, position);
CREATE INDEX IF NOT EXISTS yard_layout_items_parent_idx ON yard_layout_items (parent_id);

CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS yard_layout_items_touch ON yard_layout_items;
CREATE TRIGGER yard_layout_items_touch
    BEFORE UPDATE ON yard_layout_items
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- Git-like revisions: every save stores the whole layout as JSON with a parent pointer.
-- Reverting creates a new revision pointing at the old state; history is never lost.
CREATE TABLE IF NOT EXISTS yard_revisions (
    id           BIGSERIAL PRIMARY KEY,
    parent_id    BIGINT REFERENCES yard_revisions(id),
    layout       JSONB NOT NULL CHECK (jsonb_typeof(layout) = 'array'),
    note         TEXT NOT NULL DEFAULT '',
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS yard_revisions_created_idx ON yard_revisions (created_at DESC);
