-- Wann ein Gerät zuletzt Ereignisse gemeldet hat (anders als `last_seen_at` zählen Zustandsabfragen nicht).
ALTER TABLE devices ADD COLUMN last_event_at timestamptz;

-- Zweiter Companion hat das Tracking gestartet, während ein anderer lief: Warnung bis zur Bestätigung oder bis einer endet.
CREATE TABLE device_clash (
    user_id text PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    day     date        NOT NULL,
    first   uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    second  uuid        NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    at      timestamptz NOT NULL DEFAULT now(),
    ack     boolean     NOT NULL DEFAULT false
);
