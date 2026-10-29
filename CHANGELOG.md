# Änderungen

## 0.9.9

Aktualisierung von 0.9.8: keine Änderungen an der Konfiguration oder Datenbank nötig.

- **Startdatum der Zeiterfassung selbst festlegen:** Unter *Einstellungen → Profil → Zeiterfassung ab* kann jeder Benutzer das Datum ändern, ab dem Tickly Arbeitszeit und Überstundenkonto rechnet. Tage davor zählen nicht, es entstehen dort also keine Minusstunden, zum Beispiel nach Testtagen mit unvollständiger Aufzeichnung. Die Zeiten früherer Tage bleiben gespeichert und zählen wieder, wenn das Datum früher gesetzt wird. „Mitglied seit“ zeigt jetzt das Datum der Kontoanlage statt des Startdatums. Der Vorgang steht im Protokoll.

## 0.9.8

Aktualisierung von 0.9.7: keine Änderungen an der Konfiguration oder Datenbank nötig. **Der macOS-Companion muss einmal neu gekoppelt werden** (siehe unten).

- **macOS: kein Schlüsselbund mehr.** Der Companion legt das Geräte-Token unter macOS nicht mehr im Schlüsselbund ab (dort fragte macOS bei der unsignierten App nach), sondern in einer Datei `device.token` im Datenordner, nur für den Benutzer lesbar. **Nach dem Update ist einmal eine neue Kopplung nötig**; der alte Schlüsselbund-Eintrag `de.tickly.companion` kann in der Schlüsselbundverwaltung gelöscht werden. Unter Windows ändert sich nichts.

## 0.9.7

Aktualisierung von 0.9.6: Die Datenbank wird beim Start automatisch angepasst (Migration 0013).

- **Pause und Beenden im Web erreichen den Companion:** Der Companion holt den Zustand vom Server nun alle 4 Sekunden (statt nur mit dem Lebenszeichen alle 30 Sekunden), sobald nichts zu senden ist. Wird im Web pausiert oder beendet, zeigt ein laufender Companion das nach wenigen Sekunden ebenfalls.
- **Warnung bei zwei Companions:** Startet ein zweiter Companion das Tracking, während ein anderer desselben Benutzers noch meldet, erscheint eine Warnung in der Weboberfläche und in beiden Companions („Tracking läuft auf zwei Geräten“), bis einer endet oder sie mit **Verstanden** bestätigt wird. Es wird nichts blockiert.
- **macOS:** Der Terminalbefehl, falls sich die unsignierte App nicht öffnen lässt (`xattr -dr com.apple.quarantine "/Applications/Tickly Companion.app"`), steht jetzt gut sichtbar in `docs/COMPANION.md` und im Release-Text.

## 0.9.6

Aktualisierung von 0.9.5: Die Datenbank wird beim Start automatisch angepasst (Migration 0012).

- **Plausibilitätsprüfung nach Mitternacht:** Läuft ein Tracking über Mitternacht weiter und passt das nicht zu den bisherigen Zeiten (weniger als drei Nächte in den letzten 60 Tagen mit Arbeit bis zur jetzigen Uhrzeit), fragt Tickly ab 01:00 Uhr nach, ob es versehentlich noch läuft: Hinweis auf der Übersicht, im Companion und per Mail (einmal je Nacht). Mit **Ende festlegen …** wird das Tracking am Vortag oder nach Mitternacht beendet (mit Vorschau von Brutto, Pause und Netto), **Ich arbeite wirklich** bestätigt die Nachtarbeit. Es wird nichts automatisch beendet.
- **Companion:** Die angezeigte Zeit sprang gelegentlich vor und wieder zurück. Ursache war die Anzeige, nicht die Aufzeichnung: Der Companion rechnete mit der Uhrzeit des letzten Ticks, das Fenster zählte aber ab dem Eintreffen der Meldung weiter. Dauerte eine Übertragung zum Server länger, entstand ein Versatz in Höhe dieser Dauer. Jetzt rechnet der Companion die Anzeige zum Abfragezeitpunkt und das Fenster zählt von diesem Bezugszeitpunkt aus weiter. Aufgezeichnet wird weiterhin nur vom Companion; der Server wendet dessen Ereignisse an und rechnet nicht parallel mit.

## 0.9.5

Aktualisierung von 0.9.4: keine Änderungen an der Konfiguration oder Datenbank nötig.

- E-Mails zeigen oben das Tickly-Logo. Es ist als Bild in die Mail eingebettet (nicht als Link), erscheint also auch dort, wo Mailprogramme Fremdbilder blockieren, und unabhängig von der Erreichbarkeit des Servers.
- Die Seitenleiste zeigt unten links die laufende Serverversion („Server 0.9.4“).
- Der Menüpunkt „Benutzerverwaltung“ heißt jetzt **Serververwaltung**, denn er enthält mehr als die Benutzer (Richtlinien, Sicherheit, E-Mail, Sitzungen, Protokoll, Sicherung).

## 0.9.4

**Releases entstehen lokal:** `scripts/release.sh` baut die Installer und das Docker-Image auf dem Entwicklungsrechner (Windows nur noch als `…-setup.exe`, kein `.msi`). Die Compose-Datei wurde dabei erstmals in einem echten Docker getestet.

**macOS:** Die Companion-App wird nur noch für Apple Silicon (arm64) gebaut, Intel-Macs werden nicht mehr unterstützt (letzte Version dafür: 0.9.3). Die Datei heißt jetzt `…_aarch64.dmg`.

Aktualisierung von 0.9.3: Die Datenbank wird beim Start automatisch erweitert. Konten, bei denen der Überstunden-Stand zu Beginn 0 ist, werden nach der Anmeldung einmal danach gefragt (mit „Keine Überstunden“ bestätigt man 0).

- **Beenden zeigt Minus:** Das Fenster „Tracking beenden“ zeigt zusätzlich den Saldo des Tages (Netto − Soll) und das Überstundenkonto vorher und nachher. Endet der Tag mit Minus, weist ein Hinweis darauf hin, dass es dem Überstundenkonto abgezogen wird.
- Fehlerbehebung: Die voraussichtliche Tagesende-Zeit konnte über 24 Uhr hinausgehen („28:58“ bei Beginn um 20:27). Sie steht jetzt als „04:58 (morgen)“.
- Fehlerbehebung: Plus- und Minuswerte in Zeilen mit Beschriftung (z. B. Einstellungen, Beenden-Fenster) hatten keine Farbe.
- **Überstunden-Stand zu Beginn nachtragen:** Konten, bei denen er noch nicht festgelegt ist (erster Administrator über die Einrichtung, direkt vom Administrator angelegte Konten, bestehende Konten mit 0), sehen nach der Anmeldung ein Eingabefenster und einen Hinweis. Der Benutzer legt ihn **einmalig** selbst fest (auch „Keine Überstunden“ = 0), danach kann ihn nur noch ein Administrator ändern. Die Einrichtungsseite fragt ihn ebenfalls ab. In der Benutzerverwaltung steht „noch nicht festgelegt“. Zuvor stand in den Einstellungen irreführend „Bei der Registrierung festgelegt“.
- Sicherheitsfragen: In der Auswahl lässt sich jede Frage nur einmal wählen (in den anderen Auswahlfeldern ist sie ausgegraut). Bisher meldete erst das Absenden den Fehler.

## 0.9.3

**Wichtig beim Aktualisieren von 0.9.2:** Der Port lauscht jetzt standardmäßig auf **allen Adressen** (`0.0.0.0`), vorher nur lokal. Wer ihn weiter nur lokal erreichbar halten will, setzt `TICKLY_BIND=127.0.0.1` in der `.env`. Sonst gehört `ORIGIN_SECRET` gesetzt (und eine Firewall davor). Den Cloudflare-Tunnel (`--profile tunnel`) gibt es in der Compose-Datei nicht mehr.

- **Betrieb hinter eigenem Proxy:** Der Cloudflare-Tunnel ist aus der Compose-Datei entfernt (`CLOUDFLARE_TUNNEL_TOKEN` entfällt). Der Port lauscht jetzt auf `0.0.0.0` (einstellbar mit `TICKLY_BIND`, z. B. `127.0.0.1`); davor gehört ein Proxy mit TLS (Zoraxy, Nginx, Caddy, Cloudflare). **Wer bisher den Tunnel nutzte, muss ihn selbst betreiben oder einen anderen Proxy wählen.**
- **Origin-Prüfung (`ORIGIN_SECRET`):** Ist der Wert gesetzt, werden nur Anfragen mit dem Header `X-Origin-Secret` angenommen, alle anderen bekommen 403 und einen Protokolleintrag (Art „Origin-Prüfung“, höchstens einmal je Adresse und Minute). Der Healthcheck des Containers ist ausgenommen. Die Administration zeigt, ob die Prüfung aktiv ist.

## 0.9.2

Aktualisierung von 0.9.1: Die Datenbank wird beim Start automatisch erweitert (neue Tabellen für Sicherheitsfragen und Hinweise). `ADMIN_EMAIL`/`ADMIN_PASSWORD` stehen nicht mehr in der Compose-Datei; neue Installationen legen den Administrator im Browser an, bestehende bleiben unverändert. Bestehende Konten ohne Sicherheitsfragen sehen einen Hinweis zum Festlegen.

- **Benutzer direkt anlegen:** Administratoren können ein Konto sofort mit Initialpasswort anlegen (statt per Einladungslink). Beim ersten Anmelden legt der Benutzer ein eigenes Passwort fest und trägt fehlende Sicherheitsfragen nach (auch bei jedem anderen Initialpasswort für ein Konto ohne Fragen).
- **Zeiterfassung rein im Web:** Das Tracking lässt sich ohne Companion starten („Im Web starten“) und beenden. „Tracking beenden“ zeigt vorher den Zeitpunkt samt Brutto, Pause und Netto; „Ende festlegen …“ beendet mit eigener Uhrzeit (alles danach entfällt, Vorschau für Brutto/Netto). Der bisherige Knopf „Stoppen“ heißt jetzt „Pause“. Der Hinweis „Companion nicht aktiv“ bleibt. Ohne Companion läuft das Tracking über Mitternacht weiter.
- **Mail bei langem Tracking:** Läuft das Tracking 10 Stunden (Brutto; einstellbar, 0 = aus), erscheint ein Hinweis auf der Übersicht und eine Mail „Läuft dein Tracking noch?“, höchstens einmal am Tag.
- **Passwort zurücksetzen per Sicherheitsfragen:** Bei der Registrierung (und bei der Einrichtung) wählt man drei Fragen aus Vorschlägen oder eigene; unter „Passwort vergessen“ lässt sich das Passwort damit selbst zurücksetzen. Alle Antworten müssen stimmen (Groß-/Kleinschreibung, Leer- und Satzzeichen egal), nur als Hash gespeichert, Sperre nach 5 Fehlversuchen je Konto, IP-Sperre, Hinweis-Mail, alle Sitzungen enden, der zweite Faktor bleibt. Bestehende Konten werden per Hinweis zum Festlegen aufgefordert; Fragen sind unter Einstellungen änderbar. Administratoren können den Weg abschalten (Registrierung & Richtlinien). Sicherungen enthalten die Antworten.
- **Initialpasswort durch Administratoren:** „Neues Passwort setzen“ in der Benutzerverwaltung (eigenes oder zufällig erzeugt). Der Benutzer muss es beim nächsten Passwort-Login ändern, bis dahin ist die Sitzung gesperrt. Ersetzt die bisherige Attrappe „Passwort-Reset senden“.
- Fehlerbehebung: In der Benutzerverwaltung wurde der „Selbst“-Schutz fest auf das Demo-Konto bezogen.
- Fehlerbehebung: Ein falsches Passwort beim Ändern meldete fälschlich „Sitzung abgelaufen“.
- **Ersteinrichtung im Browser:** Der erste Administrator wird nicht mehr über `ADMIN_EMAIL`/`ADMIN_PASSWORD` in der Compose-Datei angelegt, sondern beim ersten Aufruf in der Weboberfläche. Ein Einrichtungscode aus dem Server-Protokoll schützt davor, dass ein Fremder die Einrichtung zuerst abschließt. Bis dahin ist die Registrierung gesperrt. (`ADMIN_EMAIL`/`ADMIN_PASSWORD` funktionieren für Skripte weiterhin, stehen aber nicht mehr in Compose und `.env.example`.)
- **`RUN_AS`:** Benutzer (uid:gid) des Server-Containers einstellbar, damit Sicherungen im eigenen Verzeichnis dem eigenen Benutzer gehören.
- **`DB_PATH`:** Das Datenverzeichnis der Datenbank lässt sich in der `.env` auf einen eigenen Pfad legen (Standard bleibt das Docker-Volume).
- Eine leere Installation (noch kein Benutzer) wird nicht automatisch gesichert; so verdrängt sie im Notfall keine guten Sicherungen.
- `.env.example` und Installationsanleitung: Befehle zum Erzeugen langer, sicherer Werte für `DB_PASSWORD` und `TICKLY_SECRET`.
- Fehlerbehebung: Das Dashboard blieb leer, wenn ein Konto weniger als 61 Tage Verlauf hatte (betraf neue Konten im Echtbetrieb).

## 0.9.1

Erste veröffentlichte Version; Funktionsumfang wie unter 0.9.0 beschrieben (0.9.0 wurde nie veröffentlicht).

- Release-Workflow: ohne Apple-Zertifikat werden unsignierte macOS-Installer gebaut, statt am Signieren zu scheitern.
- Prototyp entfernt.

## 0.9.0 – Erste Veröffentlichung

Funktionsumfang:

- **Erfassung:** Companion-App für Windows und macOS (Tauri) mit Arbeitsbeginn, Sperre/Ruhezustand, aktiver App, Neustart-Fortsetzung, Absturz-Erkennung, Rückfrage „Arbeitest du gerade?“, automatischem Start per Zeitfenster, Stoppen/Fortsetzen (auch in der Weboberfläche) und **Offline-Betrieb** mit späterer Übertragung.
- **Berechnung:** Soll je Wochentag, Brutto/Netto, automatische Pausenregeln, Feiertage je Bundesland, Urlaub/Krankheit/Zeitausgleich, eigene freie Tage, Überstunden je Woche/Monat/gesamt, Prognose, Übertrag bei der Registrierung.
- **Auswertung:** Tagesansicht mit Zeitverlauf und Programmwechseln, Wochen-/Monats-/Jahresstatistik (Arbeitszeit ohne Pausen je Tag), App-Rangliste mit Symbolen, CSV-Export, Hell/Dunkel/System.
- **Nachträglich bearbeiten** und manuell ergänzen, mit Protokoll und einstellbarem Zeitraum.
- **Benutzer:** offene oder geschlossene Registrierung, Freigabe, Einladungen, E-Mail-Bestätigung, Passwort zurücksetzen/ändern, Passwortrichtlinie.
- **Anmeldung:** Passkeys, Authenticator-App (TOTP) mit Wiederherstellungscodes, **MFA-Pflicht mit Karenzzeit**, einstellbare Sitzungsdauer.
- **Betrieb:** PostgreSQL, Docker/Compose, Cloudflare-Tunnel, echte Client-IP hinter Proxy, **Protokoll**, **automatische IP-Sperren** mit Entsperren, Sitzungen und Geräte (IP, Land, Version) für Administratoren, **SMTP per Oberfläche**, **Sicherung/Wiederherstellung** mit Zeitplan, getrenntem Verzeichnis und Verschlüsselung (`TICKLY_SECRET`).

Lizenz: MIT.

Bekannte Grenzen siehe [README](README.md#bekannte-grenzen): Windows-Companion nicht auf Windows getestet, Installer unsigniert, Docker-Image in dieser Umgebung nicht gebaut.
