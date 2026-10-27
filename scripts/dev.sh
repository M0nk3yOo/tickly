#!/usr/bin/env bash
# Startet eine lokale PostgreSQL-Instanz (Port 5433, Daten in .pgdata) und den Server.
# Demo-Modus (Standard): TICKLY_DEMO=1. Echtbetrieb: TICKLY_DEMO=0 ADMIN_EMAIL=… ADMIN_PASSWORD=… scripts/dev.sh
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
PG_BIN=${PG_BIN:-$( (ls -d /opt/homebrew/opt/postgresql@*/bin /usr/local/opt/postgresql@*/bin /usr/lib/postgresql/*/bin 2>/dev/null || true) | tail -1)}
export PATH="$PG_BIN:$PATH"
[ -d .pgdata ] || initdb -D .pgdata -U tickly --auth=trust -E UTF8 >/dev/null
pg_isready -h localhost -p 5433 >/dev/null 2>&1 || pg_ctl -D .pgdata -o "-p 5433 -k /tmp" -l .pgdata/server.log start >/dev/null
createdb -h localhost -p 5433 -U tickly "${DB_NAME:-tickly}" 2>/dev/null || true
export DATABASE_URL="postgres://tickly@localhost:5433/${DB_NAME:-tickly}"
export TICKLY_DEMO="${TICKLY_DEMO:-1}"
exec cargo run -q -p tickly-api
