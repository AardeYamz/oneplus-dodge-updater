# dodge

Tooling and notes for keeping the OnePlus 13 (CPH2655, codename `dodge`) on the latest LineageOS nightly with MindTheGapps, OrangeFox and KernelSU-Next.

```
docs/        flash-instructions.md (manual steps), chat-history.md
scripts/     update-lineage.ps1 (scripted update), build-toolbox.ps1 (builds the exe)
toolbox/     Electron app that runs the update steps with a live log
artifacts/   downloads and flash images (gitignored; lives on X:Android DevelopmentDevicesDodge)
  builds/<major>-<date>/   recovery.img, init_boot.img, build-manifest.xml
  lineage/ mindthegapps/ orangefox/ kernelsu-next/ pi-modules/   zips and APKs
  fox-extract/             unzipped OrangeFox recovery.img
  images/                  patched init_boot images, loose images
  archives/                other downloaded bundles
platform-tools/             adb / fastboot (gitignored)
```

- Update the phone: `powershell -File scripts\update-lineage.ps1`, or open the toolbox.
- Build the toolbox exe: `pwsh scripts\build-toolbox.ps1` (copies `Dodge Toolbox <version>.exe` to the repo root).
- See `toolbox/README.md` for the app and `docs/flash-instructions.md` for the manual sequence.
