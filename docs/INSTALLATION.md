# Installation und Betrieb

Tickly besteht aus dem **Server** (ein Container mit API und Weboberfläche, dazu PostgreSQL) und der **Companion-App** auf den Arbeitsplätzen. Diese Seite beschreibt den Server.

## Voraussetzungen

- Ein Rechner oder Server mit **Docker** und **Docker Compose** (empfohlen), etwa 1 GB RAM.
- Eine **Domain mit HTTPS** für den öffentlichen Zugang. Passkeys und die Companion-Kopplung brauchen HTTPS, sobald der Server nicht nur lokal läuft. Davor gehört ein **Reverse-Proxy mit TLS**, zum Beispiel Zoraxy, Nginx, Caddy oder Cloudflare (siehe [Öffentlich erreichbar machen](#3-öffentlich-erreichbar-machen)).
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
| `ORIGIN_SECRET` | Geheimer Wert für den Header `X-Origin-Secret`. Ist er gesetzt, nimmt Tickly nur Anfragen an, die ihn mitbringen (siehe [Öffentlich erreichbar machen](#3-öffentlich-erreichbar-machen)). Mind. 24 Zeichen, besser länger: `openssl rand -base64 48`. |
| `TICKLY_PORT`, `TICKLY_BIND` | Port (Standard 3000) und Adresse, auf der er lauscht (Standard `0.0.0.0` = von außen erreichbar; nur lokal: `127.0.0.1`). |
| `DB_PATH` | Wo die Datenbank ihre Dateien ablegt, Standard: Docker-Volume. Mit einem Pfad (z. B. `/srv/tickly/db`) liegen die Daten dort, siehe [Datenbank an einen anderen Ort legen](#datenbank-an-einen-anderen-ort-legen). |
| `RUN_AS` | Benutzer des Server-Containers als `uid:gid`, Standard `65532:65532`. Eigenen Benutzer eintragen, damit dir die Dateien in `BACKUP_PATH` gehören: `echo "RUN_AS=$(id -u):$(id -g)" >> .env`. Gilt nur für den Server, nicht für die Datenbank. |
| `BACKUP_PATH` | Pfad für Sicherungen, besser auf einem anderen Laufwerk (siehe [Sicherung](#sicherung)). |
| `TICKLY_TZ` | Zeitzone für Arbeitstage, Standard `Europe/Berlin`. |
| `TRUSTED_PROXIES` | Zusätzliche Proxy-Adressen, deren Weiterleitungs-Header gelten (Komma getrennt, auch CIDR). Lokale/private Adressen (Docker-Netz) gelten automatisch. |
| `LOG_RETENTION_DAYS` | So lange bleibt das Protokoll (Standard 365). |
| `COMPANION_LATEST_VERSION` | Neueste Companion-Version für den Hinweis „veraltet“. Hat Vorrang vor der Update-Prüfung. Standard: die bei GitHub gefundene bzw. höchste gemeldete. |
| `TICKLY_UPDATE_CHECK` | `0` schaltet die Suche nach neuen Versionen ab (der Server kontaktiert dann nie GitHub). Standard: an, siehe [Update-Hinweise](#update-hinweise). |
| `TICKLY_UPDATE_REPO` | GitHub-Repository, dessen neuestes Release geprüft wird. Standard `M0nk3yOo/tickly`. |

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

Der Port lauscht auf **allen Adressen** (`0.0.0.0`). Er spricht nur HTTP, ohne TLS. Stelle deshalb einen Proxy mit HTTPS davor und schütze den Port mit `ORIGIN_SECRET` und einer Firewall (siehe unten). Willst du ihn nur lokal erreichbar machen, setze `TICKLY_BIND=127.0.0.1`.

## 3. Öffentlich erreichbar machen

Tickly spricht HTTP auf Port 3000 (`TICKLY_PORT`). Davor kommt ein **Proxy, der TLS beendet** und an `http://<Docker-Host>:3000` weiterleitet. Liegt der Proxy im selben Docker-Netz, ist `http://api:3000` das Ziel.

**1. `PUBLIC_URL`** in der `.env` auf die öffentliche Adresse setzen, exakt wie im Browser (`https://zeit.example.com`). Passkeys und Cookies hängen daran.

**2. Origin-Secret.** Weil der Port von außen erreichbar ist, kann ihn jeder direkt ansprechen und den Proxy umgehen. Mit `ORIGIN_SECRET` akzeptiert Tickly nur Anfragen, die den Header `X-Origin-Secret` mit genau diesem Wert tragen. Alles andere bekommt `403` und einen Eintrag im Protokoll (*Administration → Protokoll → Origin-Prüfung*, auch unter „Fehlversuche“).

```sh
openssl rand -base64 48        # in die .env als ORIGIN_SECRET
```

Den Header setzt der Proxy bei jeder Anfrage an Tickly:
- **Zoraxy:** In der Regel für deine Domain unter den erweiterten Einstellungen einen **benutzerdefinierten Header** (Custom Header) hinzufügen: Name `X-Origin-Secret`, Wert wie in der `.env`.
- **Nginx:** `proxy_set_header X-Origin-Secret "WERT";`
- **Caddy:** `reverse_proxy tickly:3000 { header_up X-Origin-Secret "WERT" }`
- **Cloudflare (ohne weiteren Proxy):** *Rules → Transform Rules → Modify Request Header* → *Set static*, Name `X-Origin-Secret`, Wert wie in der `.env`.

Ohne gesetzten Header sieht man nur „403 Forbidden“. Prüfen kannst du es so: `curl -i -H 'X-Origin-Secret: WERT' http://HOST:3000/api/health` liefert `ok`. Der Docker-Healthcheck ist ausgenommen (er kommt von innen).

Das Protokoll schreibt je Adresse höchstens einen Eintrag pro Minute, mit Pfad, Grund (fehlt/falsch) und der Adresse der Verbindung; weitere Anfragen werden gezählt und im nächsten Eintrag genannt. Der Wert des Headers wird nie protokolliert.

**3. Firewall.** Öffne den Port nur für das, was ihn braucht. Der Proxy muss Tickly erreichen, das Internet nicht. Das Origin-Secret ist eine zweite Sicherung, kein Ersatz für eine Firewall, und es schützt nicht vor Mithören im selben Netz, weil die Verbindung Proxy → Tickly unverschlüsselt ist. Betreibe sie nur in einem vertrauenswürdigen Netz.

**4. Echte Besucher-Adresse.** Der Server liest sie aus `CF-Connecting-IP`, `X-Forwarded-For` oder `X-Real-IP`, aber nur, wenn die Verbindung von einem vertrauenswürdigen Proxy kommt. Lokale und private Adressen (Docker-Netz, LAN) gelten automatisch, weitere Adressen trägst du unter `TRUSTED_PROXIES` ein. Mit Cloudflare kommt zusätzlich das Land über `CF-IPCountry`. Das Cookie wird als `Secure` gesetzt, sobald `PUBLIC_URL` mit `https://` beginnt.

**Fehler 521 (Cloudflare):** Cloudflare erreicht deinen Server nicht. Meist stimmt der DNS-Eintrag nicht, der Proxy lauscht nicht auf dem Port, auf den Cloudflare zugreift (443 bei „Full“), oder eine Firewall blockiert.

## 4. Companion-Apps koppeln

In der Weboberfläche unter **Companion-App → Kopplungscode erzeugen** und den Code in der Companion-App eingeben. Siehe [Companion-App](COMPANION.md).

## Datenbank an einen anderen Ort legen

Standardmäßig liegt die Datenbank in einem Docker-Volume (`tickly-db`). Willst du sie auf ein bestimmtes Laufwerk legen, setze in der `.env` einen Pfad:

```sh
DB_PATH=/srv/tickly/db      # absolut, oder relativ zur docker-compose.yml, z. B. ./db
```

- Das Verzeichnis darf leer sein oder fehlen; Docker legt es an, die Datenbank richtet die Rechte selbst ein. Nimm ein **eigenes Unterverzeichnis**, nicht die Wurzel eines Laufwerks (dort liegt oft ein `lost+found`, mit dem die Datenbank nicht startet).
- Nimm eine **lokale Festplatte oder SSD**. Eine Datenbank auf einem Netzlaufwerk (NFS/SMB) ist langsam und riskant.
- Das gilt für den **ersten Start**. Eine schon laufende Installation ziehst du so um: Sicherung anlegen (siehe unten), `docker compose down`, den neuen Pfad in die `.env`, `docker compose up -d` und die Sicherung unter *Administration → Sicherung* einspielen. Das ist derselbe Weg wie beim [Neuaufbau](#neuaufbau-nach-einem-totalausfall).
- Sichere dieses Verzeichnis nicht durch Kopieren im laufenden Betrieb, sondern über die Sicherungen von Tickly oder `pg_dump`.

## Sicherung

- **Automatisch** täglich zur eingestellten Uhrzeit (Standard 02:00), die letzten Sicherungen bleiben erhalten; einstellbar unter *Administration → Sicherung*.
- **Getrenntes Verzeichnis:** Im Compose-Betrieb liegt `/backups` im Docker-Volume `tickly-backups`. Besser ein Pfad auf einem anderen Laufwerk oder Server (NAS):
  ```sh
  mkdir -p /mnt/nas/tickly-backups && sudo chown 65532 /mnt/nas/tickly-backups   # der Container läuft als UID 65532 (oder als RUN_AS)
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

Mit dem veröffentlichten Image (`ghcr.io/m0nk3yoo/tickly:<Version>`) genügt `docker compose pull && docker compose up -d`; wer aus dem Quellcode baut, nimmt `git pull` und `docker compose up -d --build`.

Welche Version läuft, steht in der Weboberfläche **unten links in der Seitenleiste** („Server 0.9.4“). Nach einem Update siehst du dort, ob der neue Stand aktiv ist.

### Update-Hinweise

Der Server fragt höchstens alle 6 Stunden bei GitHub (`api.github.com`, Repository aus `TICKLY_UPDATE_REPO`) nach dem neuesten Release. Ist es neuer als die laufende Version, sehen **Administratoren** oben in der Weboberfläche den Hinweis „Neue Version x.y.z verfügbar“ (wegklickbar, gilt je Version) samt Link zu den Versionshinweisen. Unter *Serververwaltung → System* stehen installierte und neueste Version, der Zeitpunkt der letzten Prüfung, **Jetzt prüfen** und der Schalter **Nach Updates suchen**. Auch die **Companion-Apps** erfahren so von neuen Versionen (Hinweis mit Download-Knopf). Installiert wird nie von selbst: Die Installer sind nicht signiert, es gibt nur den Hinweis und den Download nach einem Klick.

Abschalten, z. B. in abgeschotteten Netzen: `TICKLY_UPDATE_CHECK=0` in der `.env` (hat Vorrang) oder den Schalter in der Serververwaltung. Dann kontaktiert der Server GitHub nie. Was dabei übertragen wird, steht in [Sicherheit](SICHERHEIT.md#update-prüfung).

**x86 und ARM:** `docker compose up -d --build` baut das Image für die Architektur deines Rechners (x86-64 oder ARM64). Die veröffentlichten Images `ghcr.io/m0nk3yoo/tickly:<Version>` enthalten beide (`linux/amd64` und `linux/arm64`). Die x86-Variante ist mit Rosetta auf einem Apple-Silicon-Mac gestartet und geprüft worden (Server und Datenbank healthy, Origin-Prüfung wirksam), aber nicht auf echter x86-Hardware.

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
| Sicherung ins Verzeichnis schlägt fehl | Rechte: Der Server läuft als `RUN_AS` (Standard UID 65532) und muss schreiben dürfen. Bei eigenem `RUN_AS` muss `BACKUP_PATH` gesetzt sein und diesem Benutzer gehören. Die Prüfung steht unter *Administration → Sicherung*. |
| Gesperrt wegen Fehlversuchen | Anderer Administrator entsperrt unter *Sicherheit & Sperren*; notfalls `docker compose exec db psql -U tickly -c "update ip_blocks set unblocked_at = now() where unblocked_at is null"`. |
