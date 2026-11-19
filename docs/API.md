# Schnittstelle für eigene Programme (API)

Mit einem **persönlichen API-Schlüssel** greifen eigene Skripte, Auswertungen oder Automatisierungen auf deine Zeiten zu, ohne dein Passwort. Die Schnittstelle liegt unter `/api/v1`. Alle Antworten sind JSON, Fehler haben die Form `{ "error": "Text auf Deutsch" }`. (Die übrigen Pfade unter `/api` gehören zur Weboberfläche und zum Companion und sind nicht Teil dieser Schnittstelle; siehe [Architektur](ARCHITEKTUR.md).)

## Schlüssel anlegen

Einstellungen → **API-Schlüssel** → „Neuer Schlüssel“: Bezeichnung, Laufzeit (30 Tage, 90 Tage, 180 Tage, 1 Jahr oder unbegrenzt) und Berechtigung wählen. Der Schlüssel (`tkl_…`, 68 Zeichen) wird **genau einmal** angezeigt. Danach steht nur ein Kürzel (`tkl_ab12cd34…`) in der Liste. Du kannst höchstens 20 aktive Schlüssel haben und jeden einzeln widerrufen. Ob Schlüssel erlaubt sind, ob sie unbegrenzt gelten dürfen und wie lange höchstens, bestimmt der Administrator (siehe [Administration](ADMINISTRATION.md)).

## Anmeldung

Jede Anfrage trägt den Schlüssel im Header `Authorization`:

```sh
export TICKLY=https://zeit.example.de
export KEY=tkl_…
curl -H "Authorization: Bearer $KEY" $TICKLY/api/v1/me
```

Steht Tickly hinter einem Proxy mit `ORIGIN_SECRET`, muss der Proxy den Header `X-Origin-Secret` setzen wie bei jeder anderen Anfrage; der Schlüssel ersetzt ihn nicht.

## Berechtigungen und Grenzen

- **Nur lesen:** erlaubt nur `GET`. Jede andere Methode antwortet mit `403`.
- **Lesen und Schreiben:** zusätzlich Tracking steuern, Abwesenheiten ändern, Zeiten eintragen.
- Ein Schlüssel gilt **nur für `/api/v1/*`**. Alles andere (Passwörter, Passkeys, Authenticator-App, Schlüsselverwaltung, Administration, Geräte-Kopplung, Sitzungen, Einstellungen) lehnt ihn mit `401` ab, auch für Administratoren.
- Es gelten dieselben Regeln wie in der Weboberfläche: „Beginn ändern“, „Ende vorab festlegen“ und die Rückwirkungs-Grenze für Zeiten bestimmt der Administrator.
- Wird der Benutzer gesperrt, antwortet die Schnittstelle mit `403`; wird er gelöscht, verschwinden seine Schlüssel. Eine Passwortänderung widerruft Schlüssel **nicht** (siehe [Sicherheit](SICHERHEIT.md)).

## Fehlercodes

| Code | Bedeutung |
|---|---|
| `400` | Ungültige Eingabe (Datum, Uhrzeit, Zeitraum über 366 Tage …) |
| `401` | Schlüssel fehlt, unbekannt, **abgelaufen** oder **widerrufen**; Pfad gehört nicht zur Schnittstelle |
| `403` | „Nur lesen“-Schlüssel beim Schreiben, Konto gesperrt, API-Schlüssel vom Administrator ausgeschaltet, Funktion vom Administrator gesperrt (z. B. „Ende vorab festlegen“), IP-Adresse gesperrt |
| `404` | Eintrag nicht gefunden |
| `409` | Passt nicht zum Zustand (z. B. „Tracking läuft bereits“, „Es läuft kein Tracking“) |

Wiederholt **falsche** Schlüssel zählen wie falsche Anmeldungen: nach zu vielen Fehlversuchen (Standard 5 in 10 Minuten) wird die IP-Adresse automatisch gesperrt, bis sie ein Administrator freigibt oder die Sperre abläuft. Abgelaufene oder widerrufene Schlüssel zählen dafür nicht, stehen aber (höchstens alle zehn Minuten je Schlüssel) im Protokoll.

## Zahlen

Dauern heißen `…Min` und sind **Minuten** (Dezimalzahl), Uhrzeiten sind `"HH:MM"` in der Zeitzone des Servers (`/me` nennt sie), Daten `JJJJ-MM-TT`. Gerechnet wird ausschließlich vom Server mit denselben Regeln wie in der Weboberfläche. `countedMin` ist die gezählte Arbeitszeit je nach Modus (Netto oder Brutto), `diffMin` der Beitrag zum Überstundenkonto.

## Lesen

### `GET /api/v1/me`

```sh
curl -H "Authorization: Bearer $KEY" $TICKLY/api/v1/me
```
```json
{ "id": "…", "name": "Lena Hoffmann", "email": "lena@example.de", "role": "user", "timezone": "Europe/Berlin", "version": "0.9.18", "permission": "read" }
```

### `GET /api/v1/status`

Heute: Zustand und Zahlen.

```sh
curl -H "Authorization: Bearer $KEY" $TICKLY/api/v1/status
```
```json
{ "date": "2026-11-19", "state": "running", "tracking": true, "start": "08:12", "end": null, "plannedEnd": null,
  "grossMin": 187.5, "breakMin": 0, "netMin": 187.5, "countedMin": 187.5, "sollMin": 480, "diffMin": -292.5, "liveDiffMin": -292.5,
  "balanceMin": 125, "activeApp": "VS Code", "nowMin": 680.5, "holiday": null, "absence": null, "kind": null,
  "limits": { "plannedEnd": true, "startChange": true, "editDaysBack": 90 } }
```

`state`: `none` (heute noch nichts), `running`, `locked` (Bildschirm gesperrt), `paused`, `asking` (Rückfrage offen), `idle`, `stopped`.

### `GET /api/v1/days?from=JJJJ-MM-TT&to=JJJJ-MM-TT`

Tageswerte. Ohne Angaben: erster des Monats bis heute. Höchstens 366 Tage.

```sh
curl -H "Authorization: Bearer $KEY" "$TICKLY/api/v1/days?from=2026-11-01&to=2026-11-07"
```
```json
{ "from": "2026-11-01", "to": "2026-11-07", "days": [
  { "date": "2026-11-02", "worked": true, "future": false, "start": "08:05", "end": "17:01", "grossMin": 536, "breakMin": 45, "netMin": 491, "countedMin": 491,
    "sollMin": 480, "diffMin": 11, "holiday": null, "absence": null, "kind": null } ] }
```

`absence` ist `vacation`, `sick` oder `comp`, `kind` zusätzlich `holiday`; `holiday` nennt den Namen des Feiertags.

### `GET /api/v1/days/{datum}`

Wie ein Eintrag aus `/days`, zusätzlich `segments` (Abschnitte des Tages mit `kind` = `app`, `locked`, `from`, `to`, `durationMin`, `app`, `note`).

```sh
curl -H "Authorization: Bearer $KEY" $TICKLY/api/v1/days/2026-11-02
```

### `GET /api/v1/balance`

Überstundenkonto: `balanceMin` (Stand inklusive Übertrag `startBalanceMin`), `trackStart`, sowie `week` und `month` mit `countedMin`, `sollMin`, `diffMin`.

```sh
curl -H "Authorization: Bearer $KEY" $TICKLY/api/v1/balance
```

### `GET /api/v1/absences`

Urlaub, Krankheit, Zeitausgleich: `{ "absences": [ { "id": 7, "type": "vacation", "from": "…", "to": "…", "note": "", "workdays": 5 } ] }`.

### `GET /api/v1/export.csv?from=&to=`

Tageswerte als CSV (UTF-8 mit BOM, Trennzeichen `;`, Dauern als `H:MM`), gleiches Format wie der Export in den Einstellungen plus Spalte „Hinweis“. Zeitraum wie bei `/days`.

```sh
curl -H "Authorization: Bearer $KEY" "$TICKLY/api/v1/export.csv?from=2026-01-01&to=2026-12-31" -o tickly.csv
```

## Schreiben (Schlüssel „Lesen und Schreiben“)

### Abwesenheiten

`POST /api/v1/absences` (Antwort `201` mit `id`), `PUT /api/v1/absences/{id}`, `DELETE /api/v1/absences/{id}`. Körper: `type` (`vacation`, `sick`, `comp`), `from`, `to`, `note`.

```sh
curl -X POST -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"type":"vacation","from":"2026-12-21","to":"2026-12-23","note":"Brückentage"}' $TICKLY/api/v1/absences
```

### Tracking steuern

Gleiche Regeln wie die Weboberfläche ohne Companion. Die Antwort enthält den neuen `status` (wie `GET /status`).

```sh
curl -X POST -H "Authorization: Bearer $KEY" $TICKLY/api/v1/tracking/start     # starten oder nach einer Pause fortsetzen
curl -X POST -H "Authorization: Bearer $KEY" $TICKLY/api/v1/tracking/pause     # pausieren
curl -X POST -H "Authorization: Bearer $KEY" $TICKLY/api/v1/tracking/stop      # jetzt beenden
curl -X POST -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" -d '{"at":"16:30"}' $TICKLY/api/v1/tracking/stop
curl -X DELETE -H "Authorization: Bearer $KEY" $TICKLY/api/v1/tracking/planned-end   # vorab festgelegtes Ende aufheben
```

`stop` mit `at`: eine Uhrzeit bis jetzt beendet rückwirkend zu diesem Zeitpunkt, alles danach entfällt. Eine **spätere** Uhrzeit **legt das Ende vorab fest** (`"planned": true`); das Tracking endet dann von selbst. Das geht nur, wenn der Administrator „Ende vorab festlegen“ erlaubt (sonst `403`).

### Zeit eintragen

`POST /api/v1/entries` ergänzt einen Abschnitt manuell (wie „Zeit eintragen“ in der Tagesansicht). Vorhandene Abschnitte bleiben; Überschneidungen werden abgelehnt. Es gilt die Rückwirkungs-Grenze des Administrators (Standard 90 Tage), für die Zukunft geht nichts. Antwort `201` mit `before`/`after` (Arbeitszeit des Tages).

```sh
curl -X POST -H "Authorization: Bearer $KEY" -H "Content-Type: application/json" \
  -d '{"date":"2026-11-17","from":"13:00","to":"15:30","app":"Kundentermin","note":"Vor Ort"}' $TICKLY/api/v1/entries
```

`app` ist die Bezeichnung (Standard „Manuell“), `note` optional.

## Protokoll

Anlegen und Widerrufen von Schlüsseln, abgelehnte Schlüssel und Änderungen über `/entries` stehen im Protokoll der Serververwaltung (Art „API-Schlüssel“ bzw. „Zeiten bearbeitet“, bei Einträgen mit dem Zusatz „über API-Schlüssel“). Die Nutzung selbst wird nicht je Anfrage protokolliert, nur „zuletzt benutzt“ am Schlüssel (höchstens einmal je Minute).
