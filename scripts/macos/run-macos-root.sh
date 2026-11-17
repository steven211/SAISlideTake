#!/usr/bin/env bash
# SAISlideTake 產品根目錄可攜式啟動器 (Portable Root Launcher)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. 自動定位並綁定同層級共用資料目錄
DATA_DIR="$SCRIPT_DIR/SAISlideTake_Data"
mkdir -p "$DATA_DIR"
export SAIST_PRESENTATION_DIR="$DATA_DIR"

# 2. 自動尋找最新版本之 SAISlideTake 目錄（嚴格支援含空白字元之目錄與路徑）
LATEST_APP=""
while IFS= read -r ver_dir; do
    if [ -n "$ver_dir" ] && [ -d "$ver_dir/mac/SAISlideTake.app" ]; then
        LATEST_APP="$ver_dir/mac/SAISlideTake.app"
        break
    fi
done < <(find "$SCRIPT_DIR" -maxdepth 1 -mindepth 1 -type d -name "SAISlideTake*" | sort -V -r)

if [ -z "$LATEST_APP" ]; then
    echo -e "\033[0;31m[ERROR] 找不到 SAISlideTake.app，請確認已安裝版本目錄。\033[0m" >&2
    exit 1
fi

echo -e "\033[0;36m=== SAISlideTake Portable Suite ===\033[0m"
echo -e "\033[0;36m[PORTABLE] 產品根目錄:   $SCRIPT_DIR\033[0m"
echo -e "\033[0;36m[DATA]     共用資料庫:   $DATA_DIR\033[0m"
echo -e "\033[0;32m[APP]      執行應用程式: $LATEST_APP\033[0m"

xattr -cr "$LATEST_APP" 2>/dev/null || true
open "$LATEST_APP"
