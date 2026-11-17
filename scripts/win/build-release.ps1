$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path "$PSScriptRoot\..\..").Path
$FrontendRoot = Join-Path $ProjectRoot "src\saist-web"
$PortalRoot = Join-Path $ProjectRoot "src\saist-portal"

Write-Host "=== SAISlideTake Windows Single Release DeskAPP Build ===" -ForegroundColor Cyan
Push-Location $ProjectRoot
try {
    # 1. 前端靜態資源建置 (供 Rust 編譯期內嵌)
    if (-not (Test-Path (Join-Path $FrontendRoot "node_modules"))) {
        Write-Host "[BUILD] Installing web dependencies..." -ForegroundColor Gray
        Push-Location $FrontendRoot; npm install; Pop-Location
    }
    Write-Host "[BUILD] Building web front assets..." -ForegroundColor Gray
    Push-Location $FrontendRoot; npm run build; Pop-Location

    # 2. Rust Release 二進位檔編譯 (透過 Tauri CLI 啟用 custom-protocol 並內嵌 Teal icon.ico 與 Web dist)
    Write-Host "[BUILD] Compiling Rust release binary via Tauri CLI..." -ForegroundColor Gray
    $TauriCli = Join-Path $FrontendRoot "node_modules\.bin\tauri.cmd"
    if (-not (Test-Path $TauriCli)) {
        throw "Tauri CLI not found at: $TauriCli"
    }
    $env:CARGO_TARGET_DIR = Join-Path $ProjectRoot "9.output\target"
    Push-Location $PortalRoot
    try {
        & $TauriCli build --no-bundle
        if ($LASTEXITCODE -ne 0) { throw "Tauri release build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }

    # 3. 讀取版本號
    $Version = "0.1.0"
    $CargoToml = Get-Content (Join-Path $PortalRoot "Cargo.toml") -Raw
    if ($CargoToml -match 'version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    }

    # 4. 組裝產品根目錄發行結構
    # 結構：
    # 9.output\release\SAISlideTake\
    # ├── SAISlideTake <version>\win\SAISlideTake.exe
    # ├── SAISlideTake <version>\win\run-windows.bat
    # ├── SAISlideTake_Data\
    # ├── SAISlideTake.json
    # ├── run-windows.bat
    # └── run-macos.sh

    $ReleaseProductRoot = Join-Path $ProjectRoot "9.output\release\SAISlideTake"
    $VersionDir = Join-Path $ReleaseProductRoot "SAISlideTake $Version"
    $WinOutput = Join-Path $VersionDir "win"

    New-Item -ItemType Directory -Path $WinOutput -Force | Out-Null

    $SourceExe = Join-Path $ProjectRoot "9.output\target\release\SAISlideTake.exe"
    if (-not (Test-Path $SourceExe)) {
        throw "Build failed: $SourceExe not found."
    }
    Copy-Item $SourceExe (Join-Path $WinOutput "SAISlideTake.exe") -Force

    $WinLauncher = Join-Path $ProjectRoot "scripts\win\run-windows.bat"
    if (Test-Path $WinLauncher) {
        Copy-Item $WinLauncher (Join-Path $WinOutput "run-windows.bat") -Force
    }

    # 確保共用資料目錄存在（不覆蓋已有資料）
    $DataDir = Join-Path $ReleaseProductRoot "SAISlideTake_Data"
    if (-not (Test-Path $DataDir)) {
        New-Item -ItemType Directory -Path $DataDir -Force | Out-Null
    }

    # 複製產品根目錄設定與啟動器
    Copy-Item (Join-Path $ProjectRoot "SAISlideTake.json") (Join-Path $ReleaseProductRoot "SAISlideTake.json") -Force
    $RootWinLauncher = Join-Path $ProjectRoot "scripts\win\run-windows-root.bat"
    if (Test-Path $RootWinLauncher) {
        Copy-Item $RootWinLauncher (Join-Path $ReleaseProductRoot "run-windows.bat") -Force
    }
    $RootMacLauncher = Join-Path $ProjectRoot "scripts\macos\run-macos-root.sh"
    if (Test-Path $RootMacLauncher) {
        Copy-Item $RootMacLauncher (Join-Path $ReleaseProductRoot "run-macos.sh") -Force
    }

    Write-Host "[SUCCESS] Portable DeskAPP packaged successfully at: $ReleaseProductRoot" -ForegroundColor Green
    Write-Host "  - Executable: $(Join-Path $WinOutput 'SAISlideTake.exe')" -ForegroundColor Green
    Write-Host "  - Data:       $DataDir" -ForegroundColor Green
    Write-Host "  - Launcher:   $(Join-Path $ReleaseProductRoot 'run-windows.bat')" -ForegroundColor Green

    # 5. 自動同步部署至本機發布庫 (ReleaseAPP / Release)
    $ReleaseBases = @(
        "R:\ReleaseAPP",
        "R:\Release",
        "G:\ReleaseAPP",
        "G:\Release",
        (Join-Path $env:USERPROFILE "ReleaseAPP"),
        (Join-Path $env:USERPROFILE "Release")
    )

    foreach ($Base in $ReleaseBases) {
        if (Test-Path $Base) {
            $TargetProduct = Join-Path $Base "SAISlideTake"
            $TargetVersion = Join-Path $TargetProduct "SAISlideTake $Version\win"
            Write-Host "[DEPLOY] Publishing to $TargetProduct..." -ForegroundColor Cyan
            New-Item -ItemType Directory -Path $TargetVersion -Force | Out-Null
            Copy-Item (Join-Path $WinOutput "*") $TargetVersion -Recurse -Force

            $TargetData = Join-Path $TargetProduct "SAISlideTake_Data"
            if (-not (Test-Path $TargetData)) {
                New-Item -ItemType Directory -Path $TargetData -Force | Out-Null
            }
            Copy-Item (Join-Path $ReleaseProductRoot "SAISlideTake.json") (Join-Path $TargetProduct "SAISlideTake.json") -Force
            if (Test-Path (Join-Path $ReleaseProductRoot "run-windows.bat")) {
                Copy-Item (Join-Path $ReleaseProductRoot "run-windows.bat") (Join-Path $TargetProduct "run-windows.bat") -Force
            }
            if (Test-Path (Join-Path $ReleaseProductRoot "run-macos.sh")) {
                Copy-Item (Join-Path $ReleaseProductRoot "run-macos.sh") (Join-Path $TargetProduct "run-macos.sh") -Force
            }
            Write-Host "[SUCCESS] Deployed to $TargetProduct" -ForegroundColor Green
        }
    }
} finally {
    Pop-Location
}
