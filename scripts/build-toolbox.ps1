<#
.SYNOPSIS
  Builds the Dodge Toolbox portable exe and copies it to the repo root.
#>
$ErrorActionPreference = 'Stop'

$root = Split-Path $PSScriptRoot
$app  = Join-Path $root 'toolbox'

Push-Location $app
try {
    if (-not (Test-Path 'node_modules')) {
        npm ci
        if ($LASTEXITCODE) { throw 'npm ci failed' }
    }

    npm run dist
    if ($LASTEXITCODE) { throw 'npm run dist failed' }

    $exe = Get-ChildItem 'dist' -Filter 'Dodge Toolbox *.exe' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $exe) { throw 'Build produced no exe in toolbox\dist' }

    # The exe looks for lineage\, mindthegapps\ and orangefox\ next to itself.
    Copy-Item $exe.FullName $root -Force
    Write-Host "Built: $(Join-Path $root $exe.Name)" -ForegroundColor Green
}
finally {
    Pop-Location
}
