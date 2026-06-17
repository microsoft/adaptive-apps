<#
.SYNOPSIS
  Install the Adaptive Apps CLI (ada) on Windows.

.DESCRIPTION
  Downloads a release archive of `ada` from
  https://github.com/microsoft/adaptive-apps/releases, extracts the binary
  to `$AdaHome\bin`, stages the bundled Radius artifacts under
  `$AdaHome\radius`, and appends `$AdaHome\bin` to the *User* PATH
  environment variable (idempotent — skipped if already present).

  Default `$AdaHome` is `$env:USERPROFILE\.adaptive`. Override with the
  `ADA_HOME` environment variable or the `-AdaHome` parameter.

.PARAMETER Version
  Release tag to install (e.g. `cli-v0.1.0`). Defaults to the latest
  `cli-v*` release on the upstream repo.

.PARAMETER AdaHome
  Install root. Defaults to `$env:ADA_HOME` if set, otherwise
  `$env:USERPROFILE\.adaptive`.

.PARAMETER Repo
  GitHub `owner/repo` to pull from. Defaults to `microsoft/adaptive-apps`.

.PARAMETER SkipPathUpdate
  Skip appending `$AdaHome\bin` to the User PATH.

.EXAMPLE
  iwr -useb https://raw.githubusercontent.com/microsoft/adaptive-apps/main/cli/install.ps1 | iex
#>
[CmdletBinding()]
param(
  [string] $Version        = $env:ADA_VERSION,
  [string] $AdaHome        = $(if ($env:ADA_HOME) { $env:ADA_HOME } else { Join-Path $env:USERPROFILE '.adaptive' }),
  [string] $Repo           = $(if ($env:ADA_REPO) { $env:ADA_REPO } else { 'microsoft/adaptive-apps' }),
  [switch] $SkipPathUpdate
)

$ErrorActionPreference = 'Stop'

# Force a modern TLS suite; older PowerShell defaults to 1.0, which most
# GitHub-hosted release endpoints reject.
try {
  [Net.ServicePointManager]::SecurityProtocol = 'Tls12, Tls11, Tls'
} catch { }

function Write-Step($msg) { Write-Host "==> $msg" -ForegroundColor Cyan }
function Write-Warn2($msg) { Write-Host "!!  $msg" -ForegroundColor Yellow }

if ((Get-ExecutionPolicy) -gt 'RemoteSigned' -and (Get-ExecutionPolicy) -ne 'Bypass') {
  Write-Warn2 "PowerShell execution policy is '$(Get-ExecutionPolicy)'."
  Write-Warn2 "If the install fails, run: Set-ExecutionPolicy RemoteSigned -Scope CurrentUser"
}

function Resolve-Target {
  switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { return 'x86_64-pc-windows-msvc' }
    'ARM64' { return 'aarch64-pc-windows-msvc' }
    default { throw "Unsupported architecture: $($env:PROCESSOR_ARCHITECTURE)" }
  }
}

# GitHub /releases/latest ignores tag prefix; pull a page and filter
# explicitly so we never grab a non-CLI release that happens to be newer.
function Resolve-Tag {
  if ($Version) { return $Version }
  $api = "https://api.github.com/repos/$Repo/releases?per_page=30"
  $headers = @{ 'User-Agent' = 'ada-installer' }
  if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)" }
  $releases = Invoke-RestMethod -UseBasicParsing -Uri $api -Headers $headers
  $match = $releases | Where-Object { $_.tag_name -like 'cli-v*' -and $_.tag_name -notlike '*rc*' } | Select-Object -First 1
  if (-not $match) { throw "Could not resolve the latest cli-v* release from $Repo; pass -Version cli-vX.Y.Z" }
  return $match.tag_name
}

$target  = Resolve-Target
$tag     = Resolve-Tag
$ver     = $tag -replace '^cli-v',''
$asset   = "ada-$ver-$target.zip"
$url     = "https://github.com/$Repo/releases/download/$tag/$asset"

$binDir    = Join-Path $AdaHome 'bin'
$radiusDir = Join-Path $AdaHome 'radius'
$adaExe    = Join-Path $binDir 'ada.exe'

Write-Step 'Installing Adaptive Apps CLI'
Write-Host "  release  : $tag"
Write-Host "  target   : $target"
Write-Host "  asset    : $asset"
Write-Host "  home     : $AdaHome"
Write-Host "  bin dir  : $binDir"

if (Test-Path $adaExe -PathType Leaf) {
  Write-Step "Existing install detected: $adaExe"
  try { & $adaExe --version } catch { }
  Write-Step 'Reinstalling...'
}

New-Item -ItemType Directory -Force -Path $binDir, $radiusDir | Out-Null

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("ada-install-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  $archive = Join-Path $tmp $asset
  Write-Step "Downloading $url"
  $oldPp = $ProgressPreference
  $ProgressPreference = 'SilentlyContinue'
  try {
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $archive
  } finally {
    $ProgressPreference = $oldPp
  }

  Write-Step 'Extracting archive'
  Microsoft.PowerShell.Archive\Expand-Archive -Path $archive -DestinationPath $tmp -Force

  $extracted = Join-Path $tmp ("ada-$ver-$target")
  if (-not (Test-Path $extracted)) { throw "Archive layout unexpected; missing $extracted" }

  $binSrc = Join-Path $extracted 'bin\ada.exe'
  if (-not (Test-Path $binSrc)) { throw 'Archive missing bin\ada.exe' }

  Write-Step "Installing ada.exe to $binDir"
  Copy-Item -Force -Path $binSrc -Destination $adaExe

  $radiusSrc = Join-Path $extracted 'radius'
  if (Test-Path $radiusSrc) {
    Write-Step "Staging Radius artifacts under $radiusDir"
    Get-ChildItem -Path $radiusDir -Force -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force
    Copy-Item -Recurse -Force -Path (Join-Path $radiusSrc '*') -Destination $radiusDir
  } else {
    Write-Warn2 'Release archive did not bundle radius/ — skipping'
  }

  try { Write-Step "Installed $(& $adaExe --version)" } catch { Write-Step 'Installed ada' }
  Write-Host "  binary   : $adaExe"
  Write-Host "  radius   : $radiusDir"

  # --- PATH management (User scope, idempotent) -----------------------------
  if (-not $SkipPathUpdate) {
    Write-Step "Updating User PATH to include $binDir"
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries  = @()
    if ($userPath) { $entries = $userPath.Split(';') | Where-Object { $_ -ne '' } }
    $already  = $entries | Where-Object { $_.TrimEnd('\') -ieq $binDir.TrimEnd('\') }
    if ($already) {
      Write-Host "  PATH already contains $binDir — nothing to do"
    } else {
      $newPath = ($entries + $binDir) -join ';'
      [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
      # Refresh the current session so the user can invoke `ada` immediately
      # without opening a new terminal.
      $env:Path = "$binDir;$env:Path"
      Write-Host "  added $binDir to User PATH"
    }
  } else {
    Write-Warn2 "Skipping PATH update (per -SkipPathUpdate). Add '$binDir' to PATH manually."
  }

  Write-Host ''
  Write-Host 'Get started:' -ForegroundColor Green
  Write-Host '  ada --help'
  Write-Host '  ada radius path --artifact root --require-exists'
  Write-Host '  ada bootstrap --portfolio min --platform localk8s'
  Write-Host ''
}
finally {
  Remove-Item -Recurse -Force -Path $tmp -ErrorAction SilentlyContinue
}
