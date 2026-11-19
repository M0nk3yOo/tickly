# Architektur und API

## Überblick

```
Companion (Windows/macOS)  ──HTTPS + Bearer-Token──▶  Proxy (Zoraxy/Cloudflare …) ──HTTP + X-Origin-Secret──▶  api (Axum) ──▶ PostgreSQL
Browser (Weboberfläche)    ──HTTPS + Cookie────────▶                                                                   │
                                                                                                                       └─ liefert auch web/ aus
```

**Grundgedanke:** Die Rechenlogik steckt in **einer** Rust-Bibliothek (`tickly-core`) ohne Ein-/Ausgabe und ohne eigene Uhr (Zeit kommt als Parameter). Server und Companion benutzen genau dieselbe Funktion `apply_event` für den Tageszustand. Deshalb kann der Companion **offline mitrechnen** und kommt später beim Server zum gleichen Ergebnis. Der Server bleibt maßgeblich.

## Bausteine

| Baustein | Aufgabe |
|---|---|
| `crates/core` | Soll/Ist, Brutto/Netto, Pausenregeln, Feiertage je Bundesland (inkl. beweglicher), Abwesenheiten, Salden, **Tageszustand** (`DayState`: Segmente, Ereignisse, Start/Stopp/Pause/Sperre/Rückfrage/Zeitfenster) |
| `apps/api` | Axum-Server, SQLx/PostgreSQL, WebAuthn, TOTP, Sitzungen, Gerätekopplung, Protokoll/IP-Sperren, Sicherung, SMTP, Verschlüsselung (`secrets`) |
| `crates/platform` | `Platform`-Trait; macOS (`NSWorkspace`, `CGSession…`, Power-Off-Notification) und Windows (`GetForegroundWindow`, `OpenInputDesktop`, `WM_ENDSESSION`) |
| `crates/agent` | `Engine`: beobachtet das System, führt den Tag lokal mit, schreibt Ereignisse in eine dauerhafte **Outbox** und sendet sie (`/api/device/events`) |
| `apps/companion` | Tauri 2: Tray, Fenster (React), Token-Ablage, Engine-Thread alle 2 s |
| `web/` | Weboberfläche ohne Build-Schritt. Rechnet nichts selbst: alle Zahlen kommen vom Server (`/api/snapshot`) |

## Datenmodell (PostgreSQL)

Migrationen in `apps/api/migrations`, werden beim Start eingespielt.

| Tabelle | Inhalt |
|---|---|
| `users` | Konto, Rolle, Status, Einstellungen (`jsonb`), TOTP, Beginn der Erfassung |
| `credentials` | Passkeys |
| `recovery_codes` | Einmalcodes (Hash) |
| `absences` | Urlaub, Krankheit, Zeitausgleich |
| `days` | je Benutzer und Datum der Tageszustand (`jsonb`) |
| `api_keys` | persönliche API-Schlüssel (nur SHA-256-Hash und Präfix, Berechtigung, Ablauf, zuletzt benutzt, widerrufen) |
| `devices`, `pair_codes` | gekoppelte Companion-Geräte (Token-Hash, Version, `last_seq`), Kopplungscodes |
| `sessions` | Sitzungen mit IP, Land, Browser, letzter Aktivität |
| `audit_log`, `ip_blocks` | Protokoll, IP-Sperren |
| `email_tokens` | Bestätigungs- und Reset-Links (Hash) |
| `backups` | gespeicherte Sicherungen (optional verschlüsselt) |
| `app_settings` | Richtlinien (`reg`), SMTP, Sicherung, Sperren, Prüfwert des Schlüssels |

Ein Tag ist **ein** Datensatz: `segs` (Abschnitte `app`/`locked`/`off` in Minuten seit Mitternacht), `events`, `stop`-Zustand, `last_beat`. Abschnitte vergangener Tage ohne Eintrag stammen im Demo-Modus aus einem Generator.

## Das Companion-Protokoll

1. **Koppeln:** `POST /api/device/pair {code, name, os, version}` → Token.
2. **Ereignisse:** `POST /api/device/events {events:[{seq, kind, at, app?, os?, answer?}]}` mit `Authorization: Bearer`. `seq` steigt je Gerät immer weiter; der Server ignoriert bereits gesehene Nummern (**idempotent**). Die Antwort enthält den Zustand (`state`: Einstellungen, Tag, Salden, Zeitzone) – daraus übernimmt der Companion den Stand, wenn alles übertragen ist.
3. Ereignisarten: `boot`, `beat` (alle 30 s), `lock`, `unlock`, `app`, `pause`, `start`, `end` (Tracking beenden, App läuft weiter), `answer`, `away` (Antwort `pause` oder `work` auf die Rückfrage nach einer längeren Sperre, `day.away`), `quit`, `shutdown`.
4. Zeitstempel (`at`) sind die echten Zeitpunkte; der Server rechnet sie in Datum und Minuten der Zeitzone `TICKLY_TZ` um. Bis zu einer Stunde vorgehende Uhren werden auf die Serverzeit begrenzt, ältere als 14 Tage abgelehnt.
5. Die Version steht im `User-Agent` (`tickly-companion/x.y.z`).

## API (Auszug)

Alle Pfade unter `/api`. Antworten sind JSON; Fehler `{ "error": "…" }`. Anmeldung per Cookie `tickly_sid` (Weboberfläche) bzw. Bearer-Token (Geräte).

| Bereich | Pfade |
|---|---|
| Öffentlich | `GET /health`, `GET /config` |
| Anmeldung | `POST /auth/login`, `/auth/totp`, `/auth/logout`, `/auth/register`, `/auth/verify`, `/auth/resend-verification`, `/auth/forgot`, `/auth/reset`, `GET /session`, `POST /me/password` |
| Passkeys | `POST /passkey/register/{start,finish}`, `/passkey/login/{start,finish}`, `PUT/DELETE /passkeys/{id}` |
| TOTP | `POST /totp/{setup,enable,disable,recovery}` |
| Daten | `GET /snapshot`, `GET /today`, `PUT /settings`, `POST /me`, `POST /absences`, `PUT/DELETE /absences/{id}`, `PUT /days/{date}`, `POST /companion` |
| Sitzungen | `GET /sessions`, `DELETE /sessions/{id}` |
| Geräte | `POST /devices/pair-code`, `DELETE /devices/{id}`, `PATCH /devices/{id}` (Auto-Start je Gerät), `POST /device/pair`, `POST /device/events`, `GET /device/state`, `POST /device/prefs` (das Gerät stellt seinen Auto-Start selbst ein) |
| API-Schlüssel | `GET/POST /apikeys`, `DELETE /apikeys/{id}` (Cookie); `GET /admin/apikeys`, `DELETE /admin/apikeys/{id}`, `PUT /admin/apikey-policy`; die Schnittstelle selbst unter `/v1/*` mit `Authorization: Bearer tkl_…`, siehe [API.md](API.md) |
| Administration | `/admin/users…`, `/admin/registration`, `/admin/security`, `/admin/ip-blocks`, `/admin/log`, `/admin/sessions`, `/admin/devices/{id}`, `/admin/smtp`, `/admin/backup`, `/admin/backups`, `/admin/backup-policy`, `/admin/restore` |

## Schnittstelle mit API-Schlüsseln

`/api/v1/*` (Modul `apikeys.rs`) ist die einzige Schnittstelle für eigene Programme; Beschreibung mit Beispielen in **[API.md](API.md)**. Sie hat eine **eigene Anmeldung** per Bearer-Schlüssel `tkl_…` (Middleware `v1_auth`), unabhängig von Cookie-Sitzung und Geräte-Token: `auth()` liest nur das Cookie, `auth_device()` nur Geräte-Token, deshalb öffnet ein API-Schlüssel nie Administration, Passwörter, Passkeys, Geräte oder Sitzungen. Die Handler benutzen dieselben Bausteine wie die Weboberfläche (`Calc`/`day_stat`/`total_balance` aus `tickly-core`, `events::apply` für Tracking, `edit::save_day` für Zeiten); nichts wird doppelt gerechnet. Fehlversuche mit falschem Schlüssel zählen als Art `apikey` für die IP-Sperre. Richtlinie (an/aus, unbegrenzt, längste Laufzeit) in `app_settings` unter `apikeys`; die Tabelle `api_keys` ist Teil der Sicherung.

## Zeit und Datum

Im **Echtbetrieb** gilt die Serveruhr in der Zeitzone `TICKLY_TZ`. Im **Demo-Modus** (`TICKLY_DEMO=1`) gilt ein festes Datum (26.10.2026), die Uhrzeit liefert der Browser (Zeitraffer), und für Benutzer ohne eigene Daten erzeugt ein deterministischer Generator eine Historie. Die Demo ist für Vorführung und Tests gedacht, nicht für echte Daten.

## Design-Entscheidungen

- **Server rechnet, Clients zeigen.** Keine Doppelrechnung in JavaScript – Zahlen können nicht auseinanderlaufen.
- **Ereignisse statt Zustände senden.** Offline sammeln, später in Reihenfolge abspielen, idempotent.
- **Keine automatische Löschfrist für Arbeitszeiten** (Vorgabe des Auftraggebers); das Protokoll wird nach `LOG_RETENTION_DAYS` bereinigt.
- **Ein Binary, eine Datenbank.** Der Server liefert die Weboberfläche mit aus; PostgreSQL ist die einzige Abhängigkeit.
