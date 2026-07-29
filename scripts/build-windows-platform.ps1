param(
  [Parameter(Mandatory = $true)][string]$RepositoryUrl,
  [Parameter(Mandatory = $true)][ValidatePattern('^[0-9a-fA-F]{40}$')][string]$Commit,
  [Parameter(Mandatory = $true)][string]$Worktree,
  [Parameter(Mandatory = $true)][string]$Output
)

$ErrorActionPreference = 'Stop'

function Invoke-Checked {
  param(
    [Parameter(Mandatory = $true)][string]$Description,
    [Parameter(Mandatory = $true)][scriptblock]$Command
  )

  & $Command
  if ($LASTEXITCODE -ne 0) {
    throw "$Description failed with exit code $LASTEXITCODE"
  }
}

foreach ($command in @('git', 'node', 'pnpm', 'cargo')) {
  if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
    throw "Required command is unavailable on Windows: $command"
  }
}

$worktreeParent = Split-Path -Parent $Worktree
New-Item -ItemType Directory -Force -Path $worktreeParent, $Output | Out-Null

if (-not (Test-Path (Join-Path $Worktree '.git'))) {
  if ((Test-Path $Worktree) -and (Get-ChildItem -Force $Worktree | Select-Object -First 1)) {
    throw "Build checkout exists but is not an empty Git repository: $Worktree"
  }
  Invoke-Checked 'Git clone' { git clone --filter=blob:none --no-checkout $RepositoryUrl $Worktree }
}

$checkoutOrigin = (git -C $Worktree remote get-url origin).Trim()
if ($LASTEXITCODE -ne 0) {
  throw 'Unable to read the Windows build checkout origin'
}
if ($checkoutOrigin -ne $RepositoryUrl) {
  throw "Build checkout origin mismatch: expected $RepositoryUrl, found $checkoutOrigin"
}

Invoke-Checked 'Git fetch' { git -C $Worktree fetch --no-tags --prune origin $Commit }
Invoke-Checked 'Git checkout' { git -C $Worktree checkout --detach --force FETCH_HEAD }
Invoke-Checked 'Git reset' { git -C $Worktree reset --hard $Commit }

$actualCommit = (git -C $Worktree rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $actualCommit -ne $Commit) {
  throw "Windows checkout resolved to $actualCommit instead of $Commit"
}

Push-Location $Worktree
try {
  $env:VITE_HARBOR_RELAY_NAMESPACE = 'harbor.social'
  Invoke-Checked 'pnpm install' { pnpm install --frozen-lockfile }
  Invoke-Checked 'Tauri build' { pnpm exec tauri build --no-bundle }
} finally {
  Pop-Location
}

$harborBinary = Join-Path $Worktree 'src-tauri\target\release\harbor.exe'
$harborctlBinary = Join-Path $Worktree 'src-tauri\target\release\harborctl.exe'
foreach ($binary in @($harborBinary, $harborctlBinary)) {
  if (-not (Test-Path $binary -PathType Leaf)) {
    throw "Expected Windows build output is missing: $binary"
  }
}

$outputHarbor = Join-Path $Output 'harbor.exe'
$outputHarborctl = Join-Path $Output 'harborctl.exe'
Copy-Item -Force $harborBinary $outputHarbor
Copy-Item -Force $harborctlBinary $outputHarborctl

$version = (Get-Content (Join-Path $Worktree 'package.json') -Raw | ConvertFrom-Json).version
$hashLines = @(
  "{0}  harbor.exe" -f (Get-FileHash -Algorithm SHA256 $outputHarbor).Hash.ToLowerInvariant()
  "{0}  harborctl.exe" -f (Get-FileHash -Algorithm SHA256 $outputHarborctl).Hash.ToLowerInvariant()
)
$hashLines | Set-Content -Encoding ASCII (Join-Path $Output 'SHA256SUMS')

@(
  'platform=windows-x86_64'
  'architecture=x86_64'
  "commit=$Commit"
  "version=$version"
  "built_at_utc=$([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))"
) | Set-Content -Encoding ASCII (Join-Path $Output 'build-info.txt')

Write-Host "Built windows-x86_64 at commit $Commit"
Write-Host "Artifacts: $Output"
