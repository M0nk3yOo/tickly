-- Je Gerät einstellbar: Soll das Tracking auf diesem Rechner von selbst starten (Zeitfenster)?
ALTER TABLE devices ADD COLUMN auto_track boolean NOT NULL DEFAULT true;
