-- One batch: automatic payment reminders, customer replies read from the mailbox, quote
-- expiry alerts, lost reasons, newsletter tracking and scheduling, list signup forms, HR
-- personal data and documents.

-- 1. Payment reminders reuse the follow-up sequence table: a step is either for quotes
--    (days after the quotation) or for invoices (days after the payment deadline).
ALTER TABLE followup_steps ADD COLUMN kind TEXT NOT NULL DEFAULT 'quote' CHECK (kind IN ('quote', 'invoice'));
UPDATE email_templates SET is_automatic = true WHERE key IN ('invoice_overdue_1', 'invoice_overdue_2');
INSERT INTO followup_steps (label, delay_days, template_key, kind) VALUES
    ('3 nap', 3, 'invoice_overdue_1', 'invoice'),
    ('2 hét', 14, 'invoice_overdue_2', 'invoice');

-- Which reminder went out for which invoice; one per step, ever.
CREATE TABLE invoice_reminders (
    id         BIGSERIAL PRIMARY KEY,
    invoice_id BIGINT NOT NULL REFERENCES invoices(id),
    step_id    BIGINT NOT NULL REFERENCES followup_steps(id),
    email_id   BIGINT REFERENCES email_messages(id),
    -- sent | skipped
    status     TEXT NOT NULL CHECK (status IN ('sent', 'skipped')),
    note       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (invoice_id, step_id)
);
-- The office can stop reminders for one invoice (a customer who pays late by agreement).
ALTER TABLE invoices ADD COLUMN reminders_off BOOLEAN NOT NULL DEFAULT false;

-- 3. Mail read back from the sales mailbox and matched to what it is about. A reply to a
--    quotation stops its follow-ups.
CREATE TABLE inbound_emails (
    id           BIGSERIAL PRIMARY KEY,
    -- The Message-ID header; a message is stored once however often the box is read.
    message_id   TEXT NOT NULL UNIQUE,
    in_reply_to  TEXT,
    from_address TEXT NOT NULL,
    from_name    TEXT,
    subject      TEXT NOT NULL DEFAULT '',
    body_text    TEXT NOT NULL DEFAULT '',
    received_at  TIMESTAMPTZ NOT NULL,
    lead_id      BIGINT REFERENCES leads(id),
    order_id     BIGINT REFERENCES orders(id),
    partner_id   BIGINT REFERENCES partners(id),
    -- The email of ours it answers, when its headers say so.
    reply_to_email_id BIGINT REFERENCES email_messages(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX inbound_emails_lead_idx ON inbound_emails (lead_id) WHERE lead_id IS NOT NULL;
CREATE INDEX inbound_emails_order_idx ON inbound_emails (order_id) WHERE order_id IS NOT NULL;
CREATE INDEX inbound_emails_partner_idx ON inbound_emails (partner_id) WHERE partner_id IS NOT NULL;
-- Where reading stopped last time, per mailbox folder.
CREATE TABLE mailbox_cursor (
    folder       TEXT PRIMARY KEY,
    uid_validity BIGINT NOT NULL,
    last_uid     BIGINT NOT NULL
);

-- 7. One quote-expiry alert per lead per validity date.
CREATE TABLE quote_expiry_alerts (
    lead_id     BIGINT NOT NULL REFERENCES leads(id),
    valid_until DATE NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (lead_id, valid_until)
);

-- 8. Why a lead was lost: a list the office edits, seeded from MiniCRM's "Nem vevők".
CREATE TABLE lost_reasons (
    id          BIGSERIAL PRIMARY KEY,
    label       TEXT NOT NULL CHECK (btrim(label) <> ''),
    position    INT NOT NULL DEFAULT 0,
    archived_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX lost_reasons_label_idx ON lost_reasons (lower(label)) WHERE archived_at IS NULL;
INSERT INTO lost_reasons (label, position) VALUES
    ('Mástól vásárolt', 10), ('Drága volt', 20), ('Minőségi kifogás', 30), ('Bizalom hiánya', 40),
    ('Nem érdekli', 50), ('Elérhetetlen', 60), ('Később vásárol', 70), ('Csődbe ment', 80),
    ('Használt hűtőautót vásárolt', 90), ('Egyéb ok', 100);
ALTER TABLE leads ADD COLUMN lost_reason_id BIGINT REFERENCES lost_reasons(id);

-- 10/11. A newsletter is one email per subscriber now: each with its own unsubscribe link,
--        an open pixel and tracked links, sent now or at a set time.
CREATE TABLE newsletter_sends (
    id           BIGSERIAL PRIMARY KEY,
    subject      TEXT NOT NULL,
    body         TEXT NOT NULL DEFAULT '',
    body_markdown BOOLEAN NOT NULL DEFAULT false,
    hero         TEXT,
    tag_ids      BIGINT[] NOT NULL DEFAULT '{}',
    attachment_document_ids BIGINT[] NOT NULL DEFAULT '{}',
    embed_document_ids      BIGINT[] NOT NULL DEFAULT '{}',
    send_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    recipients   INT NOT NULL DEFAULT 0,
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    cancelled_at TIMESTAMPTZ
);
ALTER TABLE email_messages
    ADD COLUMN newsletter_send_id BIGINT REFERENCES newsletter_sends(id),
    ADD COLUMN tracking_token     TEXT UNIQUE,
    ADD COLUMN opened_at          TIMESTAMPTZ,
    ADD COLUMN open_count         INT NOT NULL DEFAULT 0;
CREATE INDEX email_messages_newsletter_idx ON email_messages (newsletter_send_id) WHERE newsletter_send_id IS NOT NULL;
CREATE TABLE email_clicks (
    id         BIGSERIAL PRIMARY KEY,
    email_id   BIGINT NOT NULL REFERENCES email_messages(id),
    url        TEXT NOT NULL,
    clicked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX email_clicks_email_idx ON email_clicks (email_id);

-- 12. A website form can sign readers up to particular lists; the lists are applied when
--     the reader confirms.
ALTER TABLE newsletter_subscriptions ADD COLUMN pending_tag_ids BIGINT[] NOT NULL DEFAULT '{}';

-- 16. The papers HR collects. One row per employee, every field optional: what is missing
--     decides the "…adatokra vár" statuses.
CREATE TABLE employee_details (
    employee_id              BIGINT PRIMARY KEY REFERENCES employees(id),
    -- Személyi adatok
    birth_name               TEXT,
    birth_date               DATE,
    birth_place              TEXT,
    mother_name              TEXT,
    nationality              TEXT,
    address                  TEXT,
    id_card_number           TEXT,
    -- Pénzügyi adatok
    tax_id                   TEXT,
    taj_number               TEXT,
    bank_account             TEXT,
    -- Családi, szociális adatok
    marital_status           TEXT,
    children_count           INT CHECK (children_count IS NULL OR children_count BETWEEN 0 AND 30),
    emergency_contact_name   TEXT,
    emergency_contact_phone  TEXT,
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER employee_details_touch BEFORE UPDATE ON employee_details
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
-- Which statuses the data drives. The others (absent, partner member, left) are HR's call.
ALTER TABLE employee_statuses ADD COLUMN auto_key TEXT UNIQUE
    CHECK (auto_key IN ('missing_personal', 'missing_financial', 'missing_family', 'complete'));
UPDATE employee_statuses SET auto_key = 'missing_personal' WHERE label = 'Személyi adatokra vár';
UPDATE employee_statuses SET auto_key = 'missing_financial' WHERE label = 'Pénzügyi adatokra vár';
UPDATE employee_statuses SET auto_key = 'missing_family' WHERE label = 'Családi, szociális adatokra vár';
UPDATE employee_statuses SET auto_key = 'complete' WHERE label = 'Aktív munkavállaló';

-- 17. Documents with an expiry: medical fitness, contracts, licences.
CREATE TABLE employee_documents (
    id          BIGSERIAL PRIMARY KEY,
    employee_id BIGINT NOT NULL REFERENCES employees(id),
    kind        TEXT NOT NULL CHECK (kind IN ('medical', 'contract', 'licence', 'training', 'other')),
    title       TEXT NOT NULL CHECK (btrim(title) <> ''),
    valid_until DATE,
    file_key    TEXT,
    file_name   TEXT,
    file_type   TEXT,
    file_size   BIGINT,
    notes       TEXT,
    created_by  BIGINT REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at  TIMESTAMPTZ
);
CREATE INDEX employee_documents_employee_idx ON employee_documents (employee_id) WHERE deleted_at IS NULL;
CREATE INDEX employee_documents_expiry_idx ON employee_documents (valid_until) WHERE deleted_at IS NULL;
-- One alert per document per stage (30 days before, on expiry).
CREATE TABLE employee_document_alerts (
    document_id BIGINT NOT NULL REFERENCES employee_documents(id),
    stage       TEXT NOT NULL CHECK (stage IN ('soon', 'expired')),
    valid_until DATE NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (document_id, stage, valid_until)
);
