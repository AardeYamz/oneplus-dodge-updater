<#
.SYNOPSIS
  Update the OnePlus 13 (dodge) to the latest LineageOS nightly, then restore
  MindTheGapps, OrangeFox and KernelSU-Next. Follows the manual flash instructions in docs/reference.md.

.DESCRIPTION
  Run in PowerShell from anywhere; downloads live in artifacts\ under -DataRoot (default: DODGE_DATA_ROOT from .env, else the X: drive store). Needs adb/fastboot (platform-tools) on PATH,
  USB debugging authorized, bootloader unlocked.
  Steps that need a tap on the phone pause and wait. Each destructive step asks first.
  A same-major update keeps data. Back up before a major jump (e.g. 24 -> 25).

.PARAMETER Build
  Build date to install (yyyyMMdd). Default: newest nightly from the LineageOS API.
#>
[CmdletBinding()]
param(
  [string]$Build,
  [string]$DataRoot,
  [string]$Device = 'dodge',
  [string]$Gapps,
  [string]$FoxZip,
  [string]$KsuPackage = 'com.rifsxd.ksunext'
)
$ErrorActionPreference = 'Stop'

# Data root: -DataRoot, else DODGE_DATA_ROOT (env var or .env at the repo root), else the X: drive store.
if (-not $DataRoot) { $DataRoot = $env:DODGE_DATA_ROOT }
if (-not $DataRoot) {
  $envFile = Join-Path (Split-Path $PSScriptRoot) '.env'
  if (Test-Path $envFile) {
    $line = Get-Content $envFile | Where-Object { $_ -match '^\s*DODGE_DATA_ROOT\s*=' } | Select-Object -First 1
    if ($line) { $DataRoot = ($line -split '=', 2)[1].Trim().Trim('"', "'") }
  }
}
if (-not $DataRoot) { $DataRoot = 'X:\Android Development\Devices\Dodge' }
if (-not (Test-Path -LiteralPath $DataRoot)) { throw "Data root not found: $DataRoot (is X: connected?)" }
$Art = Join-Path $DataRoot 'artifacts'
if (-not $Gapps)  { $Gapps  = Join-Path $Art 'mindthegapps\MindTheGapps-17.0.0-arm64-20261006_111038.zip' }
if (-not $FoxZip) { $FoxZip = Join-Path $Art 'orangefox\OrangeFox-R12.0_2-dodge.zip' }

function Confirm-Step($msg) {
  if ((Read-Host "$msg [y/N]") -ne 'y') { throw 'Stopped by user.' }
}
function Wait-Tap($msg) { Read-Host "$msg, then press Enter" | Out-Null }
function Wait-Adb($state) {
  for ($i = 0; $i -lt 60; $i++) {
    if ((adb devices) -match "\s$state$") { return }
    Start-Sleep 3
  }
  throw "Phone never reached adb state '$state'."
}
function Wait-Boot {
  adb wait-for-device
  while ((adb shell getprop sys.boot_completed 2>$null) -notmatch '1') { Start-Sleep 5 }
}
function Get-Download($url, $out) {
  if (-not (Test-Path $out)) { curl.exe -L --fail -o $out $url; if ($LASTEXITCODE) { throw "Download failed: $url" } }
}

# --- Preflight ---
foreach ($t in 'adb', 'fastboot') { if (-not (Get-Command $t -ErrorAction SilentlyContinue)) { throw "$t not on PATH." } }
$state = (adb devices | Select-String "`tdevice$")
if (-not $state) { throw 'No authorized adb device. Unlock the phone and accept the USB debugging prompt.' }
$current = adb shell getprop ro.lineage.version
$kernel = adb shell uname -r
Write-Host "Installed: $current  | kernel $kernel"

# --- Pick the build ---
$api = Invoke-RestMethod "https://download.lineageos.org/api/v1/$Device/nightly/x"
$rel = if ($Build) { $api.response | Where-Object { $_.filename -match "-$Build-" } | Select-Object -First 1 }
       else { $api.response | Sort-Object datetime -Descending | Select-Object -First 1 }
if (-not $rel) { throw 'Build not found in the API response.' }
$null = $rel.filename -match 'lineage-(\d+)\.(\d+)-(\d{8})'
$major = $Matches[1]; $date = $Matches[3]
if ($current -match [regex]::Escape("$($Matches[1]).$($Matches[2])-$date")) { Write-Host 'Already on this build.'; return }
$curMajor = ($current -split '\.')[0]
Write-Host "Target: $($rel.filename)"
if ($curMajor -ne $major) { Write-Warning "Major version change ($curMajor -> $major). Check the wiki for a required wipe and back up first." }
Confirm-Step 'Proceed with the update?'

# --- Download and verify ---
$dir = "$Art\builds\$major-$date"; New-Item -ItemType Directory -Force $dir, "$Art\lineage" | Out-Null
$zip = "$Art\lineage\$($rel.filename)"
Get-Download $rel.url $zip
if ((Get-FileHash $zip -Algorithm SHA256).Hash -ne $rel.id.ToUpper()) { throw 'SHA-256 mismatch. Delete the zip and retry.' }
foreach ($img in 'recovery.img', 'init_boot.img', 'build-manifest.xml') {
  Get-Download "https://mirrorbits.lineageos.org/full/$Device/$date/$img" "$dir\$img"
}
if ((Get-Item "$dir\recovery.img").Length -ne 104857600) { throw 'recovery.img size is wrong.' }

# --- OrangeFox image (extract once; keep out of the Lineage recovery.img) ---
Expand-Archive $FoxZip -DestinationPath "$Art\fox-extract" -Force
if ((Get-Item "$Art\fox-extract\recovery.img").Length -ne 104857600) { throw 'OrangeFox recovery.img is truncated.' }

# --- 1. LineageOS Recovery, then sideload the ROM ---
adb reboot bootloader; Start-Sleep 12
fastboot flash recovery "$dir\recovery.img"
fastboot reboot recovery
Wait-Tap 'On the phone: Apply update -> Apply from ADB'
Wait-Adb 'sideload'
adb sideload $zip   # stopping near 47% with "Total xfer: 1.00x" is normal

# --- 2. Switch slot, then GApps ---
Wait-Tap 'On the phone: Advanced -> Reboot to recovery, then Apply update -> Apply from ADB'
Start-Sleep 20; Wait-Adb 'sideload'
adb sideload $Gapps   # answer Yes to the signature warning

# --- 3. Boot once ---
Wait-Tap 'On the phone: Reboot system now (first boot takes several minutes)'
Wait-Boot
Write-Host "Booted: $(adb shell getprop ro.lineage.version)"

# --- 4. OrangeFox last ---
adb reboot bootloader; Start-Sleep 12
fastboot flash recovery "$Art\fox-extract\recovery.img"
fastboot reboot; Wait-Boot

# --- 5. KernelSU-Next: patch the new init_boot in the manager ---
adb push "$dir\init_boot.img" "/sdcard/Download/init_boot_$date.img"
Write-Host "Kernel $(adb shell uname -r) -> KMI android15-6.6 (6.6.x). Set it manually; the manager auto-picks the wrong one."
Wait-Tap "On the phone: $KsuPackage -> Install -> Select and patch a file -> init_boot_$date.img, KMI android15-6.6"
$patched = (adb shell "ls -t /sdcard/Download/kernelsu_next_patched_*.img" | Select-Object -First 1).Trim()
$out = "$Art\images\ksu-patched-init_boot-$date.img"
New-Item -ItemType Directory -Force (Split-Path $out) | Out-Null
adb pull $patched $out
$ver = [regex]::Matches([Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($out)), 'vermagic=[^\x00]{0,40}') | Select-Object -First 1 -ExpandProperty Value
Write-Host $ver
if ($ver -notmatch 'vermagic=6\.6\.') { throw "Wrong KMI in the patched image ($ver). Re-patch with android15-6.6." }
adb reboot bootloader; Start-Sleep 12
fastboot flash init_boot $out
fastboot reboot; Wait-Boot

Write-Host "Done: $(adb shell getprop ro.lineage.version). Open the KernelSU-Next manager and confirm it says Working."
Write-Host "Rollback for root: fastboot flash init_boot $dir\init_boot.img"
