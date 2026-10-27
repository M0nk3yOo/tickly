-- Beginn der Karenzzeit für verpflichtende Zwei-Faktor-Anmeldung (leer = Anlegedatum des Kontos)
ALTER TABLE users ADD COLUMN mfa_grace_start timestamptz;
-- Eine Sitzung merkt sich, wann sie begann (Konten ohne MFA müssen sich täglich neu anmelden)
ALTER TABLE sessions ADD COLUMN created_at timestamptz NOT NULL DEFAULT now();
