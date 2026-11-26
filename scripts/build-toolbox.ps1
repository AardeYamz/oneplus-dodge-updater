<#
.SYNOPSIS
  Builds the portable Dodge Toolbox exe.

.DESCRIPTION
  The Toolbox is written in Rust with Tauri (https://tauri.app). Its page (toolbox\ui\) is drawn by the WebView2
  runtime that ships with Windows 10/11, so the exe is a few MB; toolbox\src\ does the phone, network and disk work.
  Needs Rust with the MSVC toolchain (https://rustup.rs).

  Runs cargo build with output in %LOCALAPPDATA%\dodge-toolbox-target, then copies "Dodge Toolbox <version>.exe" to
  the repo root. Keep the exe there so it finds artifacts\ and scripts\.

  Theme: ui\theme\ is not committed. Every cargo build first runs toolbox\build.rs, which downloads the version
  pinned in toolbox\theme.json from https://theme.yannislam.org, checks each file's SHA-256 (needs a network
  connection the first time) and renders the window and exe icon from the theme's favicon.svg. Setting
  DODGE_SKIP_THEME=1 builds without the theme, for offline compile checks only.

  Without this script: "cargo run" in toolbox\ starts a dev build, and "cargo build --release" writes
  toolbox\target\release\dodge-toolbox.exe.

.PARAMETER Fast
  Quicker build (no LTO, fewer optimizations) with a slightly larger exe. For local testing.

.PARAMETER BumpTheme
  Pin the newest published theme version in toolbox\theme.json before building.

.EXAMPLE
  pwsh scripts\build-toolbox.ps1 -Fast
#>
param(
    [switch]$Fast,
    [switch]$BumpTheme
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$app  = Join-Path $root 'toolbox'
# Build on the local disk: the target tree is thousands of small files, slow on a network or external drive.
$out  = Join-Path $env:LOCALAPPDATA 'dodge-toolbox-target'

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
