# Working instructions: LineageOS 24 + MindTheGapps + OrangeFox (OnePlus 13, dodge)

Status: working draft, not yet confirmed end to end.

Run everything from `C:\path\to\dodge\platform-tools` in PowerShell.
Always put `.\` in front of `adb.exe` and `fastboot.exe`. Plain `adb` picks up the old Essential ADB (1.0.39), which fails on the 3 GB zip.

Files (all builds must match):
- `..\24-20261011\lineage-24.0-20261011-nightly-dodge-signed (1).zip`
- `..\24-20261011\MindTheGapps-17.0.0-arm64-20261006_111038.zip` (use this one, not the 16.0.0 zip in `dodge\`)
- `..\24-20261011\OrangeFox-R12.0_2-dodge.zip` (use this one, not R11.3_2 in `dodge\`)

Order: LineageOS, then MindTheGapps, then OrangeFox last. The LineageOS zip can overwrite the recovery, so OrangeFox goes on last.

## 0. Prepare OrangeFox's recovery.img (once)

Extract into its own folder so it doesn't overwrite LineageOS's `recovery.img`:

```powershell
Expand-Archive "..\24-20261011\OrangeFox-R12.0_2-dodge.zip" -DestinationPath "..\fox-extract" -Force
(Get-Item "..\fox-extract\recovery.img").Length
```

The size must print `104857600`. If not, the extraction was cut short; do not flash it.

## 1. Install LineageOS (from LineageOS Recovery)

No wipe if you're upgrading from an earlier LineageOS. Wipe (Factory reset) only when coming from OxygenOS.

On the phone: **Apply update -> Apply from ADB**.

```powershell
.\adb.exe devices
.\adb.exe sideload "..\24-20261011\lineage-24.0-20261011-nightly-dodge-signed (1).zip"
```

`devices` must list the phone as `sideload`. Stopping at about 47% with `Total xfer: 1.00x` is normal. Wait for the phone to say it finished.

## 2. Switch slot

Do not reboot to system yet. On the phone: **Advanced -> Reboot to recovery**. This puts you on the slot LineageOS was installed to, so the GApps go to the right place.

## 3. Install MindTheGapps

On the phone: **Apply update -> Apply from ADB**.

```powershell
.\adb.exe devices
.\adb.exe sideload "..\24-20261011\MindTheGapps-17.0.0-arm64-20261006_111038.zip"
```

When recovery says it can't verify the signature, choose **Yes**. That's expected.

## 4. Boot into the system once

Tap **Reboot system now**. The first boot can take several minutes. Confirm LineageOS and Google apps work before continuing.

## 5. Install OrangeFox (last)

```powershell
.\adb.exe reboot bootloader
.\fastboot.exe devices
.\fastboot.exe flash recovery ..\fox-extract\recovery.img
.\fastboot.exe reboot recovery
```

This replaces LineageOS Recovery with OrangeFox and keeps your data. If it doesn't land in OrangeFox, use the volume keys in the bootloader menu to pick **Recovery mode**.

To go back to LineageOS Recovery: `.\fastboot.exe flash recovery ..\24-20261011\recovery.img`

## Known issues so far

- `fastboot boot ..\fox-extract\recovery.img` transferred the image but failed with `usb_read failed ... (31)`. Use `fastboot flash recovery` instead (step 5).
- Mind the folder name: `24-20261011` (not `24-202611`).
- Don't run `fastboot -w`, `fastboot erase userdata`, or "Format data" in recovery unless you mean to wipe the phone.
- If any step errors, stop and note the exact output and what the phone's screen shows.

## 6. Install KernelSU-Next (LKM, patch init_boot)

Latest release at time of writing: v3.4.2 (2026-10-13). Needs a GKI kernel. LineageOS must already be installed and booting.
The "installation" page on kernelsu-next.github.io is for building into kernel source; it does not apply here.

1. Check the kernel: `.\adb.exe shell uname -r`. This phone reports `6.6.144-4k-...`, so the KMI is **android15-6.6**. If the kernel is not GKI, stop.
2. Download `KernelSU_Next_v3.4.2_33355-release.apk` (not the "spoofed" one; see the note below) from
   https://github.com/KernelSU-Next/KernelSU-Next/releases/tag/v3.4.2 (saved in `..\kernelsu-next\`).
3. Install it and push the `init_boot.img` from the same LineageOS build:
   ```powershell
   .\adb.exe install "..\kernelsu-next\KernelSU_Next_v3.4.2_33355-release.apk"
   .\adb.exe push "..\24-20261011\init_boot.img" /sdcard/Download/
   ```
4. In the manager: **Install -> Select and patch a file** -> pick `init_boot.img`.
   **Choose the KMI manually: `android15-6.6`.** The manager auto-picks by Android version, so on Android 17 it chose `android17-6.18`. That module can't load on a 6.6 kernel, and the manager then says "not installed, click to install".
5. Pull the patched image (use the filename the manager shows, one per KMI you patch):
   ```powershell
   .\adb.exe pull /sdcard/Download/<patched-file>.img ..\ksu-patched-init_boot.img
   ```
   Optional check that the embedded module matches the kernel: search the image for the `vermagic=` string. The right one reads `vermagic=6.6.x-4k...`. A 6.12 or 6.18 value is the wrong KMI.
6. Flash it:
   ```powershell
   .\adb.exe reboot bootloader
   .\fastboot.exe flash init_boot ..\ksu-patched-init_boot.img
   .\fastboot.exe reboot
   ```
7. Open the manager. It should show **Working**. The module is built for 6.6.127 and the kernel is 6.6.144, and it loads fine.

Backup/undo: the original `24-20261011\init_boot.img` is the backup. To undo, flash it back with `fastboot flash init_boot`.
After every LineageOS update, `init_boot` is replaced, so repeat steps 4-6 with the new build's `init_boot.img`.

Result: KernelSU-Next v3.4.2 (LKM, android15-6.6) confirmed working on LineageOS 24.0-20261011 (2026-10-17).
Saved files in `..\kernelsu-next\`: the APK, the working patched image, and the stock backup.

Tried the "spoofed" manager APK (installs under a random package name) to see if it would hide from detectors on its own: it did not work. The kernel module (flashed in step 6) only recognizes the original manager's app signature, so the spoofed build showed "Not installed" even after re-patching and reflashing with it. Reverted to the original manager, which still shows Working. Uninstalled the spoofed APK.

## 7. Play Integrity: result so far (2026-10-17)

Checked with the Play Integrity API Checker app: **basic integrity passes. Device and strong integrity fail**, and were not pursued further — they require hardware attestation that an unlocked bootloader can't produce without a sourced keybox, which is out of scope here.

Module stack that got basic integrity passing: Zygisk Next (already installed) + Tricky Store OSS + its Addon + PlayIntegrityFork + VBMeta Disguiser, replacing the original Play Integrity Fix / Tricky Store / TEESimulator / Yurikey combo (uninstall those first — they conflict). All installed via the manager's **Module -> Install from storage**, since adb has no root on this device. After install + reboot + clearing Play Store/Services/Wallet data, `vbmeta.device_state` reads `locked`.

Whether basic integrity is enough depends on which app needs it (bank vs. Wallet vs. other) — still not confirmed.

## 8. App-level hiding (status)

Added Vector + HMA-OSS (Zygisk modules) and set up a blacklist-style app profile hiding the root manager and integrity-checker apps from a chosen set of target apps (banking/work apps). This hides the root manager from apps that query Android's installed-package list, but does **not** stop apps that check the filesystem directly — a native-detection test app still flags the root manager's package three separate ways even with the profile applied.

Net result: Play Integrity basic passes; app-level root hiding is partial and app-dependent. Confirm against the specific target app before relying on this.

## Known issues (additions)

- Wrong extraction path: the R12.0_2 zips are in `24-20261011`, not `dodge\`.
- KernelSU-Next says "not installed" after flashing: the patch used the wrong KMI (auto-picked `android17-6.18`). Re-patch with `android15-6.6` and reflash (step 6).
- adb has no root on this device (`su` is inaccessible), so KernelSU module installs/config have to be done through the manager app's UI, not adb/Termux commands.
