-- Follow-up emails after a quotation: "did you get our offer?" one week, two weeks and a
-- month later, unless the enquiry is decided first.
--
-- `followup_steps` is the default sequence the office edits (when, and which template);
-- sending a quotation schedules its steps for that lead as `lead_followups` rows, counted
-- from the send. The office can add, drop or cancel them per lead. A row goes out as an
-- automatic email when due: the kill switch, send window, per-recipient cap and the
-- suppression list all apply. A lead that is won, lost or converted gets no more.

INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('quote_followup_1', 'Árajánlat utánkövetés: 1 hét',
 'Megkapta árajánlatunkat? ({{lead.title}})',
 'Tisztelt Ügyfelünk!

Egy hete küldtük el árajánlatunkat ({{lead.title}}). Szeretnénk megkérdezni, megkapta-e, és van-e vele kapcsolatban kérdése.

Ha bármit pontosítana, módosítana az ajánlaton, válaszoljon erre a levélre, és hamarosan jelentkezünk.

Üdvözlettel:
Autotherm', true),
('quote_followup_2', 'Árajánlat utánkövetés: 2 hét',
 'Árajánlatunk: {{lead.title}}',
 'Tisztelt Ügyfelünk!

Két hete küldtük el árajánlatunkat ({{lead.title}}). Érdeklődnénk, sikerült-e döntést hozni, vagy segíthetünk-e további információval.

Szívesen egyeztetünk telefonon is: válaszoljon erre a levélre, és visszahívjuk.

Üdvözlettel:
Autotherm', true),
('quote_followup_3', 'Árajánlat utánkövetés: 1 hónap',
 'Még aktuális? {{lead.title}}',
 'Tisztelt Ügyfelünk!

Egy hónapja küldtük el árajánlatunkat ({{lead.title}}). Ha a terv még aktuális, szívesen frissítjük az ajánlatot a mai árakkal és határidőkkel.

Ha időközben máshogy döntött, egy rövid válasz is sokat segít nekünk.

Üdvözlettel:
Autotherm', true);

CREATE TABLE followup_steps (
    id           BIGSERIAL PRIMARY KEY,
    label        TEXT NOT NULL CHECK (btrim(label) <> ''),
    -- Days after the quotation was sent.
    delay_days   INT NOT NULL CHECK (delay_days BETWEEN 1 AND 365),
    template_key TEXT NOT NULL REFERENCES email_templates(key),
    is_active    BOOLEAN NOT NULL DEFAULT true,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO followup_steps (label, delay_days, template_key) VALUES
    ('1 hét',   7,  'quote_followup_1'),
    ('2 hét',   14, 'quote_followup_2'),
    ('1 hónap', 30, 'quote_followup_3');

CREATE TABLE lead_followups (
    id           BIGSERIAL PRIMARY KEY,
    lead_id      BIGINT NOT NULL REFERENCES leads(id),
    label        TEXT NOT NULL,
    template_key TEXT NOT NULL REFERENCES email_templates(key),
    due_at       TIMESTAMPTZ NOT NULL,
    status       TEXT NOT NULL DEFAULT 'scheduled'
                     CHECK (status IN ('scheduled', 'sent', 'cancelled', 'skipped')),
    -- The email it became, once sent.
    email_id     BIGINT REFERENCES email_messages(id),
    -- Why it was cancelled or skipped, in words.
    note         TEXT,
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX lead_followups_due_idx ON lead_followups (due_at) WHERE status = 'scheduled';
CREATE INDEX lead_followups_lead_idx ON lead_followups (lead_id, due_at);
CREATE TRIGGER lead_followups_touch BEFORE UPDATE ON lead_followups
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
