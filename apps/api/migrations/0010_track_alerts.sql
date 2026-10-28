-- Hinweis-Mail „Tracking läuft schon sehr lange“: höchstens einmal je Benutzer und Tag
CREATE TABLE track_alerts (
    user_id text        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    day     date        NOT NULL,
    sent_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, day)
);
