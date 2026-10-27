-- TOTP (Authenticator-App) als zweiter Faktor bei der Passwort-Anmeldung
ALTER TABLE users
    ADD COLUMN totp_secret    text,
    ADD COLUMN totp_enabled   boolean NOT NULL DEFAULT false,
    -- letzter akzeptierter 30-Sekunden-Schritt: derselbe Code gilt nur einmal
    ADD COLUMN totp_last_step bigint  NOT NULL DEFAULT 0;

-- Einmalcodes, falls das Gerät mit der Authenticator-App fehlt. Nur als Hash gespeichert.
CREATE TABLE recovery_codes (
    id        bigserial PRIMARY KEY,
    user_id   text NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash text NOT NULL,
    used_at   timestamptz
);
CREATE INDEX recovery_codes_user ON recovery_codes (user_id);
