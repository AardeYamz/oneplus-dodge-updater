<#
.SYNOPSIS
  Builds the Dodge Toolbox portable exe.
  Build output goes to the X: drive store (<Store>\toolbox-dist); the finished exe is copied
  to the repo root on C:, where the updater lives.
#>
param(
    [string]$Store = 'X:\Android Development\Devices\Dodge'
)
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$app  = Join-Path $root 'toolbox'
$out  = Join-Path $Store 'toolbox-dist'

if (-not (Test-Path -LiteralPath $Store)) { throw "Store folder not found: $Store (is X: connected?)" }

Push-Location $app
try {
    if (-not (Test-Path 'node_modules')) {
        npm ci
        if ($LASTEXITCODE) { throw 'npm ci failed' }
    }

    # Call electron-builder directly: npm's cmd shim splits the output path at the spaces.
    npm run theme
    if ($LASTEXITCODE) { throw 'theme fetch failed' }
    node node_modules/electron-builder/cli.js --win portable "--config.directories.output=$out"
    if ($LASTEXITCODE) { throw 'electron-builder failed' }

    $exe = Get-ChildItem -LiteralPath $out -Filter 'Dodge Toolbox *.exe' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $exe) { throw "Build produced no exe in $out" }

    # The exe looks for artifacts\ next to itself (a link to the X: store).
    Copy-Item -LiteralPath $exe.FullName $root -Force
    Write-Host "Built: $(Join-Path $root $exe.Name)" -ForegroundColor Green
}
finally {
    Pop-Location
}
