-- „Jetzt sichern“ legt manuelle Sicherungen an
ALTER TABLE backups DROP CONSTRAINT backups_kind_check;
ALTER TABLE backups ADD CONSTRAINT backups_kind_check CHECK (kind IN ('auto', 'manual', 'pre-restore'));
