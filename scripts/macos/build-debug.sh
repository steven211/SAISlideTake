#!/usr/bin/env bash
# Build debug binary for macOS
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "[BUILD] Building SAISlideTake macOS Debug Binary..."
cd "$PROJECT_ROOT"
if [ ! -d "$PROJECT_ROOT/src/saist-web/node_modules" ]; then
    (cd "$PROJECT_ROOT/src/saist-web" && npm install)
fi
(cd "$PROJECT_ROOT/src/saist-web" && npm run build)
export CARGO_TARGET_DIR="$PROJECT_ROOT/9.output/target"
cargo build --manifest-path "$PROJECT_ROOT/src/saist-portal/Cargo.toml"
echo -e "\033[0;32m[SUCCESS] Debug Binary built at: $PROJECT_ROOT/9.output/target/debug/SAISlideTake\033[0m"
