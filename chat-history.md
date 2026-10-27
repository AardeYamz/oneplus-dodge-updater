# Chat history: Flashing LineageOS 24 on OnePlus 13 (dodge)

Date: 2026-10-17

---

## 1. User

> i need help reflashing my recovery image using fastboot. what is the command i should use? do not commit anything

### Assistant

Here's the basic command:

```
fastboot flash recovery recovery.img
```

**Full steps:**

1. Reboot the device into the bootloader:
   ```
   adb reboot bootloader
   ```
   You can also power it off and hold the device's key combo, which is often Power + Volume Down.

2. Check that fastboot can see it:
   ```
   fastboot devices
   ```

3. Flash the image. Use the full path if the `.img` isn't in your current folder:
   ```
   fastboot flash recovery "C:\path\to\recovery.img"
   ```

4. Boot straight into recovery. If you boot normally first, some stock ROMs overwrite a custom recovery:
   ```
   fastboot reboot recovery
   ```
   If that doesn't work on your fastboot version, use the device's key combo instead.

**Things that vary by device:**

- **A/B devices** (most phones from about 2019 on, like Pixels) often have **no separate recovery partition**. Recovery lives inside `boot` or `vendor_boot`, and `fastboot flash recovery` fails with something like "partition does not exist." On those devices:
  - To try a recovery without flashing it: `fastboot boot recovery.img`
  - To install it, follow your device's guide. That usually means `fastboot flash boot` or `vendor_boot`, or using the recovery's own installer.
  - To target a slot: `fastboot flash recovery_a recovery.img` or use `--slot=all`.
- **The bootloader must be unlocked** (`fastboot flashing unlock`, or `fastboot oem unlock` on older devices). **Unlocking wipes the device.**
- **The image must be built for your exact model.** Flashing the wrong one can leave the device unable to boot.

If you tell me your device model, I can give you the exact steps for it. I didn't run or commit anything, and this folder isn't a git repo anyway.

---

## 2. User

> https://download.lineageos.org/devices/dodge/builds

### Assistant

*(The LineageOS download and wiki pages returned 403 Forbidden, so the device's install settings were read from the wiki's source file on GitHub: `LineageOS/lineage_wiki/_data/devices/dodge.yml`.)*

"dodge" is the **OnePlus 13**. The LineageOS pages you linked returned "403 Forbidden" to me, so I read the device's install settings from the LineageOS wiki's source file on GitHub instead. That file says to flash four extra partitions and `vbmeta` before you flash recovery. Recovery alone isn't enough.

**1. Download these images from the latest build on that page.** They must all come from the **same build date**:
- `boot.img`
- `dtbo.img`
- `init_boot.img`
- `vbmeta.img`
- `vendor_boot.img`
- `recovery.img`

**2. Get into the bootloader.** Either power the phone off and hold **Volume Up + Volume Down + Power**, or run:
```
adb reboot bootloader
fastboot devices
```
If the bootloader is still locked, run `fastboot flashing unlock`. **This wipes the phone.**

**3. Flash the partitions, then recovery.** Run these from the folder holding the images:
```
fastboot flash boot boot.img
fastboot flash dtbo dtbo.img
fastboot flash init_boot init_boot.img
fastboot flash vbmeta vbmeta.img
fastboot flash vendor_boot vendor_boot.img
fastboot flash recovery recovery.img
```

**4. Boot into recovery.** On this phone you do it from the bootloader menu: use the volume keys to choose **Recovery mode**, then press Power.

**Notes:**
- **Firmware:** the wiki says this device needs firmware based on Android 16 before you install LineageOS. If the phone has been on an older OxygenOS, update it first.
- **Images:** don't mix files from different builds, and don't use images made for another model. The OnePlus 13 models listed are CPH2649, CPH2653, CPH2655 and PJZ110.
- **Errors:** if any `fastboot flash` command fails, stop there and send me the error rather than continuing.

I didn't run or commit anything.

---

## 3. User

> i already have data in my phone. what command should i do to not lose any data?

### Assistant

The flash commands themselves don't erase your data. `fastboot flash` only writes the one partition you name, and none of the commands I gave touch `userdata`. Whether you keep your data depends on what the phone is running now.

**If the phone already runs LineageOS and you're only reflashing recovery, your data is safe.** Use images from the **same build date as the LineageOS version installed now**. You can find it under Settings → About phone → LineageOS version. Mixing builds can stop the phone from booting.
```
fastboot flash boot boot.img
fastboot flash dtbo dtbo.img
fastboot flash init_boot init_boot.img
fastboot flash vbmeta vbmeta.img
fastboot flash vendor_boot vendor_boot.img
fastboot flash recovery recovery.img
```

**If the phone runs stock OxygenOS, two steps will wipe it whatever command you use:**
- **Unlocking the bootloader.** `fastboot flashing unlock` always erases the phone if the bootloader is still locked, and nothing gets around that.
- **Switching from OxygenOS to LineageOS.** The install requires a factory reset or format in recovery, because LineageOS can't read OxygenOS's encrypted data.

In that case, back up first: photos to Google Photos or a PC, WhatsApp's built-in backup, and anything else you need off the phone.

**To avoid wiping by accident:**
- Don't run `fastboot -w`, `fastboot erase userdata` or `fastboot format userdata`.
- Don't run `fastboot flashing unlock` again if the bootloader is already unlocked.
- In recovery, don't tap "Factory reset" or "Format data."

Is the phone on LineageOS already, or still on OxygenOS? That decides whether you can keep your data.

---

## 4. User

> ok that recovered it ... how do i flash the lineage os 24 image?

### Assistant

Glad recovery is working. LineageOS itself goes on as a `.zip` that you send from your PC into recovery with `adb sideload`. You don't use `fastboot` for this step.

**1. Download the zip.** Get `lineage-24.0-<date>-nightly-dodge-signed.zip` from the same build page. It should be the **same build date** as the recovery and other images you just flashed.

**2. Only if you're coming from OxygenOS: wipe first.** In LineageOS Recovery, tap **Factory reset → Format data / factory reset**. This erases the phone, as I warned before, so back up first.
If you're already on LineageOS and just upgrading, **skip this step**. That keeps your data, but any GApps you had must be reinstalled in their Android 16 version (step 5).

**3. Start sideload mode on the phone.** In recovery, tap **Apply update → Apply from ADB**.

**4. Send the zip from your PC:**
```
adb devices
adb sideload "C:\path\to\lineage-24.0-XXXXXXXX-nightly-dodge-signed.zip"
```
`adb devices` should list the phone as `sideload`. The transfer often stops around **47%** and then says something like `Total xfer: 1.00x`. That's **normal** and means it worked. Wait until the phone itself says the install finished.

**5. Optional: install GApps now.** If you want Google apps, do it **before the first boot**. Go back to the main menu, tap **Advanced → Reboot to recovery**, then sideload the GApps zip the same way. Use a package built for **Android 16**. Recovery will say the zip's signature can't be verified; choosing **Yes** is expected for GApps.

**6. Reboot.** Tap **Reboot system now**. The first boot can take several minutes.

If the sideload or first boot fails, send me the exact error or what's on screen.

---

## 5. User

```
PS C:\Users\Yannis Lam\Desktop\dodge> adb sideload ".\lineage-24.0-20261011-nightly-dodge-signed.zip"
opening '.\lineage-24.0-20261011-nightly-dodge-signed.zip'...
connecting...
* failed to read command: No error
PS C:\Users\Yannis Lam\Desktop\dodge> adb sideload ".\lineage-24.0-20261011-nightly-dodge-signed.zip"
opening '.\lineage-24.0-20261011-nightly-dodge-signed.zip'...
connecting...
falling back to older sideload method...
terminate called after throwing an instance of 'std::bad_alloc'
  what():  std::bad_alloc
```

### Assistant

*(Checked locally: `adb version` reported 1.0.39 at `C:\Program Files (x86)\Essential\ADB\adb.exe`; the zip is 3,182,109,825 bytes; `adb devices` listed nothing.)*

The problem is your **adb, which is far too old**. It's version **1.0.39** from about 2017, a 32-bit build installed at `C:\Program Files (x86)\Essential\ADB\`. Your LineageOS zip is **3.18 GB**, which is more than that adb can handle:

- **First error** (`failed to read command`): the transfer broke as soon as it started, and the phone dropped out of sideload mode.
- **Second error** (`falling back to older sideload method` → `bad_alloc`): the phone was no longer in sideload mode, so adb tried an older method. That method loads the whole 3 GB file into memory and crashed.

`adb devices` now shows nothing, so the phone has left sideload mode as well.

**To fix it:**

1. **Get current platform-tools.** Download "SDK Platform-Tools for Windows" from Google at https://developer.android.com/tools/releases/platform-tools and extract it to, say, `C:\platform-tools`.

2. **Restart sideload mode on the phone.** In recovery, tap **Apply update → Apply from ADB** again. You need to do this before every attempt.

3. **Run the new adb by its full path,** so Windows doesn't pick up the old one:
   ```
   C:\platform-tools\adb.exe kill-server
   C:\platform-tools\adb.exe version
   C:\platform-tools\adb.exe devices
   C:\platform-tools\adb.exe sideload ".\lineage-24.0-20261011-nightly-dodge-signed.zip"
   ```
   `version` should report something recent (35.x or newer). `devices` should list the phone as `sideload`.

**If it still fails:**
- Plug straight into a USB port on the PC with a good data cable. Avoid hubs and front-panel ports.
- Read what the phone's screen says after the failure, since recovery usually shows the real reason there.
- Check the zip didn't get corrupted while downloading. Run `Get-FileHash .\lineage-24.0-20261011-nightly-dodge-signed.zip` and compare the result to the SHA256 on the build page.

You can also uninstall the old "Essential ADB" later so it doesn't get picked up again. Your earlier `fastboot` commands probably came from the same old folder. They worked, but use the new `fastboot.exe` from platform-tools from now on.

---

## 6. User

```
PS C:\Users\Yannis Lam\Desktop\dodge\platform-tools> adb.exe sideload "..\lineage-24.0-20261011-nightly-dodge-signed.zip"
opening '..\lineage-24.0-20261011-nightly-dodge-signed.zip'...
connecting...
* failed to read command: No error
PS C:\Users\Yannis Lam\Desktop\dodge\platform-tools> adb.exe sideload "..\lineage-24.0-20261011-nightly-dodge-signed.zip"
opening '..\lineage-24.0-20261011-nightly-dodge-signed.zip'...
connecting...
falling back to older sideload method...
terminate called after throwing an instance of 'std::bad_alloc'
  what():  std::bad_alloc
PS C:\Users\Yannis Lam\Desktop\dodge\platform-tools> adb.exe sideload ".\lineage-24.0-20261011-nightly-dodge-signed.zip"
opening '.\lineage-24.0-20261011-nightly-dodge-signed.zip'...
failed to stat file .\lineage-24.0-20261011-nightly-dodge-signed.zip: No such file or directory
```

> similar error?

### Assistant

*(Checked locally: `adb.exe` still resolved to the old Essential ADB; the new `platform-tools\adb.exe` is version 37.0.1 and replaced the old adb server when run.)*

It's the same error because the same old adb ran again. In PowerShell, typing `adb.exe` doesn't run the copy in your current folder. PowerShell only searches your PATH, so it picked up the old one again. I checked:

- `adb.exe` resolves to `C:\Program Files (x86)\Essential\ADB\adb.exe`, the old 1.0.39.
- `.\adb.exe` in your `platform-tools` folder is the new one, version 37.0.1.

When I ran the new adb, it found the old adb server still running, stopped it and started its own. So you don't need `kill-server` now.

Your third error was a separate path mistake. The zip is in `dodge`, the folder above `platform-tools`, so `.\` couldn't find it. `..\` is the right path.

**Try again:**

1. On the phone, tap **Apply update → Apply from ADB**. `adb devices` currently shows nothing, so this is needed.
2. In your `platform-tools` folder, run:
   ```
   .\adb.exe devices
   .\adb.exe sideload "..\lineage-24.0-20261011-nightly-dodge-signed.zip"
   ```
   `devices` should list the phone as `sideload` before you run the second command.

Remember the `.\` before `adb.exe` and `fastboot.exe` every time. Uninstalling Essential ADB would stop this from catching you again. If it still fails, tell me what the phone's screen says.

---

## 7. User

```
PS C:\Users\Yannis Lam\Desktop\dodge\platform-tools> fastboot devices
fastboot : The term 'fastboot' is not recognized as the name of a cmdlet, function, script file, or operable program.
...
Suggestion [3,General]: The command fastboot was not found, but does exist in the current location. Windows PowerShell does not load commands from the current location by default. If you trust this command, instead type: ".\fastboot".
```

> can we reinstall fastboot?

### Assistant

You don't need to reinstall anything. `fastboot.exe` is already in your `platform-tools` folder, and PowerShell's own suggestion at the bottom of the error says so. It's the same issue as with adb: PowerShell won't run programs from the current folder unless you put `.\` in front. Run:

```
.\fastboot.exe devices
```

**To type plain `adb` and `fastboot` again**, put the new folder first on your PATH for this PowerShell window:
```
$env:Path = "C:\Users\Yannis Lam\Desktop\dodge\platform-tools;" + $env:Path
```
After that, `adb` and `fastboot` run the new versions in that window. It resets when you close the window. The permanent fix is to uninstall Essential ADB and add the `platform-tools` folder to your PATH in Windows settings.

Were you going back to fastboot because the sideload failed again? If so, paste what happened and what the phone's screen shows.

---

## 8. User

> pls extract the chat history to a md file in the directory

### Assistant

Saved this file as `chat-history.md` in `C:\Users\Yannis Lam\Desktop\dodge`.
