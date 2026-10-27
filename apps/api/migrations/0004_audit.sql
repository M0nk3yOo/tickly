-- Protokoll für Administratoren und automatische IP-Sperren
CREATE TABLE audit_log (
    id         bigserial PRIMARY KEY,
    at         timestamptz NOT NULL DEFAULT now(),
    kind       text    NOT NULL,
    ok         boolean NOT NULL,
    -- Originaladresse des Clients (auch hinter Proxy) und der Sperrschlüssel (IPv6: /64-Netz)
    ip         text    NOT NULL,
    ip_key     text    NOT NULL,
    user_agent text    NOT NULL DEFAULT '',
    -- kein Fremdschlüssel: Einträge bleiben, auch wenn das Konto gelöscht wird
    user_id    text,
    email      text,
    detail     text    NOT NULL DEFAULT ''
);
CREATE INDEX audit_log_at ON audit_log (at DESC);
CREATE INDEX audit_log_ip ON audit_log (ip_key, at DESC);

CREATE TABLE ip_blocks (
    id            bigserial PRIMARY KEY,
    ip_key        text        NOT NULL,
    blocked_at    timestamptz NOT NULL DEFAULT now(),
    -- leer = bis ein Administrator entsperrt
    expires_at    timestamptz,
    reason        text        NOT NULL,
    manual        boolean     NOT NULL DEFAULT false,
    unblocked_at  timestamptz,
    unblocked_by  text
);
CREATE INDEX ip_blocks_key ON ip_blocks (ip_key, blocked_at DESC);
