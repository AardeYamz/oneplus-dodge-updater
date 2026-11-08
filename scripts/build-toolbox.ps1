<#
.SYNOPSIS
  Builds the Dodge Toolbox exe (Rust + Tauri, uses the WebView2 runtime that ships with Windows 10/11).
  Cargo builds on the local disk; the finished exe is copied to the repo root, where the updater lives.
#>
param(
    [string]$Store = 'X:\Android Development\Devices\Dodge',
    # Quicker build (no LTO, fewer optimizations), slightly larger exe. For local testing.
    [switch]$Fast,
    # Pin the newest published yl-theme version in toolbox\theme.json before building.
    [switch]$BumpTheme
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$app  = Join-Path $root 'toolbox'
# Build on the local disk: the target tree is thousands of small files, slow on X:.
$out  = Join-Path $env:LOCALAPPDATA 'dodge-toolbox-target'

if (-not (Test-Path -LiteralPath $Store)) { throw "Store folder not found: $Store (is X: connected?)" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'cargo not found: install Rust from https://rustup.rs' }

if ($BumpTheme) {
    $base = 'https://theme.yannislam.org'
    $versions = Invoke-RestMethod "$base/versions.json"
    $latest = $versions.PSObject.Properties.Name | Sort-Object | Select-Object -Last 1  # v + UTC timestamp sorts as text
    $files = 'yl-theme.css', 'yl-theme.js', 'favicon.svg' | ForEach-Object { "    `"$_`": `"$($versions.$latest.$_)`"" }
    $json = "{`n  `"version`": `"$latest`",`n  `"files`": {`n$($files -join ",`n")`n  }`n}`n"
    [IO.File]::WriteAllText((Join-Path $app 'theme.json'), $json)
    Write-Host "pinned $latest"
}

# build.rs fetches the pinned theme (SHA-256 checked) and renders the icon before compiling.
$profile = if ($Fast) { 'fast' } else { 'release' }
cargo build --manifest-path (Join-Path $app 'Cargo.toml') --profile $profile --target-dir $out
if ($LASTEXITCODE) { throw 'cargo build failed' }

$version = (Select-String -LiteralPath (Join-Path $app 'Cargo.toml') -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
$exe = Join-Path $out "$profile\dodge-toolbox.exe"
if (-not (Test-Path -LiteralPath $exe)) { throw "Build produced no exe at $exe" }

# The exe looks for artifacts\ and scripts\ next to itself.
$dest = Join-Path $root "Dodge Toolbox $version.exe"
Copy-Item -LiteralPath $exe $dest -Force
Write-Host "Built: $dest" -ForegroundColor Green
