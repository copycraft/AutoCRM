-- Onboarding by invitation (0066).
--
-- An admin no longer has to make up a temporary password and pass it on: they enter the
-- colleague's email, name and role, and the system sends a letter with a one-time link.
-- On that page the colleague picks how the app looks and sets their own password, and is
-- signed in. The same letter can be sent again later (a forgotten password): a newer
-- letter replaces the older ones.
CREATE TABLE user_invites (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- SHA-256 of the token in the link. The token itself exists only in the letter.
    token_hash  BYTEA NOT NULL UNIQUE,
    created_by  BIGINT REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at  TIMESTAMPTZ NOT NULL,
    -- The colleague set their password with it.
    used_at     TIMESTAMPTZ,
    -- A newer letter was sent, or the password was set some other way.
    revoked_at  TIMESTAMPTZ,
    CHECK (expires_at > created_at)
);
CREATE INDEX user_invites_user_idx ON user_invites (user_id) WHERE used_at IS NULL AND revoked_at IS NULL;

-- How the app looks for this person: follow the device, or always light / always dark.
-- Chosen on the invitation page, changed later under Beállításaim.
ALTER TABLE user_settings
    ADD COLUMN theme TEXT NOT NULL DEFAULT 'system' CHECK (theme IN ('system', 'light', 'dark'));

INSERT INTO email_templates (key, name, subject, body, is_automatic, category, folder) VALUES
('user_invite', 'Meghívó új munkatársnak (fiók beállítása)',
 'Meghívó az AutoCRM-be',
 'Kedves {{invite.name}}!

{{invite.by}} meghívta Önt az AutoCRM-be. A fiókja ehhez az e-mail-címhez tartozik: {{invite.email}}

Az alábbi linken kiválaszthatja, hogyan nézzen ki a program (világos vagy sötét), és beállíthatja a saját jelszavát. Utána rögtön be is lép.

{{invite.url}}

A link {{invite.expires}}-ig érvényes, és csak egyszer használható. Ha lejárt, kérjen újat a rendszergazdától. Ha nem számított erre a levélre, nyugodtan hagyja figyelmen kívül.

AutoCRM', true, 'general', 'workflow');
