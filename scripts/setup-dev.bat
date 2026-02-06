@echo off
REM Harbor Development Setup (Windows)
REM This launches the PowerShell setup script.
REM If you see an execution policy error, run:
REM   Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned

echo.
echo  Harbor Development Setup
echo  ========================
echo.
echo  Launching PowerShell setup script...
echo.

powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0setup-dev.ps1"

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo  Setup encountered an error. See output above.
    echo.
)

pause
