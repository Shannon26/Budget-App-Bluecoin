#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DB="/tmp/opencode/paisa-e2e.db"
API_LOG="/tmp/opencode/paisa-e2e-api.log"
WEB_LOG="/tmp/opencode/paisa-e2e-web.log"

export PATH="$HOME/.cargo/bin:$PATH"
rm -f "$DB" "$DB-wal" "$DB-shm"

cargo build --manifest-path "$ROOT/backend/Cargo.toml"
DATABASE_URL="sqlite://$DB?mode=rwc" "$ROOT/backend/target/debug/budget-backend" >"$API_LOG" 2>&1 &
API_PID=$!
(
  cd "$ROOT/frontend"
  exec ./node_modules/.bin/vite --host 127.0.0.1
) >"$WEB_LOG" 2>&1 &
WEB_PID=$!

cleanup() {
  kill "$API_PID" "$WEB_PID" 2>/dev/null || true
  wait "$API_PID" "$WEB_PID" 2>/dev/null || true
}
trap cleanup EXIT

curl -fsS --retry 30 --retry-connrefused --retry-delay 1 http://127.0.0.1:3000/api/health >/dev/null
curl -fsS --retry 30 --retry-connrefused --retry-delay 1 http://127.0.0.1:5173/ >/dev/null

cd "$ROOT/frontend"
npx playwright test
