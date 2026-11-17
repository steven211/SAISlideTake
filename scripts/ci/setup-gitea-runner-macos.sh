#!/usr/bin/env bash
# ==============================================================================
# SAISlideTake - Mac mini Gitea act_runner 安裝與註冊腳本
# 用法: ./scripts/ci/setup-gitea-runner-macos.sh <RUNNER_TOKEN> [GITEA_URL]
# ==============================================================================
set -euo pipefail

RUNNER_TOKEN="${1:-}"
GITEA_URL="${2:-http://github.com}"
RUNNER_DIR="$HOME/.gitea-runner"
RUNNER_NAME="$(hostname -s)-macos"
RUNNER_LABELS="macos-host:host"

if [ -z "$RUNNER_TOKEN" ]; then
    echo "用法: $0 <GITEA_RUNNER_TOKEN> [GITEA_URL]" >&2
    echo "請先至 Gitea -> 專案設定(或管理後台) -> Actions -> Runners 取得 Registration Token。" >&2
    exit 1
fi

mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

if ! command -v act_runner >/dev/null 2>&1; then
    if command -v brew >/dev/null 2>&1; then
        echo "[INFO] 透過 Homebrew 安裝 act_runner..."
        brew install act_runner
    else
        echo "[ERROR] 未找到 act_runner 與 brew，請先安裝 Homebrew 或手動下載 act_runner。" >&2
        exit 1
    fi
fi

if [ ! -f "$RUNNER_DIR/.runner" ]; then
    echo "[INFO] 註冊 Runner ($RUNNER_NAME, labels=$RUNNER_LABELS) 至 $GITEA_URL..."
    act_runner register \
        --no-interactive \
        --instance "$GITEA_URL" \
        --token "$RUNNER_TOKEN" \
        --name "$RUNNER_NAME" \
        --labels "$RUNNER_LABELS"
else
    echo "[INFO] 偵測到既有的 $RUNNER_DIR/.runner，跳過註冊步驟。"
fi

echo "[INFO] 啟動 act_runner daemon（Ctrl+C 可停止，或改用 brew services / launchd 常駐）..."
exec act_runner daemon
