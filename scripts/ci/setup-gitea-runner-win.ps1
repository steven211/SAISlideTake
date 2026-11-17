<#
.SYNOPSIS
    SAISlideTake - Windows MiniPC Gitea act_runner 安裝與註冊腳本
.EXAMPLE
    .\scripts\ci\setup-gitea-runner-win.ps1 -Token "<GITEA_RUNNER_TOKEN>"
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Token,
    [string]$GiteaUrl = "http://github.com",
    [string]$RunnerVersion = "0.2.11"
)

$ErrorActionPreference = "Stop"
$RunnerDir = Join-Path $env:USERPROFILE ".gitea-runner"
$RunnerExe = Join-Path $RunnerDir "act_runner.exe"
$RunnerName = "$($env:COMPUTERNAME)-win"
$RunnerLabels = "windows-host:host"

New-Item -ItemType Directory -Path $RunnerDir -Force | Out-Null
Set-Location $RunnerDir

if (-not (Test-Path $RunnerExe)) {
    $DownloadUrl = "https://gitea.com/gitea/act_runner/releases/download/v$RunnerVersion/act_runner-$RunnerVersion-windows-amd64.exe"
    Write-Host "[INFO] 正在下載 act_runner v$RunnerVersion..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $RunnerExe
}

$RegFile = Join-Path $RunnerDir ".runner"
if (-not (Test-Path $RegFile)) {
    Write-Host "[INFO] 註冊 Runner ($RunnerName, labels=$RunnerLabels) 至 $GiteaUrl..." -ForegroundColor Cyan
    & $RunnerExe register `
        --no-interactive `
        --instance $GiteaUrl `
        --token $Token `
        --name $RunnerName `
        --labels $RunnerLabels
} else {
    Write-Host "[INFO] 偵測到既有的 .runner 設定，跳過註冊步驟。" -ForegroundColor Yellow
}

Write-Host "[INFO] 啟動 act_runner daemon..." -ForegroundColor Green
& $RunnerExe daemon
