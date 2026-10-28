-- Passwort zurücksetzen per Sicherheitsfragen und Initialpasswort durch Administratoren
-- Die Antworten stehen nur als Argon2-Hash da (normalisiert: Kleinbuchstaben, ohne Leer- und Sonderzeichen).
CREATE TABLE security_answers (
    user_id     text     NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    slot        smallint NOT NULL CHECK (slot BETWEEN 1 AND 3),
    question    text     NOT NULL,
    answer_hash text     NOT NULL,
    PRIMARY KEY (user_id, slot)
);

-- Ein Administrator hat ein Initialpasswort gesetzt: Beim nächsten Anmelden mit Passwort muss der Benutzer es ändern.
ALTER TABLE users ADD COLUMN must_change_pw boolean NOT NULL DEFAULT false;
-- Diese Sitzung stammt aus einer Passwort-Anmeldung und ist gesperrt, bis das Passwort geändert ist.
ALTER TABLE sessions ADD COLUMN pw_change boolean NOT NULL DEFAULT false;
