-- Persönliche API-Schlüssel (Anmeldung per `Authorization: Bearer tkl_…` für /api/v1/*).
-- Gespeichert wird nur der SHA-256-Hash; das Präfix dient der Wiedererkennung in Listen.
CREATE TABLE api_keys (
    id           bigserial   PRIMARY KEY,
    user_id      text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         text        NOT NULL,
    prefix       text        NOT NULL,
    token_hash   bytea       NOT NULL UNIQUE,
    -- false = nur lesen (GET), true = lesen und schreiben
    can_write    boolean     NOT NULL DEFAULT false,
    created_at   timestamptz NOT NULL DEFAULT now(),
    -- NULL = unbegrenzt
    expires_at   timestamptz,
    last_used_at timestamptz,
    revoked_at   timestamptz,
    -- Benutzer, der den Schlüssel widerrufen hat (Besitzer oder Administrator)
    revoked_by   text
);
CREATE INDEX api_keys_user ON api_keys (user_id);
