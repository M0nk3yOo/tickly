# Tickly Companion

Tauri-2-App (Rust + React). Sitzt im Tray bzw. in der Menüleiste und meldet dem Server Rechnerstart, Bildschirmsperre, aktive App und Beenden.

## Aufbau

- `src-tauri/` – Rust: Tray, Fenster, Token-Ablage, Tracking-Schleife (alle 2 s). Die eigentliche Logik steckt in den Crates [`tickly-agent`](../../crates/agent) (Ereignisse, Outbox, Senden) und [`tickly-platform`](../../crates/platform) (Systemzugriff).
- `src/` – React: Kopplung, Status, Rückfrage „Arbeitest du gerade?“, Pausieren/Fortsetzen, Einstellungen dieses Rechners.

## Entwickeln und bauen

```sh
npm install
npm run dev                 # nur die Oberfläche im Browser (mit Beispieldaten): http://localhost:1420/?track=asking
npx tauri dev               # komplette App
npx tauri build             # Installer: .app/.dmg (macOS), .msi/.exe (Windows)
```

Unbeaufsichtigte Einrichtung: `TICKLY_PAIR="zeit.example.com|ABCD-EFGH"` koppelt beim ersten Start ohne Eingabe.

Installer für beide Systeme entstehen beim Release (`.github/workflows/release.yml`, Tag `v…`) oder manuell über `.github/workflows/companion.yml`. Anleitung für Benutzer: [docs/COMPANION.md](../../docs/COMPANION.md).

## Verhalten

- **Fenster schließen (✕)** versteckt nur das Fenster. **Beenden** (Tray, Cmd+Q) beendet das Tracking und setzt das Arbeitsende.
- **Herunterfahren/Neustart** meldet der Companion als `shutdown`: Das Tracking wird nach dem nächsten Start fortgesetzt.
- **Offline:** Ereignisse liegen in `outbox.json` (Datenordner der App) und werden mit steigenden Abständen erneut gesendet. Die Nummern (`seq`) steigen immer weiter; der Server ignoriert Wiederholungen.
- **Sperre** wird auf macOS über die Sitzungsdaten (`CGSSessionScreenIsLocked`), auf Windows über den Secure Desktop erkannt. Ein Ruhezustand zählt wie eine Sperre.
- **Aktive App:** nur der Name, nie Fenstertitel. Apps aus der Liste „Ausgeschlossene Apps“ gehen als „Sonstiges“ raus.
- **Autostart** (mit dem Rechner starten) ist eine Einstellung dieses Rechners im Companion-Fenster; nach der Kopplung ist er an. Der automatische Tracking-Start ist je Gerät auf dem Server gespeichert (`POST /api/device/prefs`).
- Datenschutz: Das Token liegt unter macOS in einer Datei, die nur der Benutzer lesen kann (der Schlüsselbund wird bewusst nicht benutzt), unter Windows in der Anmeldeinformationsverwaltung.

## Rechte

macOS braucht keine Zusatzrechte: Der Name der aktiven App und der Sperrzustand sind ohne „Bedienungshilfen“ oder „Bildschirmaufnahme“ lesbar.

## Nicht getestet

Der Windows-Teil (`crates/platform/src/win.rs`) wurde nur übersetzt, nicht auf Windows ausgeführt. Bitte vor dem Einsatz auf einem echten Rechner prüfen: aktive App, Sperre/Entsperren, Herunterfahren mit laufendem Tracking.
