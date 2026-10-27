-- Gespeicherte Sicherungen (automatisch täglich und vor jeder Wiederherstellung), gzip-komprimiertes JSON
CREATE TABLE backups (
    id         bigserial PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now(),
    kind       text        NOT NULL CHECK (kind IN ('auto', 'pre-restore')),
    users      integer     NOT NULL,
    data       bytea       NOT NULL
);
