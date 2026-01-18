@echo off
REM Launch Harbor instance as "Alice" (dev mode)
REM This uses the default data directory

set HARBOR_PROFILE=Alice
set HARBOR_DATA_DIR=%USERPROFILE%\.harbor-alice

echo Starting Harbor as Alice (dev mode)...
echo Data directory: %HARBOR_DATA_DIR%
echo.

npm run tauri dev
