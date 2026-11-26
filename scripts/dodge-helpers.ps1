# Shared by update-lineage.ps1 and scripts/test-helpers.ps1. Dot-source it; it only defines functions.
# $ErrorActionPreference does not stop on a failed native command, so every adb/fastboot call that changes the phone goes through here.
function Invoke-Native {
  param([string]$Tool, [Parameter(ValueFromRemainingArguments)][string[]]$ToolArgs)
  & $Tool @ToolArgs
  if ($LASTEXITCODE -ne 0) { throw "$Tool $($ToolArgs -join ' ') failed (exit $LASTEXITCODE)." }
}
# Sideloads stop near 47% with a non-zero exit even when they worked, so judge by the "Total xfer" line instead.
function Invoke-Sideload($file) {
  $ErrorActionPreference = 'Continue'   # adb writes progress to stderr, which Stop would turn into an error
  $o = & adb sideload $file 2>&1 | Out-String
  Write-Host $o
  if ($o -notmatch 'Total xfer') { throw "adb sideload of $file did not finish (no 'Total xfer' line)." }
}
# Make $out match the published SHA-256: keep it if it does, otherwise remove it and download it again.
# The download goes to "$out.part" and is renamed only once the hash matches, so a failed one leaves nothing behind.
function Get-Download($url, $out, $sha256) {
  if ("$sha256" -notmatch '^[0-9a-fA-F]{64}$') { throw "No usable SHA-256 for $out. Refusing to download it unchecked." }
  if (Test-Path -LiteralPath $out) {
    if ((Get-FileHash -LiteralPath $out -Algorithm SHA256).Hash -eq $sha256) { return }
    Write-Warning "$out does not match the published SHA-256. Downloading it again."
    Remove-Item -LiteralPath $out
  }
  $part = "$out.part"
  $curl = if ($IsLinux -or $IsMacOS) { 'curl' } else { 'curl.exe' }   # plain "curl" is an alias in Windows PowerShell
  try {
    # Not via Invoke-Native, which would take -o as one of its own parameters. curl writes progress to stderr, which
    # Windows PowerShell 5.1 turns into a terminating error under Stop when the streams are redirected.
    $ErrorActionPreference = 'Continue'
    & $curl -L --fail -o $part $url
    $code = $LASTEXITCODE
    $ErrorActionPreference = 'Stop'
    if ($code -ne 0) { throw "Download of $url failed (exit $code)." }
    if ((Get-FileHash -LiteralPath $part -Algorithm SHA256).Hash -ne $sha256) { throw "SHA-256 mismatch for $out. The download was removed; retry." }
    Move-Item -LiteralPath $part $out
  } finally {
    if (Test-Path -LiteralPath $part) { Remove-Item -LiteralPath $part }
  }
}
# Refuse to flash unless exactly one device is attached and it is the dodge.
# The phone never calls itself just "dodge": ro.product.device is the OnePlus codename (the OTA's pre-device list),
# and the bootloader's "product" is the SoC board, "sun", which other SM8750 phones share.
$AdbDevices = 'dodge', 'OP5D55L1', 'OP5D0DL1'
$FastbootProducts = $AdbDevices + 'sun'
function Assert-Dodge {
  $ErrorActionPreference = 'Continue'
  $adbList = @(adb devices 2>&1 | Where-Object { $_ -match '^\S+\s+(device|recovery|sideload|unauthorized|offline|bootloader)\s*$' })
  $fbList = @(fastboot devices 2>&1 | Where-Object { $_ -match '^\S+\s+fastboot\s*$' })
  $n = $adbList.Count + $fbList.Count
  if ($n -ne 1) { throw "Expected exactly one attached device, found $n. Unplug the others and retry." }
  if ($fbList.Count) {
    # "$_" keeps the bare text: Windows PowerShell 5.1 wraps stderr lines in error records that format with extra noise.
    $line = fastboot getvar product 2>&1 | ForEach-Object { "$_".Trim() } | Where-Object { $_ -like 'product:*' } | Select-Object -First 1
    $product = "$line" -replace '^product:\s*', ''
    $allowed = $FastbootProducts
  } else {
    $product = "$(adb shell getprop ro.product.device)".Trim()
    $allowed = $AdbDevices
  }
  if (-not $product) { throw "Could not read the device model. Refusing to flash (expected dodge)." }
  if ($product -cnotin $allowed) { throw "Attached device is '$product', not dodge. Refusing to flash." }
}
function Enter-Bootloader {
  Assert-Dodge
  Invoke-Native adb reboot bootloader
  Start-Sleep 12
  Assert-Dodge
}
