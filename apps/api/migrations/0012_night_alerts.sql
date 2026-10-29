-- Hinweise zum laufenden Tracking: „sehr lange“ (long) und „über Nacht“ (night), je Benutzer, Tag und Art höchstens einmal
ALTER TABLE track_alerts ADD COLUMN kind text NOT NULL DEFAULT 'long';
ALTER TABLE track_alerts DROP CONSTRAINT track_alerts_pkey;
ALTER TABLE track_alerts ADD PRIMARY KEY (user_id, day, kind);
