# Dodge Toolbox

A small Electron app for Windows that runs the steps in `../docs/flash-instructions.md` (the same sequence as `../scripts/update-lineage.ps1`) one button at a time, with a live log.

## Run

Needs Node.js 20+.

```powershell
cd toolbox
npm install
npm start
```

To build a single portable `.exe`: `npm run dist` (output in `toolbox\dist\`). `scripts\build-toolbox.ps1` does this and copies the exe to the repo root. Keep the exe there so it finds `artifacts\` and `scripts\`, or set the working folder in Settings.

## Use

1. Open **Settings** and check the platform-tools folder. The header shows the adb version and warns if the old Essential ADB (1.0.39) is being used.
2. Click **Refresh** to see the phone's state (`device`, `sideload`, `fastboot`...), build, kernel and slot.
3. Run the steps top to bottom. Yellow boxes say what to tap on the phone *before* clicking Run. Steps that reboot or flash ask first.

**Cancel** stops the running command, download or wait. **Quick actions** has reboot buttons, a box for one-off `adb`/`fastboot` commands, and a button that opens `scripts\update-lineage.ps1` in a PowerShell window.

Wipe commands (`fastboot -w`, `erase userdata`, `format`, `wipe`) are refused in the command box.

Not yet run end to end against the phone.
