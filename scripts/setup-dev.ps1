#Requires -Version 5.1
<#
.SYNOPSIS
    Harbor Development Setup Script (Windows)
.DESCRIPTION
    Installs all dependencies needed to build Harbor from source on Windows:
    - Visual Studio Build Tools (C++ workload)
    - Rust (via rustup)
    - Node.js (via winget or direct download)
    - WebView2 Runtime (usually pre-installed on Windows 10/11)
    - npm dependencies
.NOTES
    Run from an elevated (Administrator) PowerShell if you need to install
    Visual Studio Build Tools. For Rust and Node.js, regular user is fine.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step  { param($msg) Write-Host "[+] $msg" -ForegroundColor Green }
function Write-Warn  { param($msg) Write-Host "[!] $msg" -ForegroundColor Yellow }
function Write-Err   { param($msg) Write-Host "[x] $msg" -ForegroundColor Red }

$ProjectDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host ""
Write-Host "==================================" -ForegroundColor Cyan
Write-Host "  Harbor Development Setup" -ForegroundColor Cyan
Write-Host "  (Windows)" -ForegroundColor Cyan
Write-Host "==================================" -ForegroundColor Cyan
Write-Host ""

# ── 1. Check for Visual Studio Build Tools / C++ compiler ────────────

Write-Step "Checking for C++ build tools..."

$hasVS = $false
$vswherePaths = @(
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe",
    "${env:ProgramFiles}\Microsoft Visual Studio\Installer\vswhere.exe"
)

foreach ($vswhere in $vswherePaths) {
    if (Test-Path $vswhere) {
        $vsInstalls = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if ($vsInstalls) {
            $hasVS = $true
            Write-Step "Visual Studio C++ tools found: $($vsInstalls | Select-Object -First 1)"
            break
        }
    }
}

if (-not $hasVS) {
    # Check if cl.exe is on PATH anyway
    $cl = Get-Command cl.exe -ErrorAction SilentlyContinue
    if ($cl) {
        $hasVS = $true
        Write-Step "C++ compiler found at: $($cl.Source)"
    }
}

if (-not $hasVS) {
    Write-Warn "Visual Studio Build Tools with C++ workload not found."
    Write-Host ""
    Write-Host "  Option 1 (Recommended): Install Visual Studio Build Tools" -ForegroundColor White
    Write-Host "    Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/" -ForegroundColor Gray
    Write-Host "    Select 'Desktop development with C++' workload during install." -ForegroundColor Gray
    Write-Host ""
    Write-Host "  Option 2: Install full Visual Studio Community (free)" -ForegroundColor White
    Write-Host "    https://visualstudio.microsoft.com/vs/community/" -ForegroundColor Gray
    Write-Host ""

    $install = Read-Host "Attempt to install Build Tools via winget now? (y/N)"
    if ($install -eq 'y' -or $install -eq 'Y') {
        $winget = Get-Command winget -ErrorAction SilentlyContinue
        if ($winget) {
            Write-Step "Installing Visual Studio Build Tools via winget..."
            Write-Warn "This will open the Visual Studio Installer. Select 'Desktop development with C++'."
            winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
            Write-Step "Build Tools installation initiated. You may need to restart your terminal."
        } else {
            Write-Err "winget not found. Please install Build Tools manually from the URL above."
        }
    } else {
        Write-Warn "Skipping Build Tools install. Build will fail without a C++ compiler."
    }
}

# ── 2. WebView2 Runtime ──────────────────────────────────────────────

Write-Step "Checking for WebView2 Runtime..."

$webview2 = Get-ItemProperty -Path "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-E15AB5810CD8}" -ErrorAction SilentlyContinue
if (-not $webview2) {
    $webview2 = Get-ItemProperty -Path "HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BEB-E15AB5810CD8}" -ErrorAction SilentlyContinue
}

if ($webview2) {
    Write-Step "WebView2 Runtime found: $($webview2.pv)"
} else {
    Write-Warn "WebView2 Runtime not detected (pre-installed on Windows 10 21H2+ and Windows 11)."
    Write-Host "  If your app fails to launch, install from:" -ForegroundColor Gray
    Write-Host "  https://developer.microsoft.com/en-us/microsoft-edge/webview2/" -ForegroundColor Gray
}

# ── 3. Rust ──────────────────────────────────────────────────────────

Write-Step "Checking for Rust..."

$rustc = Get-Command rustc -ErrorAction SilentlyContinue
if ($rustc) {
    $rustVersion = & rustc --version
    Write-Step "Rust already installed: $rustVersion"
} else {
    Write-Step "Installing Rust via rustup..."
    Write-Host "  This will download and run the Rust installer." -ForegroundColor Gray

    $rustupInit = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit -UseBasicParsing
    & $rustupInit -y --default-toolchain stable
    Remove-Item $rustupInit -ErrorAction SilentlyContinue

    # Refresh PATH
    $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    $cargobin = Join-Path $env:USERPROFILE ".cargo\bin"
    if ($env:PATH -notlike "*$cargobin*") {
        $env:PATH = "$cargobin;$env:PATH"
    }

    $rustVersion = & rustc --version 2>$null
    if ($rustVersion) {
        Write-Step "Rust installed: $rustVersion"
    } else {
        Write-Warn "Rust installed but not on PATH yet. Restart your terminal, then re-run this script."
    }
}

# ── 4. Node.js ───────────────────────────────────────────────────────

Write-Step "Checking for Node.js..."

$node = Get-Command node -ErrorAction SilentlyContinue
if ($node) {
    $nodeVersion = & node --version
    Write-Step "Node.js already installed: $nodeVersion"
} else {
    Write-Step "Installing Node.js..."

    $winget = Get-Command winget -ErrorAction SilentlyContinue
    if ($winget) {
        Write-Step "Installing via winget..."
        winget install OpenJS.NodeJS.LTS --accept-package-agreements --accept-source-agreements
        # Refresh PATH
        $env:PATH = [System.Environment]::GetEnvironmentVariable("PATH", "User") + ";" + [System.Environment]::GetEnvironmentVariable("PATH", "Machine")
    } else {
        Write-Warn "winget not found. Please install Node.js manually:"
        Write-Host "  https://nodejs.org/" -ForegroundColor Gray
    }

    $node = Get-Command node -ErrorAction SilentlyContinue
    if ($node) {
        Write-Step "Node.js installed: $(& node --version)"
    } else {
        Write-Warn "Node.js not on PATH yet. Restart your terminal after install."
    }
}

# ── 5. npm dependencies ─────────────────────────────────────────────

Write-Step "Installing npm dependencies..."

$npm = Get-Command npm -ErrorAction SilentlyContinue
if ($npm -and (Test-Path (Join-Path $ProjectDir "package.json"))) {
    Push-Location $ProjectDir
    try {
        npm install
        Write-Step "npm dependencies installed."
    } finally {
        Pop-Location
    }
} else {
    if (-not $npm) {
        Write-Warn "npm not found — skipping. Install Node.js first, then run: npm install"
    } else {
        Write-Warn "package.json not found at $ProjectDir — skipping npm install"
    }
}

# ── Done ─────────────────────────────────────────────────────────────

Write-Host ""
Write-Host "==================================" -ForegroundColor Green
Write-Host "  Setup Complete!" -ForegroundColor Green
Write-Host "==================================" -ForegroundColor Green
Write-Host ""
Write-Host "  To build the app:        npm run tauri build" -ForegroundColor White
Write-Host "  To run in dev mode:      npm run tauri dev" -ForegroundColor White
Write-Host "  To build relay server:   cd relay-server; cargo build --release" -ForegroundColor White
Write-Host ""

if (-not $hasVS) {
    Write-Warn "REMINDER: Install Visual Studio Build Tools with C++ workload before building!"
    Write-Host ""
}
