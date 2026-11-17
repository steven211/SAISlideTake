@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

REM 1. 自動定位並綁定同層級共用資料目錄
set "DATA_DIR=%~dp0SAISlideTake_Data"
if not exist "%DATA_DIR%" mkdir "%DATA_DIR%"
set "SAIST_PRESENTATION_DIR=%DATA_DIR%"

REM 2. 自動尋找最新版本之 SAISlideTake 目錄
set "TARGET_EXE="
for /f "delims=" %%D in ('dir /b /ad /o-n "SAISlideTake*" 2^>nul') do (
    if exist "%~dp0%%D\win\SAISlideTake.exe" (
        set "TARGET_EXE=%~dp0%%D\win\SAISlideTake.exe"
        goto :found
    )
)

:found
if not defined TARGET_EXE (
    echo [ERROR] 找不到 SAISlideTake.exe，請確認已安裝版本目錄。
    pause
    exit /b 1
)

echo === SAISlideTake Portable Suite ===
echo [PORTABLE] 產品根目錄:   %~dp0
echo [DATA]     共用資料庫:   %DATA_DIR%
echo [APP]      執行應用程式: %TARGET_EXE%
start "" "%TARGET_EXE%" %*
