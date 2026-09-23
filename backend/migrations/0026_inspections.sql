-- Vehicle handover inspections (átadás-átvétel): the rental-company style
-- check-out / check-in damage record. Order-linked only: an inspection belongs to
-- the order whose car is being handed over, and surfaces inside the Átvételi lap.
--
-- Photos themselves live in `images` (category `inspection`, uploaded through the
-- normal ticket flow) and are attached here with zone + purpose + capture metadata.
-- Signatures are finger-drawn PNGs stored as `documents` of kind `other`.
-- A signed inspection is locked: every mutation endpoint refuses non-draft rows,
-- and later remarks go to `inspection_notes` instead.

CREATE TABLE inspections (
    id              BIGSERIAL PRIMARY KEY,
    order_id        BIGINT NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL CHECK (kind IN ('checkout', 'checkin')),
    status          TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'signed')),
    vehicle_plate   TEXT NOT NULL CHECK (char_length(vehicle_plate) BETWEEN 1 AND 32),
    vehicle_vin     TEXT CHECK (vehicle_vin IS NULL OR char_length(vehicle_vin) BETWEEN 1 AND 32),
    inspector_name  TEXT NOT NULL CHECK (char_length(inspector_name) BETWEEN 1 AND 200),
    driver_name     TEXT CHECK (driver_name IS NULL OR char_length(driver_name) BETWEEN 1 AND 200),
    location        TEXT CHECK (location IS NULL OR char_length(location) BETWEEN 1 AND 300),
    odometer        INTEGER CHECK (odometer IS NULL OR odometer >= 0),
    fuel_level      TEXT CHECK (fuel_level IS NULL OR char_length(fuel_level) BETWEEN 1 AND 16),
    battery_pct     INTEGER CHECK (battery_pct IS NULL OR (battery_pct BETWEEN 0 AND 100)),
    warning_lights  TEXT,
    -- The check-out a check-in is compared against. Filled automatically with the
    -- latest signed check-out of the same order when the check-in is created.
    checkout_id     BIGINT REFERENCES inspections(id) ON DELETE SET NULL,
    customer_comment TEXT,
    signed_at       TIMESTAMPTZ,
    created_by      BIGINT NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (kind <> 'checkin' OR checkout_id IS NOT NULL),
    CHECK (status <> 'signed' OR signed_at IS NOT NULL)
);
CREATE TRIGGER inspections_touch BEFORE UPDATE ON inspections FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE INDEX inspections_order_idx ON inspections (order_id, kind, status);
-- A vehicle (plate) cannot have two open check-outs at the same time. Order-linked,
-- so "the same vehicle" is the same order: one draft check-out per order.
CREATE UNIQUE INDEX inspections_one_draft_checkout ON inspections (order_id)
    WHERE kind = 'checkout' AND status = 'draft';

CREATE TABLE inspection_damages (
    id              BIGSERIAL PRIMARY KEY,
    inspection_id   BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    zone_key        TEXT NOT NULL CHECK (char_length(zone_key) BETWEEN 1 AND 64),
    damage_type     TEXT NOT NULL CHECK (damage_type IN
                        ('scratch', 'dent', 'crack', 'chip', 'broken', 'missing', 'stain', 'tear', 'other')),
    severity        TEXT NOT NULL CHECK (severity IN ('minor', 'moderate', 'severe')),
    note            TEXT,
    -- Tap-to-mark on the outline diagram, normalised 0..1. Null when not marked.
    x               DOUBLE PRECISION CHECK (x IS NULL OR (x >= 0 AND x <= 1)),
    y               DOUBLE PRECISION CHECK (y IS NULL OR (y >= 0 AND y <= 1)),
    view            TEXT NOT NULL DEFAULT 'top' CHECK (view IN ('top', 'side')),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inspection_damages_inspection_idx ON inspection_damages (inspection_id, zone_key);

CREATE TABLE inspection_photos (
    id              BIGSERIAL PRIMARY KEY,
    inspection_id   BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    image_id        BIGINT NOT NULL UNIQUE REFERENCES images(id) ON DELETE CASCADE,
    zone_key        TEXT NOT NULL CHECK (char_length(zone_key) BETWEEN 1 AND 64),
    purpose         TEXT NOT NULL CHECK (purpose IN ('overview', 'closeup', 'dashboard', 'signature')),
    damage_id       BIGINT REFERENCES inspection_damages(id) ON DELETE SET NULL,
    taken_at        TIMESTAMPTZ NOT NULL,
    lat             DOUBLE PRECISION,
    lon             DOUBLE PRECISION,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inspection_photos_inspection_idx ON inspection_photos (inspection_id, zone_key);

-- Check-in review: every check-in damage gets exactly one verdict. A verdict may
-- point at the pre-existing check-out damage it matches, or stand alone as new.
CREATE TABLE inspection_verdicts (
    id                  BIGSERIAL PRIMARY KEY,
    checkin_id          BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    checkin_damage_id   BIGINT NOT NULL UNIQUE REFERENCES inspection_damages(id) ON DELETE CASCADE,
    checkout_damage_id  BIGINT REFERENCES inspection_damages(id) ON DELETE SET NULL,
    verdict             TEXT NOT NULL CHECK (verdict IN ('preexisting', 'new', 'dismissed')),
    note                TEXT,
    reviewed_by         BIGINT NOT NULL REFERENCES users(id),
    reviewed_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inspection_verdicts_checkin_idx ON inspection_verdicts (checkin_id);

CREATE TABLE inspection_signatures (
    id              BIGSERIAL PRIMARY KEY,
    inspection_id   BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    role            TEXT NOT NULL CHECK (role IN ('inspector', 'customer')),
    name            TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    document_id     BIGINT NOT NULL REFERENCES documents(id) ON DELETE RESTRICT,
    signed_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (inspection_id, role)
);

-- Follow-up annotations on a locked inspection. Timestamped, never edited: the
-- inspection itself is immutable once signed, so remarks live here instead.
CREATE TABLE inspection_notes (
    id              BIGSERIAL PRIMARY KEY,
    inspection_id   BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    body            TEXT NOT NULL CHECK (char_length(body) BETWEEN 1 AND 2000),
    created_by      BIGINT NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inspection_notes_inspection_idx ON inspection_notes (inspection_id);

-- Zone walkaround templates. `default` always applies; `cooling` adds the extras
-- when the order's project type has a cooling spec form. Edited from the web
-- settings screen; the phone downloads the resolved list at inspection start.
CREATE TABLE inspection_zone_templates (
    id              BIGSERIAL PRIMARY KEY,
    set_key         TEXT NOT NULL DEFAULT 'default' CHECK (set_key IN ('default', 'cooling')),
    zone_key        TEXT NOT NULL CHECK (char_length(zone_key) BETWEEN 1 AND 64),
    position        INTEGER NOT NULL,
    instruction     TEXT NOT NULL CHECK (char_length(instruction) BETWEEN 1 AND 300),
    optional        BOOLEAN NOT NULL DEFAULT false,
    required        BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (set_key, zone_key)
);

INSERT INTO inspection_zone_templates (set_key, zone_key, position, instruction, optional) VALUES
    ('default', 'front',              1, 'Elölről – az egész autó a képben', false),
    ('default', 'front_left',         2, 'Bal első sarok', false),
    ('default', 'left_side',          3, 'Bal oldal – az egész autó a képben', false),
    ('default', 'rear_left',          4, 'Bal hátsó sarok', false),
    ('default', 'rear',               5, 'Hátulról – az egész autó a képben', false),
    ('default', 'rear_right',         6, 'Jobb hátsó sarok', false),
    ('default', 'right_side',         7, 'Jobb oldal – az egész autó a képben', false),
    ('default', 'front_right',        8, 'Jobb első sarok', false),
    ('default', 'roof',               9, 'Tető (ha elérhető)', true),
    ('default', 'wheels',            10, 'Kerekek, gumik (mind a négy)', false),
    ('default', 'glass',             11, 'Szélvédő és üvegek', false),
    ('default', 'interior_front',    12, 'Belső: első ülések', false),
    ('default', 'interior_rear',     13, 'Belső: hátsó ülések', false),
    ('default', 'interior_dashboard',14, 'Belső: műszerfal és óraállás', false),
    ('default', 'interior_boot',     15, 'Belső: csomagtartó / raktér', false),
    ('cooling', 'cargo_box',         16, 'Rakodótér belülről', false),
    ('cooling', 'cargo_doors',        17, 'Rakodótér ajtók', false),
    ('cooling', 'refrigeration_unit', 18, 'Hűtőaggregát és kijelzője', false);
