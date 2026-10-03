-- Per-user notification feed: what the bell on the web and the phone's notifications show.
-- Written by the server when something happens (a lead arrives from the website); read by
-- the client, which polls. A row belongs to one user and is read or unread for that user only.
CREATE TABLE notifications (
    id         BIGSERIAL PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users(id),
    kind       TEXT NOT NULL,
    title      TEXT NOT NULL,
    body       TEXT,
    -- App path to open, e.g. /leads/42.
    link       TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at    TIMESTAMPTZ
);
CREATE INDEX notifications_user_idx ON notifications (user_id, id DESC);
CREATE INDEX notifications_unread_idx ON notifications (user_id) WHERE read_at IS NULL;
