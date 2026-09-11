-- Email: templates, the permanent send log, and the suppression list.

CREATE TABLE email_templates (
    id           BIGSERIAL PRIMARY KEY,
    key          TEXT NOT NULL UNIQUE CHECK (key ~ '^[a-z][a-z0-9_]*$'),   -- referenced by code
    name         TEXT NOT NULL,                                            -- editable label
    subject      TEXT NOT NULL,
    body         TEXT NOT NULL,          -- plain text with {{variables}}; HTML part is derived
    locale       CHAR(2) NOT NULL DEFAULT 'hu',
    is_automatic BOOLEAN NOT NULL DEFAULT false,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by   BIGINT REFERENCES users(id)
);
CREATE TRIGGER email_templates_touch BEFORE UPDATE ON email_templates FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('blocker_nudge_first', 'Hiányzó tétel – első emlékeztető',
 'Emlékeztető: {{blocker.what}} ({{order.number}})',
 'Tisztelt {{partner.name}}!

A(z) {{order.number}} számú projektünkhöz ({{order.vehicle}}) még várjuk a következőt:
{{blocker.what}}

A megbeszélt határidő: {{blocker.due_date}}.

Kérjük, jelezze, mikorra várható. Ha már elküldték, köszönjük, és kérjük, tekintse tárgytalannak ezt a levelet.

Üdvözlettel:
Autotherm', true),

('blocker_nudge_escalated', 'Hiányzó tétel – ismételt emlékeztető',
 'Sürgős: {{blocker.what}} ({{order.number}}) – {{blocker.days_overdue}} napja lejárt',
 'Tisztelt {{partner.name}}!

Ismételten jelezzük, hogy a(z) {{order.number}} számú projektünkhöz ({{order.vehicle}}) továbbra is várjuk:
{{blocker.what}}

A határidő {{blocker.due_date}} volt, azóta {{blocker.days_overdue}} nap telt el, és a projekt emiatt áll.

Kérjük, mielőbb jelezzen vissza.

Üdvözlettel:
Autotherm', true),

('lead_acknowledgement', 'Érdeklődés visszaigazolása',
 'Megkaptuk érdeklődését',
 'Tisztelt {{contact.name}}!

Köszönjük megkeresését ({{lead.title}}). Kollégánk hamarosan felveszi Önnel a kapcsolatot.

Üdvözlettel:
{{user.name}}
Autotherm', false),

('order_stage_changed', 'Projekt állapotváltozás (ügyfélnek)',
 '{{order.number}} – új állapot: {{order.stage}}',
 'Tisztelt {{partner.name}}!

Tájékoztatjuk, hogy a(z) {{order.number}} számú projekt ({{order.vehicle}}) új állapotba került: {{order.stage}}.

Üdvözlettel:
Autotherm', true),

('stalled_order_alert', 'Elakadt projekt (belső)',
 'Elakadt projekt: {{order.number}} – {{order.stage}}',
 'A(z) {{order.number}} projekt ({{order.title}}, {{partner.name}}) {{order.days_in_stage}} napja áll a következő szakaszban: {{order.stage}}.

Nyitott akadályok száma: {{order.open_blockers}}.', true);

CREATE TYPE email_status AS ENUM ('queued', 'sending', 'sent', 'failed', 'cancelled', 'needs_review');

-- Every email is a row here before it is sent, and rows are never deleted.
CREATE TABLE email_messages (
    id                 BIGSERIAL PRIMARY KEY,
    -- What it's about. A blocker nudge carries both blocker_id and its order_id,
    -- so it shows up in the order's correspondence history.
    order_id           BIGINT REFERENCES orders(id),
    lead_id            BIGINT REFERENCES leads(id),
    partner_id         BIGINT REFERENCES partners(id),
    blocker_id         BIGINT REFERENCES blockers(id),

    template_key       TEXT,                         -- null for free-composed mail
    trigger            TEXT NOT NULL,                -- 'manual' | 'nudge_blocker' | 'stage_changed' | 'stalled_order'
    is_automatic       BOOLEAN NOT NULL,
    sent_by            BIGINT REFERENCES users(id),  -- null when automatic
    -- Automatic mail uses keys like 'nudge:17:3' so a double-run scheduler cannot queue twice.
    idempotency_key    TEXT UNIQUE,

    to_address         TEXT NOT NULL,
    cc                 TEXT[] NOT NULL DEFAULT '{}',
    from_address       TEXT NOT NULL,
    reply_to           TEXT,
    subject            TEXT NOT NULL,
    body_html          TEXT NOT NULL,
    body_text          TEXT NOT NULL,
    -- Requested: [{"document_id": 5}]. After sending: what was attached and what became a link.
    attachments        JSONB NOT NULL DEFAULT '[]'::jsonb,

    status             email_status NOT NULL DEFAULT 'queued',
    provider_id        TEXT,                         -- Message-ID header we generated
    error              TEXT,
    attempts           INT NOT NULL DEFAULT 0,
    queued_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    send_after         TIMESTAMPTZ NOT NULL DEFAULT now(),
    sending_started_at TIMESTAMPTZ,
    sent_at            TIMESTAMPTZ,
    cancelled_at       TIMESTAMPTZ,
    cancelled_by       BIGINT REFERENCES users(id),

    CHECK (num_nonnulls(order_id, lead_id, partner_id) <= 1),
    CHECK (blocker_id IS NULL OR order_id IS NOT NULL),
    CHECK (is_automatic = (sent_by IS NULL))
);
CREATE INDEX email_messages_order_idx ON email_messages (order_id, queued_at DESC) WHERE order_id IS NOT NULL;
CREATE INDEX email_messages_lead_idx ON email_messages (lead_id, queued_at DESC) WHERE lead_id IS NOT NULL;
CREATE INDEX email_messages_partner_idx ON email_messages (partner_id, queued_at DESC) WHERE partner_id IS NOT NULL;
CREATE INDEX email_messages_recipient_idx ON email_messages (lower(to_address), queued_at DESC);
CREATE INDEX email_messages_attention_idx ON email_messages (queued_at DESC)
    WHERE status IN ('failed', 'needs_review');

-- Addresses that asked not to receive automatic mail. Checked before every automatic send;
-- manual mail may override, automatic never does.
CREATE TABLE email_suppressions (
    email      TEXT PRIMARY KEY CHECK (email = lower(email)),
    reason     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by BIGINT REFERENCES users(id)
);
