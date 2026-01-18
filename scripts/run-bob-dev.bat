@echo off
REM Launch Harbor instance as "Bob" in dev mode
REM Uses a separate Vite port (1421) so it can run alongside Alice (1420)

set HARBOR_PROFILE=Bob
set HARBOR_DATA_DIR=%USERPROFILE%\.harbor-bob
set RUST_BACKTRACE=1
set RUST_LOG=harbor_lib=debug
set VITE_PORT=1421

echo Starting Harbor as Bob (dev mode)...
echo Data directory: %HARBOR_DATA_DIR%
echo Vite port: %VITE_PORT%
echo.

REM Run tauri dev with custom devUrl pointing to port 1421
npm run tauri dev -- --config src-tauri/tauri.bob.json

echo.
echo App exited with code %ERRORLEVEL%
pause
