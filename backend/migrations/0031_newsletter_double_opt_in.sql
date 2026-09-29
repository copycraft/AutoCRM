-- Newsletter double opt-in: a website signup is a request, not a subscription.
--
-- Until now the public endpoint subscribed whatever address the form was given, and a
-- signup for an address that had unsubscribed cleared the opt-out. Anyone could put
-- anyone on the list, and there was no record that the owner of the address agreed.
-- A website signup now stores a pending row and mails a confirmation link; only the
-- click (`confirmed_at`) makes the address part of the audience.
--
-- Active means `confirmed_at IS NOT NULL AND unsubscribed_at IS NULL`. A signup for an
-- address that unsubscribed keeps `unsubscribed_at` until its link is clicked, so the
-- opt-out stands unless the owner of the address says otherwise.
--
-- Office hand-adds are confirmed on insert: the office adds people who agreed in
-- person or in writing, and is accountable for that record.

ALTER TABLE newsletter_subscriptions
    ADD COLUMN confirmed_at         TIMESTAMPTZ,
    ADD COLUMN confirm_token        TEXT UNIQUE,
    ADD COLUMN confirm_requested_at TIMESTAMPTZ;

-- Rows from before this migration were active without a confirmation step. They stay
-- active: their consent was collected under the old flow and there is no way to ask
-- again retroactively without mailing everyone. Their `confirmed_at` is the signup time.
UPDATE newsletter_subscriptions SET confirmed_at = subscribed_at;

INSERT INTO email_templates (key, name, subject, body, is_automatic) VALUES
('newsletter_confirm', 'Hírlevél: feliratkozás megerősítése',
 'Kérjük, erősítse meg feliratkozását az Autotherm hírlevélre',
 'Kedves Olvasó!

Valaki (remélhetőleg Ön) feliratkoztatta ezt a címet az Autotherm hírlevelére. A feliratkozás megerősítéséhez kattintson az alábbi linkre:

{{newsletter.confirm_url}}

A link 7 napig érvényes. Ha nem Ön iratkozott fel, nincs teendője: megerősítés nélkül nem küldünk hírlevelet erre a címre.

Üdvözlettel:
Autotherm', true);
