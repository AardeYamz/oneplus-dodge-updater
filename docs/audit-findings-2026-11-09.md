# Audit findings, 2026-11-09

Audit of `main` at `944f692`. This file is a work list for a later fixing agent. Nothing here has been fixed. Each item lists the location, the problem, the fix and how to verify it. Check the box when the fix is merged and verified.

Scope read: `README.md`, `.claude/skills/update-lineageos/SKILL.md`, `.github/workflows/build.yml`, `scripts/*.ps1`, all of `toolbox/src/`, `toolbox/build.rs`, `toolbox/Cargo.toml`, `toolbox/ui/`, `toolbox/tauri.conf.json`. Open PRs: none (all 13 are merged). No existing findings or TODO docs in the repo. No TODO/FIXME/HACK/XXX markers and no secret-like strings in tracked files.

Verification run: `toolbox/src/parse.rs` unit tests compiled and run standalone (4 passed). The `is_wipe` blocklist was probed directly. The shared mermaid linter passes on the one block in `README.md`. Full `cargo test` was not run, because it compiles the whole Tauri tree and needs the theme download. Dependency advisories came from the GitHub Advisory Database. Action SHAs were checked against `gh api` release lookups.

## Summary

| Severity | Count | IDs |
| --- | --- | --- |
| Critical | 0 | none |
| High | 5 | H1 to H5 |
| Medium | 7 | M1 to M7 |
| Low | 13 | L1 to L13 |

## High

### H1. Vulnerable `zip` crate version in use for OrangeFox extraction

- [ ] Location: `toolbox/Cargo.toml:13` (`zip = { version = "2", ... }`), `toolbox/Cargo.lock:5080` (locked `2.6.1`), used at `toolbox/src/toolbox.rs:470` (`extract_fox`).
- What is wrong: the lock resolves `zip` to 2.6.1, which is inside three open advisories: GHSA-m665-v7pm-7grj (rated critical by the advisory, "zip cloaking" leading to arbitrary file overwrite, fixed in 9.0.5), GHSA-q7v9-x93c-xqv3 (high, symlink-target handling, fixed in 9.0.0) and GHSA-8hxf-qr8p-62x4 (medium, AES HMAC bypass, fixed in 9.0.0). The latest stable is 9.5.0.
- Why it matters: `extract_fox` unpacks a user-supplied zip. A malicious or tampered OrangeFox zip could write files outside `fox-extract` on the advisory path. Reaching it requires a bad zip, so the practical severity is lower than the advisory rating, but the fix is cheap.
- Fix: set `zip = { version = "9", default-features = false, features = ["deflate"] }`, update the call sites to the 9.x API, and run `cargo update -p zip`. Confirm `Cargo.lock` no longer contains 2.x.
- Verify: `gh api -X GET advisories -f ecosystem=rust -f affects=zip@<locked version>` returns nothing. `cargo test --profile fast` passes. A test that extracts a crafted zip with `../` entries writes nothing outside the destination.

### H2. Autorun skips every `confirm` prompt

- [ ] Location: `toolbox/ui/renderer.js:113` (`if (!auto && s.confirm && ...)`), `renderer.js:144` (`run(true)`), `renderer.js:133` (the single Start dialog).
- What is wrong: in autorun, `run(true)` skips the per-step `confirm` for `flashLineageRecovery`, `sideloadGapps` (slot-switch check), `flashFox` and `pullAndFlashKsu` (KMI check and flash). Only one Start dialog runs at the beginning, and it lists the flashes but does not ask the slot-switch question.
- Why it matters: the slot-switch confirmation exists because a stale "sideload" state can come from the old session (see `renderer.js:12` and `SKILL.md`). Autorun can sideload GApps without that check. The README says each wipe-risk step asks first, which autorun does not do.
- Fix: keep `confirm` in autorun. Show the same dialog before each `confirm` step, or make the Start dialog list each confirmation text. Do not let `auto` bypass `confirm`.
- Verify: in autorun, reach the `sideloadGapps` step and check that the slot-switch question appears before `step('sideloadGapps')` is called. Test by stubbing `tb.step` in the page.

### H3. Patched init_boot is chosen by newest mtime, not by this build

- [ ] Location: `toolbox/src/toolbox.rs:540-541` (`ls -t /sdcard/Download/kernelsu_next_patched_*.img`, take the first line), `scripts/update-lineage.ps1:122`.
- What is wrong: the code picks the newest `kernelsu_next_patched_*.img` on the phone. It is not tied to the `init_boot_<date>.img` pushed for this build. An older patched image left in `/sdcard/Download` will be selected. The only check is the KMI string (`vermagic=6.6.`), which an older image from the same kernel line also passes. The image is then flashed to `init_boot` (`toolbox.rs:556`, `update-lineage.ps1:130`).
- Why it matters: `init_boot` from a different LineageOS build can fail to boot, and the flash is only recoverable through fastboot.
- Fix: require the candidate to be newer than the push (record `date +%s` or the file mtime before the push and compare), or list the candidates and make the user pick one. Refuse to flash if the candidate was not produced after the push. Keep the vermagic check.
- Verify: with an older `kernelsu_next_patched_*.img` placed on the device and a newer one absent, the step must refuse. Test the selection logic as a pure function in `parse.rs`.

### H4. No device identity check before fastboot flash or erase

- [ ] Location: `toolbox/src/toolbox.rs:298-304` (`to_bootloader`), `:491-497` (`flash_lineage_recovery`), `:516-523` (`flash_fox`), `:556` (flash init_boot in `pull_and_flash_ksu`), `:565` (`rollback_root`). `scripts/update-lineage.ps1:64` (preflight), `:97`, `:115`, `:130`.
- What is wrong: nothing checks that the attached device is `dodge` before flashing. Preflight reads `ro.lineage.version` and the kernel only. `to_bootloader` accepts any fastboot device. Multiple attached devices are not detected. The repo has no `ro.product.device` or `getvar product` check (grep confirms).
- Why it matters: the flash steps write recovery and init_boot. A different device, or a second phone plugged in, would receive the image.
- Fix: before the first flash, read `fastboot getvar product` (and/or `adb shell getprop ro.product.device`) and require `dodge`. Refuse when more than one device is listed in `adb devices` or `fastboot devices`. Put the check in one function used by all flash paths, in both the toolbox and the script.
- Verify: unit-test the parser that reads `product: dodge` with sample output. Confirm that the flash functions call the guard first. Do not run against a real device.

### H5. PowerShell script ignores native exit codes on adb and fastboot

- [ ] Location: `scripts/update-lineage.ps1:97-98`, `:100-101`, `:105-106`, `:115-116`, `:130-131`.
- What is wrong: `$ErrorActionPreference = 'Stop'` does not stop on a non-zero exit from `adb` or `fastboot` (native commands). Only `curl.exe` (line 57) and `cargo` in `build-toolbox.ps1:36` check `$LASTEXITCODE`. A failed `fastboot flash recovery` is followed by `fastboot reboot recovery` and then the sideload prompts.
- Why it matters: the script goes on after a failed recovery or init_boot flash and tells the user to tap on the phone. It can leave the device in a half-flashed state with no error.
- Fix: after each `fastboot flash` and `fastboot reboot`, check `$LASTEXITCODE` and throw. For `adb sideload`, use the output (`Total xfer`, `Success`) rather than the exit code, as the comment at line 101 says a non-zero exit is normal at 47%.
- Verify: put stub `adb.cmd` and `fastboot.cmd` on a temporary `PATH` in a scratch directory. The stub `fastboot` exits 1 on `flash`. Pipe answers to `Read-Host`. The script must stop at the flash. No device is touched.

## Medium

### M1. Sideload and custom-command exit codes are ignored, and the step reports success

- [ ] Location: `toolbox/src/toolbox.rs:502` (`sideload_rom`), `:512` (`sideload_gapps`), `:586-587` (`custom` runs the command then returns `"Done."` regardless of its result).
- Why it matters: a failed GApps sideload is shown as a green step, and the user taps through to the next prompt. A failed custom `fastboot flash` says "Done.".
- Fix: keep the exit code for custom commands and return an error on non-zero. For sideloads, classify the output: accept `Total xfer` with `Success` or with the known 47% stop, and error otherwise.
- Verify: `custom` with a command that exits non-zero must return `ok: false`. Test the sideload output classifier with sample outputs.

### M2. Downloads are not verified for init_boot, recovery and manifest, and truncated transfers are accepted

- [ ] Location: `toolbox/src/toolbox.rs:317-339` (no check that bytes read equals `Content-Length`), `:453-459` (only `recovery.img` size is checked; `init_boot.img` and `build-manifest.xml` have no check), `:307-311` (existing file is reused without any check). `scripts/update-lineage.ps1:56-58`, `:84-89`.
- What is wrong: `init_boot.img` is pushed for patching and flashed on rollback (`toolbox.rs:565`) but is never hashed. A connection that closes early is treated as complete. The PowerShell `Get-Download` writes straight to the final path, so a failed curl leaves a partial file that the next run accepts.
- Fix: compare the bytes read with `Content-Length` when it is present and remove the partial file on error. Check each image against a published checksum if the LineageOS mirror provides one (not verified in this audit: check the mirror listing first). Otherwise at least check the size against the mirror's listing. Write the PowerShell download to a `.part` file and rename it, as the toolbox already does.
- Verify: with a server that closes early, the download must fail and leave no final file.

### M3. A failed ROM zip is never deleted, so retries keep failing

- [ ] Location: `toolbox/src/toolbox.rs:448-452` (hash mismatch returns "Delete the zip and retry"), `:308` (existing file skips download). `scripts/update-lineage.ps1:84-85`.
- Why it matters: after one bad download, every retry reuses the bad zip, and the user has to delete it by hand. The message tells them to, but the tool does not.
- Fix: on hash mismatch, delete the zip and return an error that says it was removed.
- Verify: a zip with a wrong hash is removed after a failed step, and the next run downloads it again.

### M4. Custom-command wipe blocklist misses `flash userdata` and `flash metadata`

- [ ] Location: `toolbox/src/parse.rs:72-76` (`is_wipe`), used at `toolbox/src/toolbox.rs:582`.
- What is wrong: the blocklist matches `-w`, `erase userdata`, `format` and `wipe`. Probed directly: `flash userdata userdata.img` and `flash metadata metadata.img` return `blocked=false`. Flashing a userdata image wipes data, and a metadata flash can break encryption keys. The README (line 125) lists the blocked commands as if they were complete.
- Fix: switch to an allowlist of read-only and reboot commands for the box, or block `flash` of `userdata`, `metadata`, `cache` and `super`. Update the README sentence.
- Verify: add cases to the `command_line` test in `parse.rs`: `flash userdata x.img`, `flash metadata x.img`, `flash super x.img` must all be blocked.

### M5. No server-side lock on steps; the busy flag is client-only

- [ ] Location: `toolbox/src/main.rs:16-23` (`step` runs on any thread), `toolbox/src/toolbox.rs:388-391` (every non-status step resets `cancelled`), `:189` (`running` is overwritten by a second child, so the first can no longer be cancelled). `toolbox/ui/renderer.js:56-84` (the only guard).
- Why it matters: a second invoke (a second window, a bug, or a stale handler) runs a second flash in parallel. The first step's cancel flag is cleared and its child is orphaned.
- Fix: add a mutex or an `AtomicBool` "busy" to `Toolbox`. `step` returns `{ok:false,error:"busy"}` when a step is already running.
- Verify: call `step` twice concurrently in a unit test with a stub child; the second must return busy.

### M6. No timeouts on network calls, and Cancel cannot interrupt a stalled connection

- [ ] Location: `toolbox/src/toolbox.rs:85-97` (agent built with no timeouts), `:313` (`.call()`), `:324` (cancel is checked only between chunks). `toolbox/build.rs:99-104` (no timeout on the theme fetch). `toolbox/src/toolbox.rs:361` (API call).
- Why it matters: a stalled server blocks the step indefinitely, and Cancel does not take effect until a chunk arrives.
- Fix: set connect and read timeouts on the ureq agent (`timeout_connect`, `timeout_read`), and on the build.rs request.
- Verify: point the download at a local server that stalls and confirm the step errors within the timeout.

### M7. CI and the build script do not enforce the lockfile

- [ ] Location: `.github/workflows/build.yml:29` (`cargo test ... --profile fast`) and `:31` (`cargo build ...`); `scripts/build-toolbox.ps1:35`.
- Why it matters: without `--locked`, cargo can update `Cargo.lock` during the build. The CI result then does not match the committed lock, and the shipped exe may use different crates from the reviewed ones.
- Fix: add `--locked` to all three cargo invocations.
- Verify: after adding the flag, the workflow fails if `Cargo.lock` is out of date.

## Low

### L1. Remote filename used as a path component without validation

- [ ] Location: `toolbox/src/toolbox.rs:369-373`. `filename` comes from the API response and goes into `self.artifact(&["lineage", &filename])`. `PathBuf::extend` (push) lets `..` components or an absolute path escape `artifacts\lineage`.
- Fix: check the filename against `^lineage-\d+\.\d+-\d{8}-nightly-dodge-signed\.zip$` before use.
- Verify: a filename containing `..\` is rejected.
- Note: the URL and SHA-256 both come from the same API response, so the hash does not protect against a compromised API. Recovery's signature check on sideload is the second layer.

### L2. "Already on this build" does not stop the toolbox

- [ ] Location: `toolbox/src/toolbox.rs:433-439` (`pick_build` has no already-installed check), `toolbox/ui/renderer.js:155` (text only), `:209-211`. The script stops at `scripts/update-lineage.ps1:75`.
- Fix: return a flag from `pick_build` and have the UI stop autorun, as the script does.
- Verify: with a matching `current` value, autorun must not reach `flashLineageRecovery`.

### L3. No mermaid lint workflow, though the README has a mermaid block

- [ ] Location: `.github/workflows/` has only `build.yml`. `README.md:40-45` contains a mermaid block. The org standard (global CLAUDE.md) says each repo calls the shared linter from a `mermaid-lint.yml` workflow.
- Fix: add `mermaid-lint.yml` calling `AardeYamz/personal-domain-devops/mermaid-lint@v1`, with the action pinned to a full SHA (check access first, the source repo may be private). The local linter run passes (1 block, 0 failed).
- Verify: `act -l` lists the job, and the job passes on the README.

### L4. Dependencies behind latest (not the zip crate, which is H1)

- [ ] Location: `toolbox/Cargo.toml:12` (`sha2 = "0.10"`, latest 0.11.1), `:14` (`rfd = "0.15"`, latest 0.18.0), `:18` and `:22` and `:26` (`ureq = "2"`, latest 3.5.1), `:29` (`resvg = "0.45"`, latest 0.49.0), `:30` (`ico = "0.4"`, latest 0.6.0, and both 0.4.0 and 0.6.0 are in the lock).
- Fix: bump each to the latest version and fix API changes. ureq 3 changes the agent API used in `toolbox.rs:85-97`.
- Verify: `cargo tree -d` shows no duplicate versions after the change; the parse tests and build pass.
- Note: tauri 2.12.5 and tauri-build 2.7.4 are the latest stable, so no change is needed there.

### L5. Tauri CSP may block fonts shipped with the theme

- [ ] Location: `toolbox/tauri.conf.json:9` (`font-src data:`). `toolbox/ui/index.html:7` loads `theme/yl-fonts.css`.
- Why: if the theme CSS references font files from `'self'`, `font-src data:` blocks them. Not verified, because the theme files are not in the repo.
- Fix: check the theme's font URLs. If they are relative, add `'self'` to `font-src`.
- Verify: open the built app and check the console for CSP font violations.

### L6. `run_script` builds a `cmd /c start` line from a path, and runs with ExecutionPolicy Bypass

- [ ] Location: `toolbox/src/toolbox.rs:598-606`.
- Why: a path containing `&`, `^` or `%` is interpreted by cmd.exe. The root comes from settings or the exe folder.
- Fix: launch `powershell.exe` directly with `-File` and an argument array, or use `Start-Process` with a quoted path. Drop `-ExecutionPolicy Bypass` if signed scripts are not needed, or document why it is needed.
- Verify: a folder named `a&b` launches the script without running `b`.

### L7. Unvalidated inputs: build date, reboot target, `-Build`

- [ ] Location: `toolbox/src/toolbox.rs:364` (build string goes into a filter), `:570-574` (`to` is passed to `adb reboot <to>`). `scripts/update-lineage.ps1:70` (`-Build` goes into a regex).
- Fix: accept only `^\d{8}$` for the build, and an allowlist of `system`, `bootloader`, `recovery`, `sideload`, `sideload-auto-reboot` for the reboot target.
- Verify: `pickBuild` with `"x"` must fail before the API call.

### L8. Silent fallback to the PATH `adb` (possibly the known-bad 1.0.39)

- [ ] Location: `toolbox/src/toolbox.rs:146-149`. README line 27 says old adb versions fail on the 3 GB zip.
- Fix: when `platformTools` is set and the exe is missing, return an error instead of falling back. Or make the step refuse when the adb version is 1.0.39.
- Verify: with a bad `platformTools`, the sideload step must fail with a clear message.

### L9. Boot wait has no upper bound

- [ ] Location: `toolbox/src/toolbox.rs:285-291` (`wait_boot` loops until cancelled). `scripts/update-lineage.ps1:52-55`.
- Fix: time out after a set period (for example 20 minutes) and say so in the message.
- Verify: with a stub that never reports `1`, the step errors after the timeout.

### L10. Fixed sleeps instead of waiting for the device

- [ ] Location: `scripts/update-lineage.ps1:96`, `:114`, `:129` (`Start-Sleep 12`).
- Fix: use the existing `Wait-Adb` or a fastboot device poll, as the toolbox does (`wait_for`).

### L11. `.Trim()` on a possibly null value in the script

- [ ] Location: `scripts/update-lineage.ps1:122`. If `ls` finds no file, `Select-Object -First 1` returns null and `.Trim()` throws an unclear error.
- Fix: check for empty output and throw a clear message, as the toolbox does at `toolbox.rs:542-544`.

### L12. `-Store` in build script is a no-op apart from an existence check

- [ ] Location: `scripts/build-toolbox.ps1:7` and `:20`. README line 91 describes it as the data store.
- Fix: remove the parameter, or pass it to the exe. Update the README.

### L13. Doc drift on the patched image file name

- [ ] Location: `README.md:214` pulls to `artifacts\images\ksu-patched-init_boot.img`. `scripts/update-lineage.ps1:123` and `toolbox/src/toolbox.rs:545` use `ksu-patched-init_boot-<date>.img`.
- Fix: make the README match the scripts.

Also worth a look (not verified as defects): the default MindTheGapps and OrangeFox names are pinned in `toolbox/src/toolbox.rs:75-76` and `scripts/update-lineage.ps1:38-39`. Their latest releases were not checked in this audit.

## Not in this audit

- No `cargo test` or build was run for the full crate (it needs the theme download and a full Tauri compile). Only `parse.rs` was run standalone.
- No device, no firmware download, no flash was attempted.
- Latest versions of MindTheGapps and OrangeFox were not checked.
- Theme font URLs (L5) and the mirror's published checksums (M2) need a look at the upstream sources.
