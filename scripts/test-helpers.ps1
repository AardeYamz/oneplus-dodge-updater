# Tests dodge-helpers.ps1 against stub adb/fastboot programs on a temporary PATH. No device is touched.
# Runs on Windows PowerShell, PowerShell 7 on Windows, and PowerShell 7 on Linux (needs `pwsh` for the stubs).
$ErrorActionPreference = 'Stop'
$work = Join-Path ([IO.Path]::GetTempPath()) "dodge-stubs-$PID"
New-Item -ItemType Directory -Force $work | Out-Null

# One stub implementation (stub.ps1); each tool is a tiny wrapper around it. Behaviour comes from STUB_* env vars.
@'
param([string]$Tool, [Parameter(ValueFromRemainingArguments)][string[]]$A)
$two = $env:STUB_DEVICES -eq '2'
if ($Tool -eq 'adb') {
  switch ($A[0]) {
    'devices' {
      'List of devices attached'
      if ($env:STUB_MODE -eq 'adb') { "ABC123`tdevice"; if ($two) { "DEF456`tdevice" } }
    }
    'shell' { $env:STUB_PRODUCT }
    'sideload' {
      if ($env:STUB_SIDELOAD_OK) { [Console]::Error.WriteLine('Total xfer: 1.00x'); exit 1 }
      [Console]::Error.WriteLine('adb: sideload connection failed'); exit 1
    }
    'reboot' { if ($env:STUB_FAIL -eq 'reboot') { exit 1 } }
  }
} else {
  switch ($A[0]) {
    'devices' { if ($env:STUB_MODE -eq 'fastboot') { "ABC123`tfastboot"; if ($two) { "DEF456`tfastboot" } } }
    'getvar' { [Console]::Error.WriteLine("product: $env:STUB_PRODUCT"); [Console]::Error.WriteLine('Finished. Total time: 0.001s') }
    'flash' { if ($env:STUB_FAIL -eq 'flash') { exit 1 } }
  }
}
exit 0
'@ | Set-Content (Join-Path $work 'stub.ps1')
foreach ($t in 'adb', 'fastboot') {
  if ($IsLinux -or $IsMacOS) {
    $f = Join-Path $work $t
    "#!/bin/sh`nexec pwsh -NoProfile -File `"`$(dirname `"`$0`")/stub.ps1`" $t `"`$@`"`n" | Set-Content -NoNewline $f
    chmod +x $f
  } else {
    "@pwsh -NoProfile -File `"%~dp0stub.ps1`" $t %*`r`n" | Set-Content (Join-Path $work "$t.cmd")
  }
}
$sep = [IO.Path]::PathSeparator
$env:PATH = "$work$sep$env:PATH"

. (Join-Path $PSScriptRoot 'dodge-helpers.ps1')

$failed = 0
function Test-Case($name, [scriptblock]$body, [string]$throws) {
  $env:STUB_MODE = 'adb'; $env:STUB_PRODUCT = 'OP5D55L1'; $env:STUB_DEVICES = ''; $env:STUB_FAIL = ''; $env:STUB_SIDELOAD_OK = ''
  $err = $null
  try { & $body *>&1 | Out-Null } catch { $err = $_.Exception.Message }
  $ok = if ($throws) { $err -and $err -match $throws } else { -not $err }
  if ($ok) { Write-Host "ok   $name" } else { Write-Host "FAIL $name (error: $err)"; $script:failed++ }
}

Test-Case 'guard passes for one dodge over adb' { Assert-Dodge }
Test-Case 'guard passes for one dodge in fastboot' { $env:STUB_MODE = 'fastboot'; $env:STUB_PRODUCT = 'sun'; Assert-Dodge }
Test-Case 'guard refuses the bootloader board name over adb' { $env:STUB_PRODUCT = 'sun'; Assert-Dodge } 'not dodge'
Test-Case 'guard refuses another device (adb)' { $env:STUB_PRODUCT = 'pixel'; Assert-Dodge } 'not dodge'
Test-Case 'guard refuses another device (fastboot)' { $env:STUB_MODE = 'fastboot'; $env:STUB_PRODUCT = 'pixel'; Assert-Dodge } 'not dodge'
Test-Case 'guard refuses two devices' { $env:STUB_DEVICES = '2'; Assert-Dodge } 'exactly one'
Test-Case 'guard refuses no device' { $env:STUB_MODE = 'none'; Assert-Dodge } 'exactly one'
Test-Case 'guard refuses an unreadable model' { $env:STUB_PRODUCT = ''; Assert-Dodge } 'Could not read'
Test-Case 'a failed fastboot flash stops the script' { $env:STUB_FAIL = 'flash'; Invoke-Native fastboot flash recovery x.img } 'failed \(exit 1\)'
Test-Case 'a successful fastboot flash goes on' { Invoke-Native fastboot flash recovery x.img }
Test-Case 'Enter-Bootloader stops on a wrong device before rebooting' { $env:STUB_PRODUCT = 'pixel'; Enter-Bootloader } 'not dodge'
Test-Case 'sideload with Total xfer counts as done despite exit 1' { $env:STUB_SIDELOAD_OK = '1'; Invoke-Sideload 'rom.zip' }
Test-Case 'sideload without Total xfer fails' { Invoke-Sideload 'rom.zip' } 'did not finish'

# Downloads, from file:// URLs so no network is needed.
$src = Join-Path $work 'source.img'; [IO.File]::WriteAllText($src, 'abc')
$url = [Uri]::new($src, [UriKind]::Absolute).AbsoluteUri   # a bare [uri] cast leaves a Linux path relative
$good = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'   # SHA-256 of "abc"
$out = Join-Path $work 'out.img'
function Assert-Gone { if ((Test-Path $out) -or (Test-Path "$out.part")) { throw 'a file was left behind' } }
Test-Case 'a download with the wrong hash fails and leaves nothing' {
  try { Get-Download $url $out ('0' * 64) } catch { if ($_.Exception.Message -notmatch 'mismatch') { throw } ; Assert-Gone; return }
  throw 'no error'
}
Test-Case 'a failed transfer leaves nothing' {
  try { Get-Download "$url.missing" $out $good } catch { if ($_.Exception.Message -notmatch 'failed \(exit') { throw } ; Assert-Gone; return }
  throw 'no error'
}
Test-Case 'a missing hash is refused' { Get-Download $url $out '' } 'No usable SHA-256'
Test-Case 'a good download is kept' { Get-Download $url $out $good; if ((Get-Content -Raw $out) -ne 'abc') { throw 'wrong content' } }
Test-Case 'a bad existing file is replaced' { Set-Content $out 'junk'; Get-Download $url $out $good; if ((Get-Content -Raw $out) -ne 'abc') { throw 'not replaced' } }

Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
if ($failed) { Write-Host "$failed test(s) failed"; exit 1 }
Write-Host 'all passed'
