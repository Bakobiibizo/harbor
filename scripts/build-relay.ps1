# Build the relay server release binary and update the SHA256 hash file

$ErrorActionPreference = "Stop"

$ProjectDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$RelayDir = Join-Path $ProjectDir "relay-server"
$BinDir = Join-Path $RelayDir "bin"

Write-Host "[+] Building relay server (release)..." -ForegroundColor Green
cargo build --release --manifest-path (Join-Path $RelayDir "Cargo.toml")

Write-Host "[+] Copying binary to $BinDir..." -ForegroundColor Green
if (-not (Test-Path $BinDir)) { New-Item -ItemType Directory -Path $BinDir | Out-Null }
Copy-Item (Join-Path $RelayDir "target\release\harbor-relay.exe") (Join-Path $BinDir "harbor-relay.exe") -Force

Write-Host "[+] Computing SHA256..." -ForegroundColor Green
$hash = (Get-FileHash (Join-Path $BinDir "harbor-relay.exe") -Algorithm SHA256).Hash.ToLower()
"$hash  relay-server/bin/harbor-relay.exe" | Set-Content (Join-Path $BinDir "harbor-relay.sha256") -NoNewline

Write-Host "[+] Done." -ForegroundColor Green
Write-Host "    Binary: $BinDir\harbor-relay.exe"
Write-Host "    SHA256: $hash"
