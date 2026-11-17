@echo off
REM Unified Windows AI Debug / HMR entry point for SAISlideTake.
call "%~dp0win\SAISlideTake_DEBUG.bat" %*
exit /b %ERRORLEVEL%
