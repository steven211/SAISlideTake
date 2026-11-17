#!/usr/bin/env bash
# SAISlideTake macOS 免簽名可攜式啟動腳本 (版本目錄內部)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_PATH="$SCRIPT_DIR/SAISlideTake.app"

if [ ! -d "$APP_PATH" ]; then
    echo -e "\033[0;31m[ERROR] 找不到應用程式: $APP_PATH\033[0m" >&2
    exit 1
fi

# 自動尋找與綁定同層級 SAISlideTake_Data
ROOT_DATA="$(cd "$SCRIPT_DIR/../../" && pwd)/SAISlideTake_Data"
if [ -d "$ROOT_DATA" ] || [ -f "$SCRIPT_DIR/../../SAISlideTake.json" ]; then
    mkdir -p "$ROOT_DATA"
    export SAIST_PRESENTATION_DIR="$ROOT_DATA"
    echo -e "\033[0;36m[DATA] 綁定可攜式共用資料庫: $ROOT_DATA\033[0m"
fi

echo -e "\033[0;36m[LAUNCH] 正在清除 quarantine 隔離屬性 (xattr -cr)...\033[0m"
xattr -cr "$APP_PATH" 2>/dev/null || true

echo -e "\033[0;32m[LAUNCH] 正在啟動 SAISlideTake...\033[0m"
open "$APP_PATH"
