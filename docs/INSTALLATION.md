# Installation und Betrieb

Tickly besteht aus dem **Server** (ein Container mit API und Weboberfläche, dazu PostgreSQL) und der **Companion-App** auf den Arbeitsplätzen. Diese Seite beschreibt den Server.

## Voraussetzungen

- Ein Rechner oder Server mit **Docker** und **Docker Compose** (empfohlen), etwa 1 GB RAM.
- Eine **Domain mit HTTPS** für den öffentlichen Zugang. Passkeys und die Companion-Kopplung brauchen HTTPS, sobald der Server nicht nur lokal läuft. Geplant ist **Cloudflare** (Tunnel, keine offenen Ports) – ein eigener Reverse-Proxy ist nicht nötig.
- Optional: ein **SMTP-Server** für E-Mails (kann später in der Oberfläche eingerichtet werden).

## 1. Konfiguration

```sh
git clone <dieses Repository> tickly && cd tickly
cp .env.example .env
```

Sichere Werte erzeugst du direkt im Terminal (macOS, Linux, unter Windows in Git Bash oder WSL). Länger ist besser:

```sh
openssl rand -hex 32        # → DB_PASSWORD (64 Zeichen, nur Hex, damit die Datenbank-Adresse heil bleibt)
openssl rand -base64 48     # → TICKLY_SECRET (64 Zeichen; "-base64 96" für noch länger)
```

Beide Werte gehören zusätzlich in deinen Passwortmanager. Das Passwort des Administrators legst du später im Browser fest (siehe [Ersteinrichtung](#ersteinrichtung-im-browser)); ein Passwortmanager erzeugt dafür ein langes Passwort, oder im Terminal: `echo "$(openssl rand -base64 36)-Aa1#"`.

Wichtige Werte in `.env`:

| Variable | Bedeutung |
|---|---|
| `PUBLIC_URL` | Öffentliche Adresse, z. B. `https://zeit.example.com`. **Passkeys sind an diese Domain gebunden** – eine spätere Änderung macht sie unbrauchbar. |
| `DB_PASSWORD` | Passwort der Datenbank (nur im internen Netz erreichbar). Zufällig und lang, nur Hex-Zeichen: `openssl rand -hex 32`. |
| `TICKLY_SECRET` | Schlüssel zum **Verschlüsseln** von Sicherungen, TOTP-Geheimnissen und dem SMTP-Passwort (mind. 24 Zeichen, besser länger: `openssl rand -base64 48`). **Sicher aufbewahren, nie ändern oder verlieren** – ohne ihn sind verschlüsselte Daten nicht mehr lesbar. |
| `CLOUDFLARE_TUNNEL_TOKEN` | Token des Tunnels (siehe unten), nur für `--profile tunnel`. |
| `BACKUP_PATH` | Pfad für Sicherungen, besser auf einem anderen Laufwerk (siehe [Sicherung](#sicherung)). |
| `TICKLY_TZ` | Zeitzone für Arbeitstage, Standard `Europe/Berlin`. |
| `TRUSTED_PROXIES` | Zusätzliche Proxy-Adressen, deren Weiterleitungs-Header gelten (Komma getrennt, auch CIDR). Lokale/private Adressen (Docker-Netz) gelten automatisch. |
| `LOG_RETENTION_DAYS` | So lange bleibt das Protokoll (Standard 365). |
| `COMPANION_LATEST_VERSION` | Neueste Companion-Version für den Hinweis „veraltet“. Standard: höchste gemeldete. |

Weitere Umgebungsvariablen des Servers (selten nötig): `PORT`, `BIND`, `WEBAUTHN_RP_ID`, `DATABASE_URL` (ohne Compose), `TICKLY_SECRET_FILE` (Datei statt Variable, z. B. Docker-Secret), `RUST_LOG`.

## 2. Starten

```sh
docker compose up -d --build        # nur lokal: http://localhost:3000
```

Der erste Build dauert einige Minuten. Beim ersten Start legt der Server das Datenbankschema an.

### Ersteinrichtung im Browser

Der erste Administrator wird **nicht in der `.env`**, sondern im Browser angelegt:

1. Einrichtungscode aus dem Protokoll holen: `docker compose logs api | grep Einrichtungscode`
2. `PUBLIC_URL` öffnen (lokal `http://localhost:3000`). Solange es keinen Benutzer gibt, zeigt Tickly die Einrichtung.
3. Code, Name, E-Mail und ein Passwort eintragen (mind. 14 Zeichen, Groß-/Kleinbuchstaben, Zahl, Sonderzeichen; gerne deutlich länger, am besten aus einem Passwortmanager). Danach bist du als Administrator angemeldet.
4. Richte unter *Einstellungen → Sicherheit* einen Passkey oder TOTP ein, denn Administratoren brauchen wie alle einen zweiten Faktor.

Der Code schützt davor, dass jemand, der die Adresse zuerst erreicht, den Administrator anlegt. Er ändert sich bei jedem Neustart, solange noch kein Benutzer existiert, und wird danach nicht mehr gebraucht. Bis zur Einrichtung ist die Registrierung gesperrt, falsche Codes zählen wie Fehlanmeldungen für die IP-Sperre.

Der Port ist an `127.0.0.1` gebunden. **Gib ihn nie direkt ins Internet frei**; öffentlicher Zugang läuft über den Tunnel (oder deinen eigenen Proxy, der die echte Adresse in `CF-Connecting-IP` bzw. `X-Forwarded-For` setzt).

## 3. Öffentlich über Cloudflare

1. Cloudflare Zero Trust → *Networks* → *Tunnels* → Tunnel anlegen, Token kopieren → in `.env` als `CLOUDFLARE_TUNNEL_TOKEN`.
2. Beim Tunnel einen **Public Hostname** anlegen: deine Domain, Service `http://api:3000`.
3. Starten mit Tunnel:
   ```sh
   docker compose --profile tunnel up -d --build
   ```

Cloudflare beendet TLS. Der Server erkennt über `CF-Connecting-IP` die **echte Besucher-Adresse** und über `CF-IPCountry` das Land (für Sperren, Protokoll und Sitzungsliste). Das Cookie wird als `Secure` gesetzt, sobald `PUBLIC_URL` mit `https://` beginnt.

## 4. Companion-Apps koppeln

In der Weboberfläche unter **Companion-App → Kopplungscode erzeugen** und den Code in der Companion-App eingeben. Siehe [Companion-App](COMPANION.md).

## Sicherung

- **Automatisch** täglich zur eingestellten Uhrzeit (Standard 02:00), die letzten Sicherungen bleiben erhalten; einstellbar unter *Administration → Sicherung*.
- **Getrenntes Verzeichnis:** Im Compose-Betrieb liegt `/backups` im Docker-Volume `tickly-backups`. Besser ein Pfad auf einem anderen Laufwerk oder Server (NAS):
  ```sh
  mkdir -p /mnt/nas/tickly-backups && sudo chown 65532 /mnt/nas/tickly-backups   # der Container läuft als UID 65532
  # in .env:  BACKUP_PATH=/mnt/nas/tickly-backups
  ```
- **Verschlüsselt**, wenn `TICKLY_SECRET` gesetzt ist (Dateiendung `.json.gz.enc`).
- **Wiederherstellen:** *Administration → Sicherung* → Datei wählen oder eine gespeicherte Sicherung. Der jetzige Stand wird vorher automatisch gesichert.
- Zusätzlich auf Datenbankebene möglich: `docker compose exec db pg_dump -U tickly tickly > backup.sql`.

### Neuaufbau nach einem Totalausfall

Wenn nur das Sicherungsverzeichnis übrig ist:

1. Server neu aufsetzen (siehe oben) mit **demselben `TICKLY_SECRET`** und derselben `PUBLIC_URL` (Passkeys sind an die Domain gebunden). Beim ersten Aufruf legst du über die Ersteinrichtung (Code aus `docker compose logs api`) einen vorläufigen Administrator an.
2. Das gerettete Verzeichnis als `BACKUP_PATH` einbinden (oder die Datei später hochladen).
3. Als vorläufiger Administrator anmelden → *Administration → Sicherung* → die gewünschte Sicherung wählen → *Wiederherstellen*.
4. Danach bist du abgemeldet, weil der vorläufige Administrator durch die Wiederherstellung ersetzt wird. **Melde dich mit einem Konto aus der Sicherung an** (Passwort bzw. Passkey/TOTP wie vorher). Der vorläufige Administrator existiert dann nicht mehr.

Hinweise: Der neue Server legt beim Start selbst eine Sicherung des (leeren) Stands in dasselbe Verzeichnis. Wähle in der Liste deshalb die richtige Datei nach Datum aus, nicht einfach die neueste, und kopiere das Verzeichnis vorher an einen sicheren Ort. Mit falschem `TICKLY_SECRET` meldet Tickly „Entschlüsselung fehlgeschlagen“ und ändert nichts. Companion-Geräte bleiben gekoppelt; Sitzungen sind nicht Teil der Sicherung, alle anderen müssen sich neu anmelden.

Die **Sicherung ersetzt kein Backup des Servers**: Kopiere das Sicherungsverzeichnis regelmäßig weg und bewahre `TICKLY_SECRET` getrennt davon auf.

## Aktualisieren

```sh
git pull
docker compose up -d --build
```

Datenbankänderungen werden beim Start automatisch eingespielt. Lies vorher den [Changelog](../CHANGELOG.md). Sichere vor jedem Update.

## Ohne Docker

Rust-Toolchain und PostgreSQL ≥ 14 vorausgesetzt:

```sh
createdb tickly
export DATABASE_URL=postgres://benutzer:passwort@localhost/tickly
export PUBLIC_URL=https://zeit.example.com TICKLY_SECRET=…
cargo run --release -p tickly-api      # liest die Weboberfläche aus ./web (WEB_DIR)
```

## Fehlersuche

| Problem | Lösung |
|---|---|
| Server startet nicht: „TICKLY_SECRET passt nicht zu den vorhandenen Daten“ | Es wurde ein anderes Geheimnis gesetzt als beim ersten Verschlüsseln. Das ursprüngliche wieder eintragen. |
| „Die Daten sind mit TICKLY_SECRET verschlüsselt, aber es ist nicht gesetzt“ | `TICKLY_SECRET` fehlt in der Umgebung. |
| Seite zeigt „Willkommen bei Tickly“ und verlangt einen Code | Es gibt noch keinen Benutzer. Den Einrichtungscode zeigt `docker compose logs api`. |
| Passkey-Anmeldung funktioniert nicht | `PUBLIC_URL` muss exakt die Adresse sein, unter der du den Server aufrufst (HTTPS bzw. `localhost`). |
| Alle Besucher erscheinen mit einer internen Adresse (172.x) | Der Proxy liefert keine `CF-Connecting-IP`/`X-Forwarded-For`, oder seine Adresse ist nicht vertrauenswürdig → `TRUSTED_PROXIES` setzen. |
| Sicherung ins Verzeichnis schlägt fehl | Rechte: Der Container läuft als UID 65532 und muss schreiben dürfen. Die Prüfung steht unter *Administration → Sicherung*. |
| Gesperrt wegen Fehlversuchen | Anderer Administrator entsperrt unter *Sicherheit & Sperren*; notfalls `docker compose exec db psql -U tickly -c "update ip_blocks set unblocked_at = now() where unblocked_at is null"`. |
