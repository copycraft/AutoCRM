-- Global search: the folded form every word is also matched against. Must agree with
-- domain::search_terms::fold: lower-case, Hungarian accents removed, only [a-z0-9] kept.
CREATE FUNCTION search_fold(text) RETURNS text
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$
    SELECT regexp_replace(translate(lower($1), 'áéíóöőúüű', 'aeiooouuu'), '[^a-z0-9]', '', 'g')
$$;

-- Where a website lead came from. A separate table: the lead itself stays as it was, and
-- only leads the website filed have a row here. `channel` is worked out by the server from
-- the other columns (domain::attribution), so reports group on one reliable value.
CREATE TABLE lead_attribution (
    lead_id      BIGINT PRIMARY KEY REFERENCES leads(id),
    channel      TEXT NOT NULL CHECK (channel IN
                     ('paid', 'organic', 'social', 'email', 'referral', 'direct')),
    utm_source   TEXT,
    utm_medium   TEXT,
    utm_campaign TEXT,
    referrer     TEXT,
    landing_page TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX lead_attribution_channel_idx ON lead_attribution (channel);
