# Änderungen

## Unveröffentlicht

- **Ersteinrichtung im Browser:** Der erste Administrator wird nicht mehr über `ADMIN_EMAIL`/`ADMIN_PASSWORD` in der Compose-Datei angelegt, sondern beim ersten Aufruf in der Weboberfläche. Ein Einrichtungscode aus dem Server-Protokoll schützt davor, dass ein Fremder die Einrichtung zuerst abschließt. Bis dahin ist die Registrierung gesperrt. (`ADMIN_EMAIL`/`ADMIN_PASSWORD` funktionieren für Skripte weiterhin, stehen aber nicht mehr in Compose und `.env.example`.)
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
