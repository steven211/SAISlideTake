#!/usr/bin/env bash
# macOS HMR Debug entry point for SAISlideTake
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec "$SCRIPT_DIR/macos/SAISlideTake_DEBUG.sh" "$@"
