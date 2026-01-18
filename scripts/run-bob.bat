@echo off
REM Launch Harbor instance as "Bob" (requires built app)
REM First run: npm run tauri build
REM This uses a separate data directory so Alice and Bob have separate identities

set HARBOR_PROFILE=Bob
set HARBOR_DATA_DIR=%USERPROFILE%\.harbor-bob
set RUST_BACKTRACE=full
set RUST_LOG=harbor_lib=debug,info

echo Starting Harbor as Bob...
echo Data directory: %HARBOR_DATA_DIR%
echo.
echo NOTE: If you haven't built the app yet, run: npm run tauri build
echo.

REM Create the data directory if it doesn't exist
if not exist "%HARBOR_DATA_DIR%" mkdir "%HARBOR_DATA_DIR%"

REM Run and capture output to log file (append)
echo. >> "%HARBOR_DATA_DIR%\bob.log"
echo ============== Session started at %date% %time% ============== >> "%HARBOR_DATA_DIR%\bob.log"

if exist "src-tauri\target\release\harbor.exe" (
    echo Running release build...
    "src-tauri\target\release\harbor.exe" >> "%HARBOR_DATA_DIR%\bob.log" 2>&1
    set EXITCODE=%ERRORLEVEL%
    echo App exited with code %EXITCODE% >> "%HARBOR_DATA_DIR%\bob.log"
    echo.
    echo App exited with code %EXITCODE%
    echo Check log at: %HARBOR_DATA_DIR%\bob.log
    echo.
    echo Last 30 lines of log:
    powershell -command "Get-Content '%HARBOR_DATA_DIR%\bob.log' -Tail 30"
    pause
) else if exist "src-tauri\target\debug\harbor.exe" (
    echo Running debug build...
    "src-tauri\target\debug\harbor.exe" >> "%HARBOR_DATA_DIR%\bob.log" 2>&1
    set EXITCODE=%ERRORLEVEL%
    echo App exited with code %EXITCODE% >> "%HARBOR_DATA_DIR%\bob.log"
    echo.
    echo App exited with code %EXITCODE%
    echo Check log at: %HARBOR_DATA_DIR%\bob.log
    echo.
    echo Last 30 lines of log:
    powershell -command "Get-Content '%HARBOR_DATA_DIR%\bob.log' -Tail 30"
    pause
) else (
    echo ERROR: No built executable found. Please run: npm run tauri build
    pause
)
