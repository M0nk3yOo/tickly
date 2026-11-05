# Entwicklung und Release

## Voraussetzungen

- Rust (stable, `rustup`), Node.js ≥ 20
- PostgreSQL-Programme (`brew install postgresql@17` bzw. `apt install postgresql`) für Tests und Entwicklung
- Für die Companion-App: Tauri-Voraussetzungen ([tauri.app](https://tauri.app/start/prerequisites/)); unter macOS genügen die Xcode Command Line Tools

## Server und Weboberfläche starten

```sh
scripts/dev.sh                     # PostgreSQL (Port 5433, Daten in .pgdata) + Server im Demo-Modus, http://localhost:3000
TICKLY_DEMO=0 scripts/dev.sh                                 # Echtbetrieb ohne Demo-Daten: Einrichtung im Browser, Code im Terminal
# (für Skripte weiterhin möglich: ADMIN_EMAIL=… ADMIN_PASSWORD='…' legt den ersten Administrator direkt an)
```

`web/` wird direkt ausgeliefert – nach einer Änderung genügt Neuladen im Browser. Rust-Änderungen: Server neu starten (`cargo run` baut neu).

## Tests

```sh
cargo test --workspace             # 70+ Tests: Logik, Companion-Kern, API gegen echtes PostgreSQL
node scripts/test-dayedit.mjs      # Rechenkern von „Zeiten bearbeiten“ (web/dayedit.js)
```

Die API-Tests brauchen eine PostgreSQL-Instanz (`TEST_DATABASE_URL`, Standard `postgres://tickly@localhost:5433/postgres`, von `scripts/dev.sh` bereitgestellt). Jeder Test arbeitet in einer aus einer Vorlage kopierten eigenen Datenbank. Sie decken u. a. ab: Rechenlogik, Passkey-Zeremonie mit Software-Token, TOTP, MFA-Pflicht, IP-Sperren, Sitzungen, Sicherung/Wiederherstellung (auch verschlüsselt, auch alte Dateien), Mails über einen Test-SMTP-Server, Offline-Betrieb des Companion gegen den echten Server.

Windows-Teil des Companion prüfen (ohne Windows): `cargo check -p tickly-platform --target x86_64-pc-windows-msvc` (Ziel mit `rustup target add` installieren).

## Companion-App

```sh
cd apps/companion
npm install
npm run dev            # nur die Oberfläche im Browser mit Beispieldaten, http://localhost:1420/?track=asking
npx tauri dev          # komplette App
npx tauri build        # Installer für das aktuelle System
```

Die App ist ein eigenes Cargo-Projekt (`apps/companion/src-tauri`), nicht Teil des Workspace.

## Screenshots für die Dokumentation

Siehe Kopf von `scripts/screenshots.mjs` (Demo-Server und Vite-Server laufen lassen, Chrome/Edge nötig).

## Release

Releases entstehen **lokal** mit `scripts/release.sh`: Es baut die Installer und das Docker-Image auf deinem Rechner und veröffentlicht alles als GitHub-Release.

Version steht an vier Stellen und muss gemeinsam erhöht werden: `Cargo.toml` (`[workspace.package]`), `apps/companion/package.json`, `apps/companion/src-tauri/tauri.conf.json`, `apps/companion/src-tauri/Cargo.toml` (und `apps/companion/src/api.ts` für die Vorschau).

1. `CHANGELOG.md` um einen Abschnitt `## <Version>` ergänzen, Versionen erhöhen, committen und nach `main` pushen.
2. `scripts/release.sh 0.9.4 --dry-run` baut alles nach `release/0.9.4/` (Probelauf, nichts wird veröffentlicht).
3. `scripts/release.sh 0.9.4` baut, setzt und pusht den Tag `v0.9.4` und legt das Release mit allen Dateien und `SHA256SUMS` an.

**Einmal einrichten (macOS):**
```sh
brew install llvm nsis docker docker-buildx colima
cargo install --locked cargo-xwin
rustup target add aarch64-apple-darwin x86_64-pc-windows-msvc
colima start --cpu 4 --memory 8 --disk 60
gh auth refresh -h github.com -s write:packages,read:packages     # damit das Image nach ghcr.io geschoben werden darf
```
Docker braucht `"cliPluginsExtraDirs": ["/opt/homebrew/lib/docker/cli-plugins"]` in `~/.docker/config.json`, damit `docker buildx` gefunden wird.

**Was lokal geht und was nicht:**
- macOS-`.dmg` (nur Apple Silicon, keine Intel-Macs) und Docker-Image (linux/amd64 und linux/arm64): ja.
- Windows: nur der **NSIS-Installer (`…-setup.exe`)** per Cross-Build mit `cargo-xwin`. Das `.msi` braucht WiX und gibt es nur unter Windows.
- Der GitHub-Workflow **Release** (`.github/workflows/release.yml`) startet nur noch von Hand und ist die Reserve, falls du ein `.msi` brauchst oder ohne eigenen Rechner bauen willst.

**Signierung (empfohlen vor dem Einsatz im Team):**
- *macOS:* Apple-Developer-ID + Notarisierung; Secrets `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` setzen (siehe Tauri-Dokumentation „Code Signing“).
- *Windows:* Code-Signing-Zertifikat; `TAURI_SIGNING_…`/Zertifikats-Secrets bzw. Azure Trusted Signing.

## Mitwirken

Tickly steht unter der [GPL, Version 3 oder später](../LICENSE); mit einem Beitrag stimmst du zu, dass er unter dieser Lizenz veröffentlicht wird.

Fehler und Wünsche als Issue. Bei Änderungen bitte Tests ergänzen und `cargo test --workspace` grün halten. Texte der Oberfläche sind deutsch, Datum und Zeit immer als `TT.MM.JJJJ` und 24 Stunden (keine nativen `type=date`/`type=time`-Felder).
