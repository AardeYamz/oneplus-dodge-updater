---
name: update-lineageos
description: Update the OnePlus 13 (CPH2655, codename dodge) to the latest LineageOS nightly and restore MindTheGapps, OrangeFox and KernelSU-Next. Use when asked to update, upgrade or flash the phone's LineageOS.
---

# Update LineageOS on dodge

Source of truth: "Manual flash" in `docs/update-flow.md`. The scripted version is `scripts/update-lineage.ps1`.

## Before flashing

1. `adb devices -l` must show `device`. If `unauthorized`, ask the user to accept the USB debugging prompt (or revoke authorizations and replug).
2. Read the current build: `adb shell getprop ro.lineage.version`, and the kernel with `uname -r`.
3. Latest build: `curl -s https://download.lineageos.org/api/v1/dodge/nightly/x` (the WebFetch tool gets 403; use curl). Fields: `filename`, `url`, `id` (SHA-256), `size`, `datetime`.
4. Verify the zip's SHA-256 against `id` before flashing.
5. **Confirm with the user before flashing** (state the current build, target build and plan). Same-major updates keep data. For a major jump, check the device wiki for a wipe and ask about a backup.

## Flash sequence (needs the user to tap on the phone)

Use the current Android SDK Platform-Tools `adb`/`fastboot`. Old adb (1.0.39) fails on the 3 GB zip, so check `adb --version` if a sideload fails.

1. `adb reboot bootloader`, then `fastboot flash recovery <build>\recovery.img` (LineageOS Recovery, 104857600 bytes), then `fastboot reboot recovery`.
2. User: Apply update -> Apply from ADB. Wait for `sideload` in `adb devices`, then `adb sideload <lineage zip>`. Stopping at ~47% with `Total xfer: 1.00x` is normal.
3. User: Advanced -> Reboot to recovery (this switches slots), then Apply from ADB again. **Ask the user to confirm the slot switch**: `sideload` showing up can be the old session. Then sideload `artifacts\mindthegapps\MindTheGapps-17.0.0-...zip`, answering Yes to the signature warning.
4. User: Reboot system now. Wait for `sys.boot_completed`. Check `ro.lineage.version`.
5. OrangeFox last: extract `artifacts\orangefox\OrangeFox-R12.0_2-dodge.zip` to `artifacts\fox-extract\` (recovery.img must be 104857600 bytes), then `fastboot flash recovery` it. Use `flash`, not `fastboot boot`, which fails with `usb_read failed (31)`.
6. KernelSU-Next: the update replaces `init_boot`, so root is lost until it is re-patched.
   - `adb push <build>\init_boot.img /sdcard/Download/`.
   - User patches it in the manager (`com.rifsxd.ksunext`): Install -> Select and patch a file. **The KMI must be set manually to `android15-6.6`**; the manager auto-picks `android17-6.18`, which won't load on the 6.6 kernel.
   - `adb pull` the `kernelsu_next_patched_*.img`. Check that the `vermagic=` string in it reads `6.6.x`.
   - `fastboot flash init_boot <patched>`, then `fastboot reboot`. The manager should show Working.
   - Rollback: `fastboot flash init_boot <build>\init_boot.img`.

## Never

- Run `fastboot -w`, `fastboot erase userdata` or "Format data" without the user asking for a wipe.
- Flash before the user has confirmed.

## Folder layout

All downloads live under `artifacts\` (gitignored). `artifacts\builds\<major>-<date>\` (e.g. `24-20261027`) holds `recovery.img`, `init_boot.img` and `build-manifest.xml` from `https://mirrorbits.lineageos.org/full/dodge/<date>/`. `artifacts\lineage\` holds the ROM zips, `mindthegapps\` the GApps zips, `orangefox\` the Fox zips, `kernelsu-next\` the manager APKs, `images\` the patched init_boot images.

## Script

`powershell -File scripts\update-lineage.ps1 [-Build yyyyMMdd]` runs the whole sequence with a pause for each phone tap and a y/N check before it starts. Last verified by hand on 2026-10-27 (24.0-20261011 -> 24.0-20261027). The script itself has not been run end to end.
