-- Hat der Benutzer seinen Überstunden-Stand zu Beginn schon festgelegt? Einmalig selbst, danach nur ein Administrator.
-- Bestehende Konten mit einem Stand ungleich 0 und Demo-Konten gelten als festgelegt; bei 0 wird einmal nachgefragt.
ALTER TABLE users ADD COLUMN start_bal_set boolean NOT NULL DEFAULT false;
UPDATE users SET start_bal_set = true WHERE demo OR coalesce((settings->>'startBal')::float8, 0) <> 0;
