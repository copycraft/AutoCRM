-- Per-user preferences. Independent of the global admin settings:
-- these follow the user across devices and never change system behaviour.
-- Absent row means defaults (comfortable density, 50 rows per page).
CREATE TABLE user_settings (
    user_id             BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    density             TEXT NOT NULL DEFAULT 'comfortable' CHECK (density IN ('comfortable', 'compact')),
    page_size           INT NOT NULL DEFAULT 50 CHECK (page_size IN (25, 50, 100)),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TRIGGER user_settings_touch BEFORE UPDATE ON user_settings FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
