# Sicherheit und Datenschutz

## Was gespeichert wird

| Daten | Inhalt | Schutz |
|---|---|---|
| Konten | Name, E-Mail, Rolle, Einstellungen | Datenbank |
| Passwörter | nur als **Argon2id-Hash** | nie im Klartext, nie in Protokollen |
| Passkeys | öffentlicher Schlüssel und Zähler | privater Schlüssel bleibt auf dem Gerät |
| TOTP-Geheimnis | Schlüssel der Authenticator-App | mit `TICKLY_SECRET` **verschlüsselt** (sonst Klartext) |
| Wiederherstellungscodes | nur als SHA-256-Hash | |
| Zeiten | Start/Ende, Sperrzeiten, **App-Namen** je Abschnitt | **keine Fenstertitel, keine Inhalte, keine Screenshots, keine Tastatureingaben** |
| Geräte-Token | nur als SHA-256-Hash; auf dem Gerät: macOS Datei nur für den Benutzer lesbar (kein Schlüsselbund), Windows Anmeldeinformationsverwaltung | Aufheben macht es sofort ungültig |
| API-Schlüssel | nur als **SHA-256-Hash** plus kurzes Präfix (`tkl_ab12cd34…`) zur Wiedererkennung; der Klartext wird **einmal** beim Anlegen angezeigt | Widerruf macht ihn sofort ungültig; Teil der Sicherung (als Hash) |
| SMTP-Passwort | | mit `TICKLY_SECRET` verschlüsselt |
| Protokoll | Zeit, Benutzer, **IP-Adresse**, Land, Browser, Ereignis | nur Administratoren; Löschung nach `LOG_RETENTION_DAYS` |
| Sitzungen | zufällige Kennung, IP, Land, Browser | Beenden jederzeit |

## Anmeldung

- **Passkeys (WebAuthn)** – phishingresistent, an die Domain gebunden.
- **Passwort** mit Richtlinie: mind. 14 Zeichen, Groß-/Klein, Zahl, Sonderzeichen. Optional **TOTP** als zweiter Faktor mit 10 Wiederherstellungscodes. Der Administrator kann **MFA verpflichtend** machen (Karenzzeit, 24-Stunden-Sitzungen, danach Einrichtungszwang).
- **Sitzungen** per `HttpOnly`-Cookie (`SameSite=Lax`, `Secure` bei HTTPS), serverseitig gespeichert, beendbar, einstellbare Dauer.
- **Schutz vor Raten:** automatische IP-Sperre, Begrenzung der TOTP-Versuche (10 je Benutzer, 5 je Anmeldeticket), jeder TOTP-Code gilt nur einmal, Kopplungscodes (8 Zeichen, 10 Minuten, einmalig) und Reset-/Bestätigungslinks (zufällige 256-Bit-Token, nur als Hash gespeichert, einmalig, 1 h bzw. 48 h).
- Antworten bei „Passwort vergessen“ und Anmeldung verraten nicht, ob ein Konto existiert (für unbekannte Adressen kommen stabile Scheinfragen). Die Registrierung nennt bereits vergebene Adressen; eine eigene Sicherheitsfrage kann ebenfalls auf ein Konto hindeuten.
- **Sicherheitsfragen** (Passwort selbst zurücksetzen) sind ein bewusst schwächerer Weg: Antworten lassen sich erraten oder recherchieren. Deshalb: drei Fragen, alle müssen stimmen; Antworten stehen nur als Argon2-Hash in der Datenbank; 5 Fehlversuche je Konto und Stunde sperren den Weg (das kann ein Angreifer ausnutzen, um den Weg für ein Opfer zu blockieren, der Link per Mail und die Anmeldung bleiben davon unberührt), zusätzlich die IP-Sperre; ein Reset beendet alle Sitzungen, ändert aber **nie** Passkey oder TOTP; der Benutzer bekommt eine Mail. Administratoren können den Weg abschalten.
- **Initialpasswort:** Ein Administrator kann ein Passwort setzen. Es gilt nur bis zur ersten Passwort-Anmeldung: Die Sitzung ist gesperrt, bis der Benutzer ein eigenes festlegt.

## API-Schlüssel

Persönliche Schlüssel (`tkl_` + 256 Bit Zufall, Details in [API.md](API.md)) gelten **nur für `/api/v1/*`**. Sie ersetzen kein Passwort und öffnen weder Administration noch Passwörter, Passkeys, Authenticator-App, Geräte-Kopplung, Sitzungen oder Schlüsselverwaltung: Diese Pfade lesen ausschließlich das Sitzungs-Cookie.

- **Speicherung:** nur SHA-256-Hash (bei 256 Bit Zufall genügt ein schneller Hash, wie bei Geräte-Token) und ein Präfix. Der Klartext erscheint genau einmal in der Antwort beim Anlegen und ist danach nicht mehr abrufbar, auch nicht für Administratoren.
- **Rechte:** „Nur lesen“ erlaubt nur `GET`. Laufzeit, Obergrenze (20 aktive je Benutzer), Schalter „API-Schlüssel erlauben“ und „unbegrenzt erlauben“ bestimmt der Administrator. Abgelaufene und widerrufene Schlüssel antworten mit 401.
- **Konten:** Gesperrte Benutzer können ihre Schlüssel nicht nutzen, gelöschte Benutzer verlieren sie. Bewusste Entscheidung: Eine **Passwortänderung widerruft Schlüssel nicht**. Schlüssel sind eigenständige, einzeln widerrufbare Zugänge (z. B. für ein laufendes Skript); wer sie nach einem Vorfall loswerden will, widerruft sie in den Einstellungen oder lässt sie vom Administrator ungültig machen.
- **Schutz vor Raten:** Unbekannte Schlüssel zählen wie falsche Anmeldungen für die automatische IP-Sperre; gesperrte Adressen kommen auch mit gültigem Schlüssel nicht durch. `ORIGIN_SECRET` gilt unverändert für alle Anfragen.
- **Protokoll:** Anlegen, Widerrufen (auch durch Administratoren) und Ablehnungen bekannter Schlüssel (höchstens alle zehn Minuten je Schlüssel und Grund). „Zuletzt benutzt“ wird höchstens einmal je Minute geschrieben.

## Verschlüsselung ruhender Daten

Mit **`TICKLY_SECRET`** (mind. 24 Zeichen; oder `TICKLY_SECRET_FILE`) werden verschlüsselt: **Sicherungen** (Dateien und Datenbankeinträge), **TOTP-Geheimnisse** und das **SMTP-Passwort**. Verfahren: AES-256-GCM, Schlüssel per HKDF-SHA256 aus dem Geheimnis abgeleitet, je Wert ein zufälliger Nonce.

- Das Geheimnis liegt **nur in der Umgebung des Servers** (Variable, Docker-Secret, Passwortmanager) – nicht in der Datenbank und nicht in den Sicherungen.
- Beim Start prüft der Server, ob das Geheimnis zu den Daten passt, und verweigert den Start, statt Daten unlesbar zu machen. Wird erstmals ein Geheimnis gesetzt, werden vorhandene TOTP-Geheimnisse nachträglich verschlüsselt.
- **Geht das Geheimnis verloren, sind verschlüsselte Sicherungen, TOTP und das SMTP-Passwort nicht wiederherstellbar.** (Nutzer können dann TOTP neu einrichten, der SMTP-Zugang muss neu eingegeben werden.)
- Die **übrigen Daten** (Zeiten, Konten) liegen unverschlüsselt in PostgreSQL. Für Verschlüsselung der gesamten Datenbank sorgst du auf Speicherebene (verschlüsseltes Dateisystem/Volume, Verschlüsselung beim Hoster).

## Netzwerk

- TLS endet am vorgeschalteten Proxy (Zoraxy, Nginx, Cloudflare …); die Verbindung Proxy → Tickly ist unverschlüsseltes HTTP und gehört in ein vertrauenswürdiges Netz. Der Port lauscht auf allen Adressen; **`ORIGIN_SECRET`** sorgt dafür, dass nur Anfragen mit dem passenden Header `X-Origin-Secret` angenommen werden (Vergleich in konstanter Zeit, Wert nie im Protokoll). Verstöße bekommen 403 und stehen im Protokoll, je Adresse höchstens einmal pro Minute. Das ersetzt keine Firewall.
- Die **echte Client-IP** gilt nur, wenn die Verbindung von einem vertrauenswürdigen Proxy kommt (lokal, private Netze, `TRUSTED_PROXIES`). Wer den Server direkt erreicht, kann seine Adresse nicht fälschen.
- **Update-Prüfung:** Der Server kontaktiert standardmäßig **`api.github.com`** (HTTPS), um die neueste Version zu erfahren, beim Start verzögert und danach höchstens alle 6 Stunden. Übertragen wird nur die Anfrage selbst: `GET /repos/<Repository>/releases/latest` mit dem User-Agent `tickly-server/<Version>`; GitHub sieht dabei zwangsläufig die IP-Adresse des Servers. Es werden **keine Benutzer-, Geräte- oder Zeitdaten** gesendet, keine Cookies und keine Kennungen. Die Antwort (Version, Link, gekürzte Versionshinweise, Download-Links) wird in der Datenbank zwischengespeichert; Fehler stehen nur im Server-Protokoll. Abschalten: `TICKLY_UPDATE_CHECK=0` (hat Vorrang) oder in der Serververwaltung unter *System*; im Demo-Modus wird nie angefragt. Der Companion geht dafür **nicht** selbst ins Netz, er bekommt die Information vom Server; heruntergeladen wird erst nach einem Klick im Browser.
- Der Container läuft als unprivilegierter Benutzer (Standard UID 65532, per `RUN_AS` änderbar) auf einem Image ohne Shell.

## Update-Prüfung

Siehe oben unter *Netzwerk*: ausgehend nur eine anonyme Anfrage an GitHub, abschaltbar.

## Datenschutz (DSGVO)

Tickly erfasst personenbezogene Beschäftigtendaten (Arbeitszeiten, App-Namen, IP-Adressen). Das ist in vielen Ländern mitbestimmungs- und informationspflichtig. Hinweise:

- **Transparenz:** Benutzer sehen alles, was erfasst wird, in der Tagesansicht. Informiere sie vorab, was erfasst wird (und was nicht).
- **Datensparsamkeit:** App-Erfassung lässt sich abschalten, Apps lassen sich ausschließen; Fenstertitel werden nie gelesen.
- **Aufbewahrung:** Arbeitszeitdaten haben **keine automatische Löschfrist** (bewusste Entscheidung) – lege eine organisatorische Regel fest und lösche Konten bei Bedarf in der Serververwaltung. Das Protokoll wird automatisch nach `LOG_RETENTION_DAYS` bereinigt.
- **Auftragsverarbeitung:** Betreibst du Tickly selbst, bleibt alles bei dir. Ein vorgeschalteter Dienst wie Cloudflare sieht als Proxy den Datenverkehr; schließe ggf. einen AV-Vertrag.
- Dieses Dokument ist keine Rechtsberatung.

## Bekannte Grenzen

- Kein Rate-Limit außerhalb der Anmeldewege, keine CSRF-Token (Schutz über `SameSite=Lax` und JSON-Anfragen).
- SMTP-Server und Sicherungsverzeichnis werden von Administratoren festgelegt und nicht weiter eingeschränkt (ein Administrator kann so interne Adressen ansprechen).
- Installer der Companion-App sind **nicht signiert**.
- Sicherheitslücken bitte vertraulich melden: siehe [SECURITY.md](../SECURITY.md).
