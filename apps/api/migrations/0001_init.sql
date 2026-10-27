-- Tickly: Grundschema

CREATE TABLE users (
    id            text PRIMARY KEY,
    name          text        NOT NULL,
    email         text        NOT NULL,
    role          text        NOT NULL CHECK (role IN ('admin', 'user')),
    status        text        NOT NULL CHECK (status IN ('active', 'pending', 'blocked', 'invited')),
    password_hash text,
    webauthn_id   uuid        NOT NULL,
    settings      jsonb       NOT NULL,
    -- Demo-Benutzer bekommen eine erzeugte Historie (siehe tickly-core::demo)
    demo          boolean     NOT NULL DEFAULT false,
    seed          text        NOT NULL DEFAULT '',
    demo_base     double precision NOT NULL DEFAULT 541,
    -- Beginn der Überstundenrechnung
    track_start   date        NOT NULL,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_seen_at  timestamptz
);
CREATE UNIQUE INDEX users_email_lower ON users (lower(email));

CREATE TABLE credentials (
    id           bigserial PRIMARY KEY,
    user_id      text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         text        NOT NULL,
    passkey      jsonb       NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz
);
CREATE INDEX credentials_user ON credentials (user_id);

CREATE TABLE absences (
    id        bigserial PRIMARY KEY,
    user_id   text NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    kind      text NOT NULL CHECK (kind IN ('vacation', 'sick', 'comp')),
    date_from date NOT NULL,
    date_to   date NOT NULL CHECK (date_to >= date_from),
    note      text NOT NULL DEFAULT ''
);
CREATE INDEX absences_user ON absences (user_id, date_from);

-- Ein Arbeitstag je Benutzer und Datum (Abschnitte, Ereignisse, Status)
CREATE TABLE days (
    user_id    text NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day        date NOT NULL,
    state      jsonb NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, day)
);

CREATE TABLE sessions (
    id         text PRIMARY KEY,
    user_id    text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- eingeschränkt: nur Passkey-Einrichtung (Konto wartet auf Freigabe)
    limited    boolean     NOT NULL DEFAULT false,
    expires_at timestamptz NOT NULL
);
CREATE INDEX sessions_user ON sessions (user_id);

CREATE TABLE app_settings (
    key   text PRIMARY KEY,
    value jsonb NOT NULL
);

-- Gekoppelte Companion-Geräte. Das Token wird nur als Hash gespeichert.
CREATE TABLE devices (
    id           uuid PRIMARY KEY,
    user_id      text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name         text        NOT NULL,
    os           text        NOT NULL,
    version      text        NOT NULL DEFAULT '',
    token_hash   bytea       NOT NULL UNIQUE,
    last_seq     bigint      NOT NULL DEFAULT 0,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz,
    revoked_at   timestamptz
);
CREATE INDEX devices_user ON devices (user_id);

CREATE TABLE pair_codes (
    code       text PRIMARY KEY,
    user_id    text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
