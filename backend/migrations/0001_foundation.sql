-- Foundation: staff accounts, sessions, audit trail, settings, background jobs.
-- Migrations are forward-only. Never edit a migration that has run anywhere; add a new one.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Keeps updated_at honest without every UPDATE statement having to remember it.
CREATE FUNCTION touch_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TYPE user_role AS ENUM ('admin', 'office', 'designer', 'viewer');

CREATE TABLE users (
    id                   BIGSERIAL PRIMARY KEY,
    email                TEXT NOT NULL,
    display_name         TEXT NOT NULL,
    role                 user_role NOT NULL,
    password_hash        TEXT NOT NULL,              -- Argon2id PHC string
    is_active            BOOLEAN NOT NULL DEFAULT true,
    must_change_password BOOLEAN NOT NULL DEFAULT true,
    failed_logins        INT NOT NULL DEFAULT 0,
    locked_until         TIMESTAMPTZ,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX users_email_key ON users (lower(email));
CREATE TRIGGER users_touch BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

CREATE TYPE session_kind AS ENUM ('web', 'mobile');

CREATE TABLE sessions (
    id           BIGSERIAL PRIMARY KEY,
    user_id      BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- sha256 of the opaque bearer/cookie token. The token itself is never stored,
    -- so a database leak does not hand out live sessions.
    token_hash   BYTEA NOT NULL UNIQUE,
    kind         session_kind NOT NULL,
    device_label TEXT,
    user_agent   TEXT,
    ip           TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX sessions_user_idx ON sessions (user_id) WHERE revoked_at IS NULL;

CREATE TABLE audit_log (
    id        BIGSERIAL PRIMARY KEY,
    at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    user_id   BIGINT REFERENCES users(id),     -- null for system/background actions
    entity    TEXT NOT NULL,                   -- 'order', 'partner', 'blocker', ...
    entity_id BIGINT NOT NULL,
    action    TEXT NOT NULL,                   -- 'create', 'update', 'stage_change', ...
    changes   JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX audit_log_entity_idx ON audit_log (entity, entity_id, at DESC);

-- Single-row settings table. Typed columns rather than key/value so a typo is a
-- compile error in the repo layer, not a silently ignored setting.
CREATE TABLE settings (
    id                               BOOLEAN PRIMARY KEY DEFAULT true CHECK (id),
    -- Global kill switch for automatic email. Off until someone deliberately turns it on.
    automatic_email_enabled          BOOLEAN NOT NULL DEFAULT false,
    max_auto_emails_per_recipient_day INT NOT NULL DEFAULT 3 CHECK (max_auto_emails_per_recipient_day >= 0),
    send_window_start                TIME NOT NULL DEFAULT '08:00',
    send_window_end                  TIME NOT NULL DEFAULT '17:00',
    send_window_weekdays_only        BOOLEAN NOT NULL DEFAULT true,
    nudge_interval_days              INT NOT NULL DEFAULT 3 CHECK (nudge_interval_days >= 1),
    nudge_escalate_after             INT NOT NULL DEFAULT 2 CHECK (nudge_escalate_after >= 1),
    stage_change_notifications       BOOLEAN NOT NULL DEFAULT false,
    stalled_alert_recipients         TEXT[] NOT NULL DEFAULT '{}',
    updated_at                       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by                       BIGINT REFERENCES users(id)
);
INSERT INTO settings DEFAULT VALUES;
CREATE TRIGGER settings_touch BEFORE UPDATE ON settings FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- Postgres-backed job queue. Claimed with FOR UPDATE SKIP LOCKED; a lease on locked_at
-- lets a crashed worker's jobs be reclaimed.
CREATE TABLE jobs (
    id           BIGSERIAL PRIMARY KEY,
    kind         TEXT NOT NULL,
    payload      JSONB NOT NULL DEFAULT '{}'::jsonb,
    -- Optional: at most one unfinished job per key (e.g. 'fetch_fx_rates:2026-09-10').
    dedupe_key   TEXT,
    run_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts     INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    locked_at    TIMESTAMPTZ,
    locked_by    TEXT,
    completed_at TIMESTAMPTZ,
    failed_at    TIMESTAMPTZ,                  -- dead-lettered after max_attempts
    last_error   TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX jobs_runnable_idx ON jobs (run_at) WHERE completed_at IS NULL AND failed_at IS NULL;
CREATE UNIQUE INDEX jobs_dedupe_idx ON jobs (dedupe_key)
    WHERE dedupe_key IS NOT NULL AND completed_at IS NULL AND failed_at IS NULL;
CREATE INDEX jobs_failed_idx ON jobs (failed_at DESC) WHERE failed_at IS NOT NULL;
