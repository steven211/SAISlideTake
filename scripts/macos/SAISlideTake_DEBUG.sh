#!/usr/bin/env bash
# SAISlideTake HMR Debug Launcher for macOS
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
FRONTEND_ROOT="$PROJECT_ROOT/src/saist-web"
PORTAL_MANIFEST="$PROJECT_ROOT/src/saist-portal/Cargo.toml"
VITE_LOG="$PROJECT_ROOT/9.output/vite-dev.log"

echo -e "\033[0;36m=== SAISlideTake macOS HMR Debug Launcher ===\033[0m"
echo "[DIR] Project Root: $PROJECT_ROOT"
mkdir -p "$PROJECT_ROOT/9.output"
cd "$PROJECT_ROOT"

if [ ! -d "$FRONTEND_ROOT/node_modules" ]; then
    (cd "$FRONTEND_ROOT" && npm install)
fi

(cd "$FRONTEND_ROOT" && npm run build)
npm run dev --prefix "$FRONTEND_ROOT" > "$VITE_LOG" 2>&1 &
VITE_PID=$!
cleanup() { kill "$VITE_PID" 2>/dev/null || true; }
trap cleanup EXIT INT TERM

for _ in $(seq 1 50); do
    if curl --silent --fail http://127.0.0.1:1425/ >/dev/null; then break; fi
    sleep 0.2
done
if ! curl --silent --fail http://127.0.0.1:1425/ >/dev/null; then
    cat "$VITE_LOG"
    exit 1
fi

export CARGO_TARGET_DIR="$PROJECT_ROOT/9.output/target"
cargo run --manifest-path "$PORTAL_MANIFEST" -- "$@"
