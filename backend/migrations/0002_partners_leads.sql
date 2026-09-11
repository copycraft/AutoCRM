-- Partners (business or person), their contacts, configurable stages, and leads.

CREATE TYPE partner_kind AS ENUM ('business', 'person');

CREATE TABLE partners (
    id               BIGSERIAL PRIMARY KEY,
    kind             partner_kind NOT NULL,
    name             TEXT NOT NULL,
    tax_number       TEXT,                      -- Hungarian: 12345678-1-23
    eu_tax_number    TEXT,                      -- Austrian/German customers
    country          CHAR(2) NOT NULL DEFAULT 'HU',
    default_currency CHAR(3) NOT NULL DEFAULT 'HUF' CHECK (default_currency IN ('HUF', 'EUR')),
    email            TEXT,
    phone            TEXT,
    website          TEXT,
    postal_code      TEXT,
    city             TEXT,
    address_line     TEXT,
    notes            TEXT,
    minicrm_id       BIGINT UNIQUE,             -- migration provenance, keep forever
    raw_import       JSONB,                     -- unmapped MiniCRM fields, never silently dropped
    archived_at      TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX partners_name_trgm ON partners USING gin (name gin_trgm_ops);
CREATE TRIGGER partners_touch BEFORE UPDATE ON partners FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TABLE contacts (
    id          BIGSERIAL PRIMARY KEY,
    partner_id  BIGINT NOT NULL REFERENCES partners(id),
    name        TEXT NOT NULL,
    email       TEXT,
    phone       TEXT,
    position    TEXT,
    notes       TEXT,
    minicrm_id  BIGINT UNIQUE,
    raw_import  JSONB,
    archived_at TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX contacts_partner_idx ON contacts (partner_id);
CREATE TRIGGER contacts_touch BEFORE UPDATE ON contacts FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TYPE image_category AS ENUM ('intake', 'production', 'completion', 'marketing');

-- Stages are configuration, not code. Code refers to `key`; humans see `label_hu`.
CREATE TABLE stage_definitions (
    id                      BIGSERIAL PRIMARY KEY,
    entity                  TEXT NOT NULL CHECK (entity IN ('lead', 'order')),
    key                     TEXT NOT NULL CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    label_hu                TEXT NOT NULL,
    position                INT NOT NULL,
    -- Gate: an order cannot move forward past this stage until it has at least
    -- min_images images of required_image_category.
    min_images              INT NOT NULL DEFAULT 0 CHECK (min_images >= 0),
    required_image_category image_category,
    is_terminal             BOOLEAN NOT NULL DEFAULT false,
    -- Exit stages (lost, cancelled) are reachable from any open stage and skip gates.
    is_exit                 BOOLEAN NOT NULL DEFAULT false,
    stall_after_days        INT CHECK (stall_after_days IS NULL OR stall_after_days > 0),
    is_active               BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (entity, key),
    CHECK (min_images = 0 OR required_image_category IS NOT NULL),
    CHECK (NOT is_exit OR is_terminal)
);

-- Placeholder stages. Rename label_hu freely in settings; never change key.
INSERT INTO stage_definitions (entity, key, label_hu, position, is_terminal, is_exit, stall_after_days) VALUES
    ('lead', 'new',       'Új',            10, false, false, 3),
    ('lead', 'contacted', 'Megkeresve',    20, false, false, 7),
    ('lead', 'quoted',    'Árajánlat',     30, false, false, 21),
    ('lead', 'won',       'Megnyert',      40, true,  false, NULL),
    ('lead', 'lost',      'Elveszett',     50, true,  true,  NULL);

INSERT INTO stage_definitions (entity, key, label_hu, position, min_images, required_image_category, is_terminal, is_exit, stall_after_days) VALUES
    ('order', 'intake',     'Átvétel',        10, 0, NULL,         false, false, 7),
    ('order', 'design',     'Tervezés',       20, 0, NULL,         false, false, 21),
    ('order', 'production', 'Gyártás',        30, 0, NULL,         false, false, 30),
    ('order', 'meo',        'MEO',            40, 1, 'completion', false, false, 7),
    ('order', 'completed',  'Kész',           50, 0, NULL,         true,  false, NULL),
    ('order', 'cancelled',  'Törölve',        60, 0, NULL,         true,  true,  NULL);

CREATE TABLE leads (
    id            BIGSERIAL PRIMARY KEY,
    title         TEXT NOT NULL,
    partner_id    BIGINT REFERENCES partners(id),   -- null while the enquirer is not yet a partner
    contact_id    BIGINT REFERENCES contacts(id),
    contact_name  TEXT,
    contact_email TEXT,
    contact_phone TEXT,
    source        TEXT,                              -- web, phone, email, referral, ...
    description   TEXT,
    assigned_to   BIGINT REFERENCES users(id),
    created_by    BIGINT REFERENCES users(id),
    minicrm_id    BIGINT UNIQUE,
    raw_import    JSONB,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX leads_title_trgm ON leads USING gin (title gin_trgm_ops);
CREATE TRIGGER leads_touch BEFORE UPDATE ON leads FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- Stage history, not a status column. Current stage = latest row; durations = window function.
-- The (stage_entity, stage_key) foreign key makes a typo'd or wrong-entity stage impossible.
CREATE TABLE lead_stages (
    id           BIGSERIAL PRIMARY KEY,
    lead_id      BIGINT NOT NULL REFERENCES leads(id),
    stage_entity TEXT NOT NULL DEFAULT 'lead' CHECK (stage_entity = 'lead'),
    stage_key    TEXT NOT NULL,
    entered_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    entered_by   BIGINT REFERENCES users(id),
    note         TEXT,
    FOREIGN KEY (stage_entity, stage_key) REFERENCES stage_definitions (entity, key)
);
CREATE INDEX lead_stages_lead_idx ON lead_stages (lead_id, entered_at DESC, id DESC);
