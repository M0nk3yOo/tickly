#!/usr/bin/env bash
# Lokales Release: baut die Companion-Installer (macOS, Windows) und das Docker-Image auf diesem Rechner und
# veröffentlicht alles als GitHub-Release.
#
#   scripts/release.sh 0.9.4              bauen, Tag setzen, Release anlegen
#   scripts/release.sh 0.9.4 --dry-run    nur bauen (nach release/0.9.4/), nichts veröffentlichen
#
# Optionen: --skip-tests  --no-windows  --no-docker  --no-push-image (Image nur bauen, nicht nach ghcr.io schieben)
#           --reuse (schon gebaute Installer in release/<Version>/ weiterverwenden, z. B. nach einem Trockenlauf)
#
# Voraussetzungen (siehe docs/ENTWICKLUNG.md): Rust + Ziele aarch64-apple-darwin und x86_64-pc-windows-msvc (nur Apple Silicon, keine Intel-Macs),
# Node, cargo-xwin, llvm und nsis (Homebrew), Docker mit buildx (z. B. Colima), gh (angemeldet), lokales PostgreSQL für die Tests.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-}"; shift || true
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Aufruf: scripts/release.sh <Version, z. B. 0.9.4> [--dry-run] [--skip-tests] [--no-windows] [--no-docker] [--no-push-image] [--reuse]" >&2; exit 2; }
DRY=0; TESTS=1; WIN=1; DOCKER=1; PUSH_IMAGE=1; REUSE=0
for a in "$@"; do case "$a" in
  --dry-run) DRY=1 ;; --skip-tests) TESTS=0 ;; --no-windows) WIN=0 ;; --no-docker) DOCKER=0 ;; --no-push-image) PUSH_IMAGE=0 ;; --reuse) REUSE=1 ;;
  *) echo "Unbekannte Option: $a" >&2; exit 2 ;;
esac; done

export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/opt/homebrew/opt/llvm/bin:/usr/local/bin:$PATH"
step() { printf '\n\033[1m== %s\033[0m\n' "$*"; }
die() { echo "FEHLER: $*" >&2; exit 1; }

REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner 2>/dev/null)" || die "gh ist nicht angemeldet oder das Repository ist nicht erreichbar (gh auth login)"
OUT="release/$VERSION"
TAG="v$VERSION"

step "Voraussetzungen prüfen ($REPO, $TAG)"
[ "$(git rev-parse --abbrev-ref HEAD)" = "main" ] || die "Bitte auf dem Zweig main arbeiten."
[ -z "$(git status --porcelain)" ] || die "Es gibt nicht committete Änderungen. Erst committen und pushen."
git fetch -q https://github.com/"$REPO".git main 2>/dev/null || true
[ "$(git rev-parse HEAD)" = "$(git rev-parse FETCH_HEAD 2>/dev/null || git rev-parse HEAD)" ] || die "Lokaler Stand und origin/main unterscheiden sich. Erst pushen."
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && die "Den Tag $TAG gibt es lokal schon."
if [ "$DRY" = 0 ]; then gh release view "$TAG" >/dev/null 2>&1 && die "Das Release $TAG gibt es schon auf GitHub."; fi
v() { grep -m1 -E "$2" "$1" | sed -E 's/.*"?([0-9]+\.[0-9]+\.[0-9]+)"?.*/\1/'; }
[ "$(v Cargo.toml '^version')" = "$VERSION" ] || die "Cargo.toml hat nicht die Version $VERSION"
[ "$(v apps/companion/package.json '"version"')" = "$VERSION" ] || die "apps/companion/package.json hat nicht die Version $VERSION"
[ "$(v apps/companion/src-tauri/tauri.conf.json '"version"')" = "$VERSION" ] || die "tauri.conf.json hat nicht die Version $VERSION"
[ "$(v apps/companion/src-tauri/Cargo.toml '^version')" = "$VERSION" ] || die "src-tauri/Cargo.toml hat nicht die Version $VERSION"
grep -q "^## $VERSION\$" CHANGELOG.md || die "CHANGELOG.md hat keinen Abschnitt „## $VERSION“"
command -v npm >/dev/null || die "npm fehlt"
INSTALLED_TARGETS="$(rustup target list --installed)"
for t in aarch64-apple-darwin $([ "$WIN" = 1 ] && echo x86_64-pc-windows-msvc); do
  echo "$INSTALLED_TARGETS" | grep -qx "$t" || die "Rust-Ziel $t fehlt (rustup target add $t)"
done
if [ "$WIN" = 1 ]; then for t in cargo-xwin makensis clang-cl; do command -v "$t" >/dev/null || die "$t fehlt (brew install llvm nsis; cargo install --locked cargo-xwin)"; done; fi
if [ "$DOCKER" = 1 ]; then
  docker info >/dev/null 2>&1 || die "Docker läuft nicht (colima start)"; docker buildx version >/dev/null 2>&1 || die "docker buildx fehlt"
  # Früh prüfen, damit nicht erst nach den langen Bauschritten auffällt, dass das Image nicht hochgeladen werden darf
  if [ "$DRY" = 0 ] && [ "$PUSH_IMAGE" = 1 ]; then
    gh auth status 2>&1 | grep -q "write:packages" || die "Dem gh-Token fehlt die Berechtigung write:packages. Einmal ausführen: gh auth refresh -h github.com -s write:packages,read:packages (oder --no-push-image)"
  fi
fi

if [ "$TESTS" = 1 ]; then
  step "Tests"
  PG_BIN="$( (ls -d /opt/homebrew/opt/postgresql@*/bin 2>/dev/null || true) | tail -1)"
  export PATH="$PG_BIN:$PATH"
  if ! pg_isready -h localhost -p 5433 >/dev/null 2>&1; then
    [ -d .pgdata ] || die "Keine Test-Datenbank (PostgreSQL auf Port 5433). scripts/dev.sh einmal starten."
    pg_ctl -D .pgdata -o "-p 5433 -k /tmp" -l .pgdata/server.log start >/dev/null
    sleep 2
  fi
  cargo test --workspace -q
fi

[ "$REUSE" = 1 ] || rm -rf "$OUT"
mkdir -p "$OUT"

if [ "$REUSE" = 1 ] && [ -f "$OUT/Tickly.Companion_${VERSION}_aarch64.dmg" ]; then
  step "Companion für macOS: vorhandene Datei wird weiterverwendet"
else
  step "Companion für macOS (Apple Silicon) bauen"
  ( cd apps/companion && npm ci --silent && env -u APPLE_CERTIFICATE -u APPLE_SIGNING_IDENTITY -u APPLE_ID -u APPLE_PASSWORD -u APPLE_TEAM_ID npx tauri build --target aarch64-apple-darwin )
  DMG="$(ls apps/companion/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/*_"$VERSION"_aarch64.dmg)"
  cp "$DMG" "$OUT/Tickly.Companion_${VERSION}_aarch64.dmg"
fi

if [ "$WIN" = 1 ] && [ "$REUSE" = 1 ] && [ -f "$OUT/Tickly.Companion_${VERSION}_x64-setup.exe" ]; then
  step "Companion für Windows: vorhandene Datei wird weiterverwendet"
elif [ "$WIN" = 1 ]; then
  step "Companion für Windows bauen (NSIS-Installer per Cross-Build; ein .msi geht nur unter Windows)"
  ( cd apps/companion && npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis )
  EXE="$(ls apps/companion/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*_"$VERSION"_x64-setup.exe)"
  cp "$EXE" "$OUT/Tickly.Companion_${VERSION}_x64-setup.exe"
fi

IMAGE="ghcr.io/$(echo "$REPO" | tr 'A-Z' 'a-z')"
PUSHED=0
if [ "$DOCKER" = 1 ]; then
  if [ "$DRY" = 0 ] && [ "$PUSH_IMAGE" = 1 ]; then
    step "Docker-Image bauen (linux/amd64 und linux/arm64) und nach $IMAGE schieben"
    gh auth token | docker login ghcr.io -u "$(gh api user -q .login)" --password-stdin >/dev/null
    docker buildx build --platform linux/amd64,linux/arm64 -t "$IMAGE:$VERSION" -t "$IMAGE:latest" --push .
    PUSHED=1
  else
    step "Docker-Image bauen (nur lokal, ohne Push)"
    docker buildx build --load -t "$IMAGE:$VERSION" .
  fi
fi

step "Prüfsummen und Release-Text"
( cd "$OUT" && shasum -a 256 *.dmg *.exe > SHA256SUMS )
awk -v v="## $VERSION" 'index($0, v)==1 {on=1; next} /^## / {on=0} on' CHANGELOG.md > "$OUT/notes.md"
{
  echo; echo "**Installation:** [docs/INSTALLATION.md](docs/INSTALLATION.md) · **Companion:** [docs/COMPANION.md](docs/COMPANION.md)"
  if [ "$PUSHED" = 1 ]; then echo; echo "Docker-Image: \`$IMAGE:$VERSION\`"; fi
  echo; echo "> Die Installer sind nicht signiert (siehe docs/COMPANION.md). Prüfsummen: \`SHA256SUMS\`."
  echo; echo "**macOS: App lässt sich nicht öffnen?** Nach dem Kopieren nach *Programme* im Terminal ausführen und die App dann normal starten:"
  echo; echo '```sh'; echo 'xattr -dr com.apple.quarantine "/Applications/Tickly Companion.app"'; echo '```'

} >> "$OUT/notes.md"
cat "$OUT/SHA256SUMS"

if [ "$DRY" = 1 ]; then
  echo; echo "Trockenlauf: alles gebaut nach $OUT/, nichts veröffentlicht."; exit 0
fi

step "Tag $TAG setzen, pushen und Release anlegen"
GIT_ID=(-c "user.name=$(git log -1 --format=%an)" -c "user.email=$(git log -1 --format=%ae)")
git "${GIT_ID[@]}" tag -a "$TAG" -m "Tickly $VERSION"
git -c credential.helper= -c credential.helper='!gh auth git-credential' push "https://github.com/$REPO.git" "$TAG"
gh release create "$TAG" --verify-tag --title "Tickly $VERSION" --notes-file "$OUT/notes.md" "$OUT"/*.dmg "$OUT"/*.exe "$OUT"/SHA256SUMS
echo; gh release view "$TAG" --json url -q .url
