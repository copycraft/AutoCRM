-- One batch: partial payments, statement and invoice language, email signatures and
-- threaded replies, newsletter language variants, HR paperwork (sick notes, on- and
-- offboarding), the cooling unit's serial, the weekly report, ad-platform conversions,
-- calendar feeds, two-factor sign-in and new-device alerts.

-- 1. Partial payments on outgoing invoices. `paid_amount` is the running sum; `paid_at` is
--    set when it reaches the gross (as before, cash is paid in full on issue).
ALTER TABLE invoices ADD COLUMN paid_amount BIGINT NOT NULL DEFAULT 0 CHECK (paid_amount >= 0);
UPDATE invoices SET paid_amount = gross_amount WHERE paid_at IS NOT NULL AND gross_amount > 0;

CREATE TABLE invoice_payments (
    id           BIGSERIAL PRIMARY KEY,
    invoice_id   BIGINT NOT NULL REFERENCES invoices(id),
    amount_minor BIGINT NOT NULL CHECK (amount_minor > 0),
    paid_on      DATE NOT NULL,
    method       TEXT NOT NULL DEFAULT 'TRANSFER' CHECK (method IN ('TRANSFER', 'CASH', 'CARD', 'OTHER')),
    note         TEXT CHECK (note IS NULL OR char_length(note) <= 500),
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX invoice_payments_invoice_idx ON invoice_payments (invoice_id, paid_on);

CREATE OR REPLACE FUNCTION invoices_cash_paid() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.status = 'issued' AND NEW.kind = 'invoice' AND NEW.payment_method = 'CASH'
       AND NEW.paid_at IS NULL
       AND (TG_OP = 'INSERT' OR OLD.status IS DISTINCT FROM 'issued') THEN
        NEW.paid_at := coalesce(NEW.issued_at, now());
        NEW.paid_amount := greatest(NEW.paid_amount, NEW.gross_amount);
    END IF;
    RETURN NEW;
END $$;

-- 2. The language a partner's invoice and proforma PDFs are rendered in (the sidecar
--    renders hu, en, de). NULL: the sidecar's default.
ALTER TABLE partners ADD COLUMN invoice_language TEXT CHECK (invoice_language IN ('hu', 'en', 'de'));

-- 3. Per user: the signature under manual mail, and the secret of the calendar feed.
ALTER TABLE user_settings
    ADD COLUMN email_signature TEXT CHECK (email_signature IS NULL OR char_length(email_signature) <= 2000),
    ADD COLUMN calendar_token  TEXT UNIQUE;

-- 4. A reply written in the CRM threads under the customer's message in their mail app.
ALTER TABLE email_messages
    ADD COLUMN in_reply_to    TEXT,
    ADD COLUMN reference_ids  TEXT;

-- 5. Newsletters in the reader's language: a send can carry one variant per language, and
--    a reader can have a language (from the signup form, an import or by hand).
ALTER TABLE newsletter_subscriptions ADD COLUMN language TEXT CHECK (language IS NULL OR language ~ '^[a-z]{2}$');
CREATE TABLE newsletter_send_variants (
    send_id       BIGINT NOT NULL REFERENCES newsletter_sends(id) ON DELETE CASCADE,
    language      TEXT NOT NULL CHECK (language ~ '^[a-z]{2}$'),
    subject       TEXT NOT NULL,
    body          TEXT NOT NULL DEFAULT '',
    body_markdown BOOLEAN NOT NULL DEFAULT false,
    hero          TEXT,
    PRIMARY KEY (send_id, language)
);

-- 6. HR: the sick-leave paper on the absence; checklists for joining and leaving; the CRM
--    account an employee uses, so leaving can switch it off.
ALTER TABLE absences
    ADD COLUMN file_key  TEXT,
    ADD COLUMN file_name TEXT,
    ADD COLUMN file_type TEXT,
    ADD COLUMN file_size BIGINT;

ALTER TABLE employees ADD COLUMN user_id BIGINT UNIQUE REFERENCES users(id);

CREATE TABLE hr_checklist_items (
    id          BIGSERIAL PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('onboarding', 'offboarding')),
    title       TEXT NOT NULL CHECK (btrim(title) <> ''),
    -- Days after the checklist is started that the task is due.
    due_days    INT NOT NULL DEFAULT 0 CHECK (due_days BETWEEN 0 AND 365),
    position    INT NOT NULL DEFAULT 0,
    archived_at TIMESTAMPTZ
);
INSERT INTO hr_checklist_items (kind, title, due_days, position) VALUES
    ('onboarding', 'Munkaszerződés aláírása', 0, 10),
    ('onboarding', 'Bejelentés a NAV felé (T1041)', 0, 20),
    ('onboarding', 'Üzemorvosi alkalmassági vizsgálat', 3, 30),
    ('onboarding', 'Munkavédelmi és tűzvédelmi oktatás', 0, 40),
    ('onboarding', 'Munkaköri leírás átadása', 1, 50),
    ('onboarding', 'Védőfelszerelés, munkaruha kiadása', 1, 60),
    ('onboarding', 'Kulcsok, belépő átadása', 0, 70),
    ('onboarding', 'Személyi, pénzügyi, családi adatok bekérése', 5, 80),
    ('offboarding', 'Munkaeszközök, kulcsok, belépő visszavétele', 0, 10),
    ('offboarding', 'Védőfelszerelés, munkaruha visszavétele', 0, 20),
    ('offboarding', 'Kilépő igazolások kiadása', 0, 30),
    ('offboarding', 'Kijelentés a NAV felé', 0, 40),
    ('offboarding', 'Utolsó bérelszámolás, szabadságmegváltás', 5, 50),
    ('offboarding', 'Rendszerhozzáférések megszüntetése', 0, 60);

ALTER TABLE tasks DROP CONSTRAINT tasks_entity_type_check;
ALTER TABLE tasks ADD CONSTRAINT tasks_entity_type_check
    CHECK (entity_type IN ('order', 'lead', 'partner', 'employee'));

-- 7. The cooling unit's serial number, scanned off its plate on the phone.
ALTER TABLE order_specs ADD COLUMN cooling_unit_serial TEXT
    CHECK (cooling_unit_serial IS NULL OR char_length(cooling_unit_serial) <= 100);
ALTER TABLE order_specs DROP CONSTRAINT order_specs_cooling_fields_only;
ALTER TABLE order_specs ADD CONSTRAINT order_specs_cooling_fields_only CHECK (
    form = 'cooling'
    OR num_nonnulls(cooling_unit_make, cooling_unit_model, atp_class, compartments,
                    defrost, electric_standby, cooling_unit_serial) = 0
);

-- 8. The Monday report: who gets it.
ALTER TABLE settings ADD COLUMN weekly_report_recipients TEXT[] NOT NULL DEFAULT '{}';

-- 9. Ad platforms. The click ids a website lead arrived with, so a won deal can be reported
--    back as a conversion (Google Ads offline import, Meta Conversions API). One report per
--    lead and platform.
ALTER TABLE lead_attribution
    ADD COLUMN gclid  TEXT CHECK (gclid IS NULL OR char_length(gclid) <= 200),
    ADD COLUMN fbclid TEXT CHECK (fbclid IS NULL OR char_length(fbclid) <= 300);
CREATE TABLE ad_conversions (
    lead_id    BIGINT NOT NULL REFERENCES leads(id),
    platform   TEXT NOT NULL CHECK (platform IN ('meta')),
    order_id   BIGINT REFERENCES orders(id),
    sent_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    response   TEXT,
    PRIMARY KEY (lead_id, platform)
);
-- Facebook/Instagram lead ads: each form submission is filed once.
CREATE TABLE meta_leadgen (
    leadgen_id TEXT PRIMARY KEY,
    lead_id    BIGINT REFERENCES leads(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO lead_sources (key, label, position, is_system)
VALUES ('meta_ads', 'Facebook/Instagram hirdetés', 120, true)
ON CONFLICT (key) DO NOTHING;

-- 10. Two-factor sign-in: an authenticator app's one-time codes (RFC 6238). The secret is
--     sealed with the server's key; `totp_last_step` stops a code being used twice.
ALTER TABLE users
    ADD COLUMN totp_secret     TEXT,
    ADD COLUMN totp_enabled_at TIMESTAMPTZ,
    ADD COLUMN totp_last_step  BIGINT;

-- 11. The letter that tells someone their account was signed in to from a new device.
INSERT INTO email_templates (key, name, subject, body, is_automatic, category, folder) VALUES
('new_device_login', 'Bejelentkezés új eszközről (munkatársnak)',
 'Új bejelentkezés az AutoCRM-be',
 'Kedves {{user.name}}!

Az AutoCRM-fiókjába most bejelentkeztek egy eddig nem használt eszközről.

Eszköz: {{login.device}}
IP-cím: {{login.ip}}
Időpont: {{login.time}}

Ha Ön volt, nincs teendője. Ha nem, változtassa meg azonnal a jelszavát, és szóljon a rendszergazdának.

AutoCRM', true, 'general', 'workflow');
