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
# Refuse to flash unless exactly one device is attached and it is the dodge.
$RequiredDevice = 'dodge'
function Assert-Dodge {
  $ErrorActionPreference = 'Continue'
  $adbList = @(adb devices 2>&1 | Where-Object { $_ -match '^\S+\s+(device|recovery|sideload|unauthorized|offline|bootloader)\s*$' })
  $fbList = @(fastboot devices 2>&1 | Where-Object { $_ -match '^\S+\s+fastboot\s*$' })
  $n = $adbList.Count + $fbList.Count
  if ($n -ne 1) { throw "Expected exactly one attached device, found $n. Unplug the others and retry." }
  if ($fbList.Count) {
    $line = (fastboot getvar product 2>&1 | Out-String) -split "`r?`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ -like 'product:*' } | Select-Object -First 1
    $product = "$line" -replace '^product:\s*', ''
  } else {
    $product = "$(adb shell getprop ro.product.device)".Trim()
  }
  if (-not $product) { throw "Could not read the device model. Refusing to flash (expected '$RequiredDevice')." }
  if ($product -ne $RequiredDevice) { throw "Attached device is '$product', not '$RequiredDevice'. Refusing to flash." }
}
function Enter-Bootloader {
  Assert-Dodge
  Invoke-Native adb reboot bootloader
  Start-Sleep 12
  Assert-Dodge
}
