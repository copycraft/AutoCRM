-- Email transport inside the global admin settings (PUT /settings).
-- Every column is NULL-able: NULL means "inherit from the process environment",
-- which is exactly how the system behaved before these columns existed.
-- email_mode NULL inherits EMAIL_MODE; when it is set, the database owns the
-- whole transport (including redirect_to) and the environment is ignored.
-- The SMTP password lives here in the clear; PostgreSQL access is the trust
-- boundary, same as the database URL that already reaches this host. It is
-- never returned by the API (see has_password below).
ALTER TABLE settings
    ADD COLUMN email_mode        TEXT CHECK (email_mode IN ('dry_run', 'smtp')),
    ADD COLUMN smtp_host         TEXT,
    ADD COLUMN smtp_port         INT CHECK (smtp_port IS NULL OR (smtp_port BETWEEN 1 AND 65535)),
    ADD COLUMN smtp_security     TEXT CHECK (smtp_security IS NULL OR smtp_security IN ('none', 'starttls', 'tls')),
    ADD COLUMN smtp_username     TEXT,
    ADD COLUMN smtp_password     TEXT,
    ADD COLUMN smtp_helo_name    TEXT,
    ADD COLUMN smtp_force_ipv4   BOOLEAN,
    ADD COLUMN redirect_to       TEXT;
