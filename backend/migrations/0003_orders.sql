-- Orders: the central entity. One vehicle conversion project.

-- Project/vehicle type is a reporting dimension, so it is configuration like stages.
CREATE TABLE project_types (
    id        BIGSERIAL PRIMARY KEY,
    key       TEXT NOT NULL UNIQUE CHECK (key ~ '^[a-z][a-z0-9_]*$'),
    label_hu  TEXT NOT NULL,
    position  INT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT true
);

-- Placeholders; rename in settings once the real categories are known.
INSERT INTO project_types (key, label_hu, position) VALUES
    ('refrigerated_body', 'Hűtőfelépítmény',     10),
    ('van_conversion',    'Furgon hűtősítés',    20),
    ('unit_install',      'Hűtőgép beépítés',    30),
    ('repair',            'Javítás / átalakítás', 40),
    ('other',             'Egyéb',               50);

CREATE TABLE orders (
    id              BIGSERIAL PRIMARY KEY,
    number          TEXT NOT NULL UNIQUE,            -- '2026-0042'; imported orders keep their own
    title           TEXT NOT NULL,
    partner_id      BIGINT NOT NULL REFERENCES partners(id),
    contact_id      BIGINT REFERENCES contacts(id),
    lead_id         BIGINT UNIQUE REFERENCES leads(id),
    project_type_id BIGINT REFERENCES project_types(id),
    currency        CHAR(3) NOT NULL CHECK (currency IN ('HUF', 'EUR')),
    -- The day whose MNB rate normalises this order's value in reports. Defaults to the
    -- day the order was created; never "today", or last year's numbers move every morning.
    valuation_date  DATE NOT NULL DEFAULT CURRENT_DATE,
    vehicle_make    TEXT,
    vehicle_model   TEXT,
    vehicle_plate   TEXT,
    vehicle_vin     TEXT,
    description     TEXT,
    due_date        DATE,
    assigned_to     BIGINT REFERENCES users(id),
    created_by      BIGINT REFERENCES users(id),
    minicrm_id      BIGINT UNIQUE,
    raw_import      JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Target for order_items' composite foreign key: line items always share the order's currency.
    UNIQUE (id, currency)
);
CREATE INDEX orders_title_trgm ON orders USING gin (title gin_trgm_ops);
CREATE INDEX orders_plate_idx ON orders (upper(regexp_replace(vehicle_plate, '[^A-Za-z0-9]', '', 'g')));
CREATE INDEX orders_partner_idx ON orders (partner_id);
CREATE TRIGGER orders_touch BEFORE UPDATE ON orders FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TABLE order_items (
    id          BIGSERIAL PRIMARY KEY,
    order_id    BIGINT NOT NULL,
    position    INT NOT NULL DEFAULT 0,
    description TEXT NOT NULL,
    quantity    NUMERIC(12,3) NOT NULL CHECK (quantity > 0),
    -- Minor units (fillér / eurocent). Negative allowed for discount lines.
    -- Line total = round(quantity * unit_price), half away from zero (Postgres round() semantics).
    unit_price  BIGINT NOT NULL,
    currency    CHAR(3) NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- ON UPDATE RESTRICT: an order's currency can only change while it has no items,
    -- because re-labelling eurocents as fillér is never a correction.
    FOREIGN KEY (order_id, currency) REFERENCES orders (id, currency) ON UPDATE RESTRICT
);
CREATE INDEX order_items_order_idx ON order_items (order_id, position, id);
CREATE TRIGGER order_items_touch BEFORE UPDATE ON order_items FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TABLE order_stages (
    id           BIGSERIAL PRIMARY KEY,
    order_id     BIGINT NOT NULL REFERENCES orders(id),
    stage_entity TEXT NOT NULL DEFAULT 'order' CHECK (stage_entity = 'order'),
    stage_key    TEXT NOT NULL,
    entered_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    entered_by   BIGINT REFERENCES users(id),
    note         TEXT,
    FOREIGN KEY (stage_entity, stage_key) REFERENCES stage_definitions (entity, key)
);
CREATE INDEX order_stages_order_idx ON order_stages (order_id, entered_at DESC, id DESC);

-- "We are waiting on a thing from someone." Nothing more.
CREATE TABLE blockers (
    id                     BIGSERIAL PRIMARY KEY,
    order_id               BIGINT NOT NULL REFERENCES orders(id),
    what                   TEXT NOT NULL,
    responsible_partner_id BIGINT REFERENCES partners(id),
    responsible_email      TEXT,              -- overrides the partner's email for nudges
    due_date               DATE,
    notes                  TEXT,
    nudge_enabled          BOOLEAN NOT NULL DEFAULT true,
    last_nudged_at         TIMESTAMPTZ,
    nudge_count            INT NOT NULL DEFAULT 0,
    resolved_at            TIMESTAMPTZ,
    resolved_by            BIGINT REFERENCES users(id),
    resolution_note        TEXT,
    created_by             BIGINT REFERENCES users(id),
    created_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX blockers_open_idx ON blockers (due_date) WHERE resolved_at IS NULL;
CREATE INDEX blockers_order_idx ON blockers (order_id);
CREATE INDEX blockers_partner_idx ON blockers (responsible_partner_id);
CREATE TRIGGER blockers_touch BEFORE UPDATE ON blockers FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
