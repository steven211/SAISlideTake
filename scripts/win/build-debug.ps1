$ErrorActionPreference = "Stop"
$ProjectRoot = (Resolve-Path "$PSScriptRoot\..\..").Path
$FrontendRoot = Join-Path $ProjectRoot "src\saist-web"
Write-Host "[START] Building and starting SAISlideTake Windows Debug App..." -ForegroundColor Cyan
Push-Location $ProjectRoot
$ViteProcess = $null
try {
    if (-not (Test-Path (Join-Path $FrontendRoot "node_modules"))) {
        Push-Location $FrontendRoot
        npm install
        if ($LASTEXITCODE -ne 0) { throw "npm install failed" }
        Pop-Location
    }
    Push-Location $FrontendRoot
    npm run build
    if ($LASTEXITCODE -ne 0) { throw "Web build failed" }
    Pop-Location
    New-Item -ItemType Directory -Force (Join-Path $ProjectRoot "9.output") | Out-Null
    $ViteLog = Join-Path $ProjectRoot "9.output\vite-dev.log"
    $ViteProcess = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", "npm run dev" -WorkingDirectory $FrontendRoot -PassThru -NoNewWindow -RedirectStandardOutput $ViteLog -RedirectStandardError "$ViteLog.err"
    $ViteReady = $false
    for ($Attempt = 0; $Attempt -lt 50; $Attempt++) {
        try { Invoke-WebRequest "http://127.0.0.1:1425/" -TimeoutSec 1 | Out-Null; $ViteReady = $true; break } catch { Start-Sleep -Milliseconds 200 }
    }
    if (-not $ViteReady) { throw "Vite did not become ready; see $ViteLog" }
    $env:CARGO_TARGET_DIR = Join-Path $ProjectRoot "9.output\target"
    cargo run --manifest-path "$ProjectRoot\src\saist-portal\Cargo.toml"
    if ($LASTEXITCODE -ne 0) { throw "SAISlideTake exited with code $LASTEXITCODE" }
} finally {
    if ($ViteProcess -and -not $ViteProcess.HasExited) { Stop-Process -Id $ViteProcess.Id -Force }
    Pop-Location
}
