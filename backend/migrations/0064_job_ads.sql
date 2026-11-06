-- Job ads: where the applicants came from, for our own analytics.
--
-- HR runs a job listing as ads on Facebook, Instagram, Google… Each ad is filed here with
-- what it cost, and gets its own tracked link to paste into the ad. The listing's page
-- counts visits per ad (a number per day, nothing about the visitor), and an application
-- remembers the ad and the channel it came through. Nothing goes back to the ad
-- platforms: the numbers are ours.
CREATE TABLE job_ads (
    id           BIGSERIAL PRIMARY KEY,
    posting_id   BIGINT NOT NULL REFERENCES job_postings(id) ON DELETE CASCADE,
    platform     TEXT NOT NULL CHECK (platform IN ('facebook', 'instagram', 'google', 'tiktok', 'other')),
    name         TEXT NOT NULL CHECK (btrim(name) <> ''),
    -- The ad as it ran, for the record.
    headline     TEXT,
    body         TEXT,
    -- What it cost, in HUF fillér.
    spend_minor  BIGINT CHECK (spend_minor IS NULL OR spend_minor >= 0),
    starts_on    DATE,
    ends_on      DATE,
    created_by   BIGINT REFERENCES users(id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at  TIMESTAMPTZ,
    CHECK (ends_on IS NULL OR starts_on IS NULL OR ends_on >= starts_on)
);
CREATE INDEX job_ads_posting_idx ON job_ads (posting_id);

-- Visits to a listing's page per day, by ad (null: not through one of our ads) and channel.
CREATE TABLE job_visits (
    posting_id BIGINT NOT NULL REFERENCES job_postings(id) ON DELETE CASCADE,
    ad_id      BIGINT REFERENCES job_ads(id) ON DELETE SET NULL,
    channel    TEXT NOT NULL,
    day        DATE NOT NULL,
    visits     INT NOT NULL DEFAULT 0 CHECK (visits >= 0)
);
CREATE UNIQUE INDEX job_visits_key ON job_visits (posting_id, COALESCE(ad_id, 0), channel, day);

ALTER TABLE job_applications
    ADD COLUMN ad_id        BIGINT REFERENCES job_ads(id) ON DELETE SET NULL,
    ADD COLUMN channel      TEXT,
    ADD COLUMN utm_source   TEXT CHECK (utm_source IS NULL OR char_length(utm_source) <= 200),
    ADD COLUMN utm_medium   TEXT CHECK (utm_medium IS NULL OR char_length(utm_medium) <= 200),
    ADD COLUMN utm_campaign TEXT CHECK (utm_campaign IS NULL OR char_length(utm_campaign) <= 200),
    ADD COLUMN referrer     TEXT CHECK (referrer IS NULL OR char_length(referrer) <= 500);
