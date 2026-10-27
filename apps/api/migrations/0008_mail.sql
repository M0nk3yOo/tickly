-- E-Mail: Bestätigung der Adresse und Passwort-Zurücksetzen per Einmal-Link
ALTER TABLE users ADD COLUMN email_verified_at timestamptz;

CREATE TABLE email_tokens (
    token_hash text PRIMARY KEY,
    user_id    text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind       text        NOT NULL CHECK (kind IN ('verify', 'reset')),
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX email_tokens_user ON email_tokens (user_id, kind, created_at DESC);
