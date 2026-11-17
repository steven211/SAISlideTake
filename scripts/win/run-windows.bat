@echo off
setlocal
cd /d "%~dp0"

set "ROOT_DATA=%~dp0..\..\SAISlideTake_Data"
if exist "%ROOT_DATA%" (
    set "SAIST_PRESENTATION_DIR=%ROOT_DATA%"
) else if exist "%~dp0..\..\SAISlideTake.json" (
    mkdir "%ROOT_DATA%" 2>nul
    set "SAIST_PRESENTATION_DIR=%ROOT_DATA%"
)

if not exist "SAISlideTake.exe" (
    echo [ERROR] 找不到 SAISlideTake.exe
    pause
    exit /b 1
)

start "" "SAISlideTake.exe" %*
