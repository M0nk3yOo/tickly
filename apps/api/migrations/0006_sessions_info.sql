-- Sitzungen für Administratoren sichtbar und beendbar machen (Adresse, Land, Browser, letzte Aktivität)
ALTER TABLE sessions
    ADD COLUMN public_id    uuid        NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN ip           text,
    ADD COLUMN country      text,
    ADD COLUMN user_agent   text        NOT NULL DEFAULT '',
    ADD COLUMN last_seen_at timestamptz NOT NULL DEFAULT now();
CREATE UNIQUE INDEX sessions_public_id ON sessions (public_id);
ALTER TABLE audit_log ADD COLUMN country text;
