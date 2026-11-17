#!/usr/bin/env bash
# Build and assemble a Portable Single Release macOS DeskAPP package for SAISlideTake
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo -e "\033[0;36m=== SAISlideTake macOS Single Release DeskAPP Build ===\033[0m"
cd "$PROJECT_ROOT"

# 1. 前端生產環境資產建置 (供 Rust 編譯期內嵌)
if [ ! -d "$PROJECT_ROOT/src/saist-web/node_modules" ]; then
    echo "[BUILD] Installing web dependencies..."
    (cd "$PROJECT_ROOT/src/saist-web" && npm install)
fi
echo "[BUILD] Building web front assets..."
(cd "$PROJECT_ROOT/src/saist-web" && npm run build)

# 2. Rust Release 二進位檔編譯與官方 App Bundle 打包
echo "[BUILD] Compiling & Bundling Tauri Release App..."
TAURI_CLI="$PROJECT_ROOT/src/saist-web/node_modules/.bin/tauri"
export CARGO_TARGET_DIR="$PROJECT_ROOT/9.output/target"
(cd "$PROJECT_ROOT/src/saist-portal" && "$TAURI_CLI" build --bundles app --no-sign)

OFFICIAL_APP="$PROJECT_ROOT/9.output/target/release/bundle/macos/SAISlideTake.app"
if [ ! -d "$OFFICIAL_APP" ]; then
    echo "[ERROR] Official Tauri App bundle not found at: $OFFICIAL_APP" >&2
    exit 1
fi
echo -e "\033[0;32m[SUCCESS] Official Release App Bundle created at: $OFFICIAL_APP\033[0m"

# 3. 取得版本號
VERSION=$(grep '^version = ' "$PROJECT_ROOT/src/saist-portal/Cargo.toml" | head -n 1 | cut -d '"' -f 2)
VERSION="${VERSION:-0.1.0}"

# 4. 組裝產品根目錄發行結構
# 結構：
# 9.output/release/SAISlideTake/
# ├── SAISlideTake <version>/mac/SAISlideTake.app
# ├── SAISlideTake <version>/mac/run-macos.sh
# ├── SAISlideTake_Data/
# ├── SAISlideTake.json
# ├── run-macos.sh
# └── run-windows.bat

RELEASE_PRODUCT_ROOT="$PROJECT_ROOT/9.output/release/SAISlideTake"
VERSION_DIR="$RELEASE_PRODUCT_ROOT/SAISlideTake $VERSION"
MAC_OUTPUT="$VERSION_DIR/mac"
BUNDLE_APP="$MAC_OUTPUT/SAISlideTake.app"

echo "[STAGE] Assembling Release Product at: $RELEASE_PRODUCT_ROOT..."
mkdir -p "$MAC_OUTPUT"
rm -rf "$BUNDLE_APP"
cp -R "$OFFICIAL_APP" "$BUNDLE_APP"
codesign -s - --force "$BUNDLE_APP" 2>/dev/null || true

# 複製版本內部啟動器
cp "$PROJECT_ROOT/scripts/macos/run-macos.sh" "$MAC_OUTPUT/run-macos.sh"
chmod +x "$MAC_OUTPUT/run-macos.sh"

# 確保共用資料目錄存在（不覆蓋已有資料）
mkdir -p "$RELEASE_PRODUCT_ROOT/SAISlideTake_Data"

# 複製產品根目錄設定檔與啟動器
cp "$PROJECT_ROOT/SAISlideTake.json" "$RELEASE_PRODUCT_ROOT/SAISlideTake.json"
cp "$PROJECT_ROOT/scripts/macos/run-macos-root.sh" "$RELEASE_PRODUCT_ROOT/run-macos.sh"
chmod +x "$RELEASE_PRODUCT_ROOT/run-macos.sh"
cp "$PROJECT_ROOT/scripts/win/run-windows-root.bat" "$RELEASE_PRODUCT_ROOT/run-windows.bat"

echo -e "\033[0;32m[SUCCESS] Portable DeskAPP packaged successfully at: $RELEASE_PRODUCT_ROOT\033[0m"
echo -e "\033[0;32m  - App:     $BUNDLE_APP\033[0m"
echo -e "\033[0;32m  - Data:    $RELEASE_PRODUCT_ROOT/SAISlideTake_Data\033[0m"
echo -e "\033[0;32m  - Launch:  $RELEASE_PRODUCT_ROOT/run-macos.sh\033[0m"

# 5. 同步部署至本地發布庫 (ReleaseAPP / Release)
LOGIN_USER="$(/usr/bin/id -un)"
ACCOUNT_HOME="$(/usr/bin/dscl /Search -read "/Users/$LOGIN_USER" NFSHomeDirectory 2>/dev/null | /usr/bin/sed -n 's/^NFSHomeDirectory: //p' || true)"
ACCOUNT_HOME="${ACCOUNT_HOME:-/Users/$LOGIN_USER}"

for target_base in "$ACCOUNT_HOME/ReleaseAPP" "$ACCOUNT_HOME/Release"; do
    if [ -d "$target_base" ]; then
        DEPLOY_PRODUCT="$target_base/SAISlideTake"
        DEPLOY_VERSION="$DEPLOY_PRODUCT/SAISlideTake $VERSION/mac"
        echo "[DEPLOY] Publishing to $DEPLOY_PRODUCT..."
        mkdir -p "$DEPLOY_VERSION"
        rm -rf "$DEPLOY_VERSION/SAISlideTake.app" "$DEPLOY_VERSION/run-macos.sh"
        cp -R "$BUNDLE_APP" "$DEPLOY_VERSION/SAISlideTake.app"
        cp "$MAC_OUTPUT/run-macos.sh" "$DEPLOY_VERSION/run-macos.sh"
        chmod +x "$DEPLOY_VERSION/run-macos.sh"

        # 確保共用資料目錄存在（不覆蓋使用者現有資料）
        mkdir -p "$DEPLOY_PRODUCT/SAISlideTake_Data"

        # 複製根目錄啟動器與設定檔
        cp "$RELEASE_PRODUCT_ROOT/SAISlideTake.json" "$DEPLOY_PRODUCT/SAISlideTake.json"
        cp "$RELEASE_PRODUCT_ROOT/run-macos.sh" "$DEPLOY_PRODUCT/run-macos.sh"
        chmod +x "$DEPLOY_PRODUCT/run-macos.sh"
        cp "$RELEASE_PRODUCT_ROOT/run-windows.bat" "$DEPLOY_PRODUCT/run-windows.bat"

        echo -e "\033[0;32m[SUCCESS] Deployed to $DEPLOY_PRODUCT\033[0m"
    fi
done
