-- Neue E-Mail-Adresse, die erst nach Bestätigung per Link an diese Adresse übernommen wird.
ALTER TABLE users ADD COLUMN pending_email text;

-- Einmal-Links gibt es jetzt auch für die Bestätigung einer neuen Adresse
ALTER TABLE email_tokens DROP CONSTRAINT email_tokens_kind_check;
ALTER TABLE email_tokens ADD CONSTRAINT email_tokens_kind_check CHECK (kind IN ('verify', 'reset', 'email'));
