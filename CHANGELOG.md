# Änderungen

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
