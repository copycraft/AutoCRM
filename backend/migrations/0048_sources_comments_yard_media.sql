-- One batch: lead sources as a list, the quote's exchange rate, comments with mentions, the
-- yard board, photo captions, annotations and trusted timestamps, a default photo category
-- per stage, document versions and thumbnails, tyres and videos on inspections, and the
-- internal incident log.

-- 1. Where a lead came from, picked from a list the office edits instead of typed freely,
--    so reports compare like with like. What the source does not say (which fair, who
--    referred them, which domain) goes in `source_detail`. `website` and `minicrm` are set
--    by the system and never offered by hand.
CREATE TABLE lead_sources (
    key         TEXT PRIMARY KEY CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    label       TEXT NOT NULL CHECK (btrim(label) <> ''),
    position    INT NOT NULL DEFAULT 0,
    is_system   BOOLEAN NOT NULL DEFAULT false,
    archived_at TIMESTAMPTZ,
    CHECK (NOT (is_system AND archived_at IS NOT NULL))
);
CREATE UNIQUE INDEX lead_sources_label_idx ON lead_sources (lower(label)) WHERE archived_at IS NULL;

INSERT INTO lead_sources (key, label, position, is_system) VALUES
    ('phone',      'Telefon',                 10,  false),
    ('email',      'E-mail',                  20,  false),
    ('walk_in',    'Személyesen',             30,  false),
    ('trade_fair', 'Kiállítás, vásár',        40,  false),
    ('referral',   'Ajánlás',                 50,  false),
    ('dealer',     'Kereskedő, viszonteladó', 60,  false),
    ('returning',  'Visszatérő ügyfél',       70,  false),
    ('social',     'Közösségi média',         80,  false),
    ('other',      'Egyéb',                   90,  false),
    ('website',    'Weboldal',                100, true),
    ('minicrm',    'MiniCRM (importált)',     110, true);

ALTER TABLE leads ADD COLUMN source_detail TEXT;

-- Existing free text: a value that names a source (by key or label, any case) becomes it;
-- anything else becomes `other`, with the text kept word for word as the detail.
UPDATE leads SET source = NULL WHERE source IS NOT NULL AND btrim(source) = '';
UPDATE leads l SET source = s.key
  FROM lead_sources s
 WHERE l.source IS NOT NULL AND l.source <> s.key
   AND lower(btrim(l.source)) IN (s.key, lower(s.label));
UPDATE leads SET source_detail = source, source = 'other'
 WHERE source IS NOT NULL AND source NOT IN (SELECT key FROM lead_sources);
ALTER TABLE leads
    ADD CONSTRAINT leads_source_fkey FOREIGN KEY (source) REFERENCES lead_sources(key);

-- 2. An EUR quote's exchange rate, frozen when the quote is set or sent, so the HUF figure
--    the customer was quoted and the order it becomes are valued at the same MNB rate.
ALTER TABLE leads
    ADD COLUMN quote_fx_rate NUMERIC(18,8) CHECK (quote_fx_rate IS NULL OR quote_fx_rate > 0),
    ADD COLUMN quote_fx_day  DATE,
    ADD CONSTRAINT leads_quote_fx_complete CHECK ((quote_fx_rate IS NULL) = (quote_fx_day IS NULL));

-- 3. Staff comments on orders and leads. A mention notifies the person named. Edited and
--    deleted comments keep their row: the thread is part of the record's history.
CREATE TABLE comments (
    id          BIGSERIAL PRIMARY KEY,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('order', 'lead')),
    entity_id   BIGINT NOT NULL,
    body        TEXT NOT NULL CHECK (char_length(btrim(body)) BETWEEN 1 AND 5000),
    created_by  BIGINT NOT NULL REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    edited_at   TIMESTAMPTZ,
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX comments_entity_idx ON comments (entity_type, entity_id, created_at)
    WHERE deleted_at IS NULL;

CREATE TABLE comment_mentions (
    comment_id BIGINT NOT NULL REFERENCES comments(id),
    user_id    BIGINT NOT NULL REFERENCES users(id),
    PRIMARY KEY (comment_id, user_id)
);

-- 4. The yard: where each vehicle stands. Places are configuration (bays, the car park,
--    the paint shop down the road); moves are history, the latest one is where it is now.
CREATE TABLE yard_locations (
    id          BIGSERIAL PRIMARY KEY,
    name        TEXT NOT NULL CHECK (btrim(name) <> ''),
    kind        TEXT NOT NULL DEFAULT 'bay' CHECK (kind IN ('bay', 'parking', 'external')),
    -- How many vehicles fit. NULL: no limit (a car park). The board warns, never refuses.
    capacity    INT CHECK (capacity IS NULL OR capacity BETWEEN 1 AND 100),
    position    INT NOT NULL DEFAULT 0,
    archived_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX yard_locations_name_idx ON yard_locations (lower(name)) WHERE archived_at IS NULL;

-- Placeholders, like the stages: rename and add the real ones in settings.
INSERT INTO yard_locations (name, kind, capacity, position) VALUES
    ('1. állás',        'bay',      1,    10),
    ('2. állás',        'bay',      1,    20),
    ('3. állás',        'bay',      1,    30),
    ('4. állás',        'bay',      1,    40),
    ('Udvar',           'parking',  NULL, 50),
    ('Fényező (külső)', 'external', NULL, 60);

CREATE TABLE vehicle_moves (
    id          BIGSERIAL PRIMARY KEY,
    vehicle_id  BIGINT NOT NULL REFERENCES vehicles(id),
    -- NULL: the vehicle left the site (handed over, or taken away).
    location_id BIGINT REFERENCES yard_locations(id),
    -- The job it was moved for, when there is one: the move shows in that job's history.
    order_id    BIGINT REFERENCES orders(id),
    note        TEXT CHECK (note IS NULL OR char_length(note) <= 500),
    moved_by    BIGINT REFERENCES users(id),
    -- clock_timestamp(), like stage history: two moves in one transaction keep their order.
    moved_at    TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX vehicle_moves_vehicle_idx ON vehicle_moves (vehicle_id, moved_at DESC, id DESC);
CREATE INDEX vehicle_moves_order_idx ON vehicle_moves (order_id) WHERE order_id IS NOT NULL;

CREATE VIEW vehicle_current_location AS
SELECT DISTINCT ON (m.vehicle_id)
       m.vehicle_id, m.location_id, m.order_id, m.moved_at, m.moved_by
FROM vehicle_moves m
ORDER BY m.vehicle_id, m.moved_at DESC, m.id DESC;

-- 5. Photos: a caption, drawn annotations, and an RFC 3161 timestamp for evidence.
--    A caption and the vehicle link are not part of an immutable image's identity, so the
--    trigger from 0004 lets both change on intake photos too.
ALTER TABLE images ADD COLUMN caption TEXT CHECK (caption IS NULL OR char_length(caption) <= 500);

-- Annotations are an overlay drawn over the display copy, never burnt into any file: the
-- original stays the evidence. Coordinates are fractions of the image (0..1).
CREATE TABLE image_annotations (
    image_id   BIGINT PRIMARY KEY REFERENCES images(id),
    shapes     JSONB NOT NULL CHECK (jsonb_typeof(shapes) = 'array'),
    updated_by BIGINT REFERENCES users(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A trusted third party's signed statement that these exact bytes (their sha256) existed
-- at `gen_time`. The whole answer is kept, so anyone can check it with `openssl ts -verify`
-- and the authority's certificate, without trusting this system.
CREATE TABLE image_timestamps (
    image_id   BIGINT PRIMARY KEY REFERENCES images(id),
    tsa_url    TEXT NOT NULL,
    response   BYTEA NOT NULL,
    gen_time   TIMESTAMPTZ NOT NULL,
    serial     TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 6. Which category a new photo falls in by default while an order is in a stage: in MEO it
--    is the completion photo the MEO gate asks for. Clients offer it next to the categories
--    anyone may file by hand.
ALTER TABLE stage_definitions
    ADD COLUMN default_image_category image_category,
    ADD CONSTRAINT stage_definitions_photo_category_orders_only
        CHECK (entity = 'order' OR default_image_category IS NULL);
UPDATE stage_definitions
   SET default_image_category = CASE key
       WHEN 'meo' THEN 'completion'::image_category
       WHEN 'completed' THEN 'completion'::image_category
       ELSE 'production'::image_category END
 WHERE entity = 'order' AND key IN ('intake', 'design', 'production', 'meo', 'completed');

-- 7. Document versions and thumbnails. A new version is a new row pointing at the one it
--    replaces; the old one is marked superseded and stays downloadable. A thumbnail is
--    rendered for drawings (DXF, the preview inside a DWG) and image files.
ALTER TABLE documents
    ADD COLUMN previous_version_id BIGINT REFERENCES documents(id),
    ADD COLUMN version             INT NOT NULL DEFAULT 1 CHECK (version >= 1),
    ADD COLUMN superseded_at       TIMESTAMPTZ,
    ADD COLUMN thumb_key           TEXT,
    ADD COLUMN thumb_error         TEXT;
CREATE UNIQUE INDEX documents_one_successor ON documents (previous_version_id)
    WHERE previous_version_id IS NOT NULL AND deleted_at IS NULL;

-- 8. Inspections: the condition of each tyre, and short video clips. Both are recorded on
--    the phone during the walkaround and synced with the rest of the draft.
CREATE TABLE inspection_tyres (
    inspection_id BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    position      TEXT NOT NULL CHECK (char_length(position) BETWEEN 1 AND 32),
    tread_mm      NUMERIC(3,1) CHECK (tread_mm IS NULL OR (tread_mm >= 0 AND tread_mm <= 30)),
    condition     TEXT NOT NULL CHECK (char_length(condition) BETWEEN 1 AND 32),
    note          TEXT CHECK (note IS NULL OR char_length(note) <= 500),
    PRIMARY KEY (inspection_id, position)
);

CREATE TABLE inspection_videos (
    id            BIGSERIAL PRIMARY KEY,
    inspection_id BIGINT NOT NULL REFERENCES inspections(id) ON DELETE CASCADE,
    document_id   BIGINT NOT NULL UNIQUE REFERENCES documents(id),
    zone_key      TEXT CHECK (zone_key IS NULL OR char_length(zone_key) BETWEEN 1 AND 64),
    duration_ms   INT CHECK (duration_ms IS NULL OR duration_ms >= 0),
    taken_at      TIMESTAMPTZ NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inspection_videos_inspection_idx ON inspection_videos (inspection_id);

-- 9. Internal incidents: damage the workshop caused (found as a "new" damage at kiadás, or
--    reported by hand), what it cost and how it was settled. Never shown to customers. A
--    rework job opened for it is linked here.
CREATE TABLE incidents (
    id              BIGSERIAL PRIMARY KEY,
    order_id        BIGINT NOT NULL REFERENCES orders(id),
    inspection_id   BIGINT REFERENCES inspections(id) ON DELETE SET NULL,
    damage_id       BIGINT REFERENCES inspection_damages(id) ON DELETE SET NULL,
    title           TEXT NOT NULL CHECK (btrim(title) <> ''),
    description     TEXT,
    cost_minor      BIGINT CHECK (cost_minor IS NULL OR cost_minor >= 0),
    currency        CHAR(3) NOT NULL DEFAULT 'HUF' CHECK (currency IN ('HUF', 'EUR')),
    -- Who caused it or answers for it, as words: not everyone on the floor has a login.
    responsible     TEXT,
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolution      TEXT,
    rework_order_id BIGINT REFERENCES orders(id),
    created_by      BIGINT REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at     TIMESTAMPTZ,
    resolved_by     BIGINT REFERENCES users(id),
    CHECK ((status = 'resolved') = (resolved_at IS NOT NULL))
);
CREATE INDEX incidents_order_idx ON incidents (order_id);
CREATE INDEX incidents_open_idx ON incidents (created_at DESC) WHERE status = 'open';
-- One incident per damage: the second click opens the first.
CREATE UNIQUE INDEX incidents_one_per_damage ON incidents (damage_id) WHERE damage_id IS NOT NULL;
CREATE TRIGGER incidents_touch BEFORE UPDATE ON incidents FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
