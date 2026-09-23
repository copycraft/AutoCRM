-- Newsletter: who may be mailed in bulk, and where the bulk addresses go.
--
-- `newsletter_subscriptions` is the list the office sees under settings. Website
-- signups arrive through the public endpoint (source 'website'); hand-added rows say
-- 'office'. Unsubscribing sets `unsubscribed_at` rather than deleting: the address must
-- stay known, or the next import would resubscribe someone who opted out.
--
-- `email_messages.bcc` carries a newsletter blast's recipients. One row per blast, not
-- one per recipient: the blast is a single send with everyone in BCC, so it is stored
-- the way it is sent.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE newsletter_subscriptions (
    id                BIGSERIAL PRIMARY KEY,
    email             TEXT NOT NULL UNIQUE,
    name              TEXT NOT NULL DEFAULT '',
    source            TEXT NOT NULL DEFAULT 'office',
    subscribed_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    unsubscribed_at   TIMESTAMPTZ,
    unsubscribe_token TEXT NOT NULL UNIQUE DEFAULT encode(gen_random_bytes(24), 'hex'),
    CONSTRAINT newsletter_email CHECK (email <> '')
);

ALTER TABLE email_messages ADD COLUMN bcc TEXT[] NOT NULL DEFAULT '{}';
