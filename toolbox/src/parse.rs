// Small text parsers for adb/fastboot output, build filenames and the custom command line.
// Those ported from the Electron version note the regex they mirror; no regex engine is pulled in.

use serde_json::Value;

/// `adb devices`: the state of the first attached device. Was `/^\S+\t(\w+)\s*$/m`.
pub fn adb_state(out: &str) -> Option<String> {
    out.lines().find_map(|line| {
        let (serial, state) = line.trim_end().split_once('\t')?;
        let ok = !serial.is_empty()
            && !serial.chars().any(char::is_whitespace)
            && !state.is_empty()
            && state.chars().all(|c| c.is_alphanumeric() || c == '_');
        ok.then(|| state.to_string())
    })
}

/// `fastboot devices`: is a device listed in fastboot mode? Was `/^\S+\s+fastboot\s*$/m`.
pub fn is_fastboot(out: &str) -> bool {
    out.lines().any(|line| {
        let starts_clean = !line.starts_with(char::is_whitespace);
        let mut t = line.split_whitespace();
        starts_clean && t.next().is_some() && t.next() == Some("fastboot") && t.next().is_none()
    })
}

/// `adb --version`: "Version 36.0.0-13206524". Was `/Version [\d.]+-?\S*/`.
pub fn adb_version(out: &str) -> Option<String> {
    out.match_indices("Version ").find_map(|(i, _)| {
        let rest = &out[i + 8..];
        if !rest.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
            return None;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some(format!("Version {}", &rest[..end]))
    })
}

/// "lineage-23.0-20261105-nightly-dodge-signed.zip" -> ("23", "20261105"). Was `/lineage-(\d+)\.(\d+)-(\d{8})/`.
pub fn lineage_version(filename: &str) -> Option<(String, String)> {
    fn digits(s: &str) -> usize {
        s.bytes().take_while(u8::is_ascii_digit).count()
    }
    filename.match_indices("lineage-").find_map(|(i, _)| {
        let s = &filename[i + 8..];
        let major = digits(s);
        let s2 = s[major..].strip_prefix('.')?;
        let minor = digits(s2);
        let s3 = s2[minor..].strip_prefix('-')?;
        (major > 0 && minor > 0 && digits(s3) >= 8).then(|| (s[..major].to_string(), s3[..8].to_string()))
    })
}

/// Splits a command line on whitespace, keeping "quoted parts" together and dropping their quotes.
/// Was `line.match(/"[^"]*"|\S+/g).map((s) => s.replace(/^"|"$/g, ''))`.
pub fn split_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return out;
        }
        let quoted = rest.strip_prefix('"').and_then(|r| r.find('"')).map(|j| j + 2);
        let end = quoted.unwrap_or_else(|| rest.find(char::is_whitespace).unwrap_or(rest.len()));
        let tok = &rest[..end];
        let tok = tok.strip_prefix('"').unwrap_or(tok);
        out.push(tok.strip_suffix('"').unwrap_or(tok).to_string());
        rest = &rest[end..];
    }
}

/// The command box takes only the commands listed here. A list of blocked words cannot keep up with every way to wipe
/// or brick a phone (`flash userdata`, `-w`, `flashing lock`, `shell rm`), so anything not listed is refused.
/// Returns true when the command flashes, so the caller must check the device first.
pub fn check_custom(tool: &str, args: &[String]) -> Result<bool, String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    // A value the user chooses must not be read as an option.
    let value = |s: &str| !s.is_empty() && !s.starts_with('-');
    let name = |s: &str| value(s) && s.chars().all(|c| c.is_ascii_alphanumeric() || "._:-".contains(c));
    let on_sdcard = |s: &str| s.starts_with("/sdcard/") && !s.split('/').any(|part| part == "..");
    let ok = match (tool, a.as_slice()) {
        ("adb", ["devices"] | ["devices", "-l"] | ["version"] | ["--version"] | ["get-state"] | ["get-serialno"]) => true,
        ("adb", ["logcat", "-d"] | ["reboot"]) => true,
        ("adb", ["reboot", "bootloader" | "recovery" | "sideload" | "sideload-auto-reboot" | "fastboot"]) => true,
        // `adb shell` joins its arguments into a shell line, so each one is checked for shell characters.
        ("adb", ["shell", "getprop"] | ["shell", "uname"] | ["shell", "uname", "-a" | "-r"]) => true,
        ("adb", ["shell", "getprop", prop]) => name(prop),
        ("adb", ["push", from, to]) => value(from) && on_sdcard(to),
        ("adb", ["pull", from]) => value(from),
        ("adb", ["pull", from, to]) => value(from) && value(to),
        ("adb", ["install", apk] | ["install", "-r", apk] | ["sideload", apk]) => value(apk),
        ("fastboot", ["devices"] | ["devices", "-l"] | ["--version"] | ["reboot"]) => true,
        ("fastboot", ["reboot", "bootloader" | "recovery" | "fastboot"]) => true,
        ("fastboot", ["getvar", var]) => name(var),
        ("fastboot", ["flash", "recovery" | "init_boot", image]) if value(image) => return Ok(true),
        _ => false,
    };
    if ok {
        return Ok(false);
    }
    Err("Not allowed here. Allowed: adb devices, version, get-state, get-serialno, logcat -d, reboot, shell getprop, shell uname, \
         push <file> /sdcard/..., pull, install, sideload; fastboot devices, getvar, reboot, flash recovery|init_boot <image>."
        .into())
}

/// Whether `adb sideload` finished. Its exit code is not enough: it can be non-zero after a good install. Accepted: the
/// "Total xfer" line, or the stop the LineageOS wiki calls normal (47% or later, "failed to read command: Success").
pub fn sideload_ok(out: &str) -> bool {
    if out.contains("Total xfer:") {
        return true;
    }
    let percent = out.rmatch_indices("(~").find_map(|(i, _)| out[i + 2..].split_once("%)")?.0.parse::<u32>().ok());
    let benign = ["Success", "No error", "Undefined error: 0"]
        .iter()
        .any(|e| out.contains(&format!("failed to read command: {e}")));
    benign && percent.is_some_and(|p| p >= 47)
}

/// One downloadable file of a build, as published by the LineageOS API.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteFile {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// A nightly with every file the update downloads, each with its published SHA-256.
#[derive(Clone, Debug)]
pub struct Release {
    pub rom: RemoteFile,
    pub images: Vec<RemoteFile>, // in the order of IMAGES
}

pub const IMAGES: [&str; 3] = ["recovery.img", "init_boot.img", "build-manifest.xml"];

/// Picks the newest build (or the one dated `date`, yyyyMMdd) from `/api/v2/devices/<device>/builds`.
pub fn pick_release(builds: &Value, date: &str) -> Result<Release, String> {
    let file = |f: &Value| {
        let s = |k: &str| f[k].as_str().unwrap_or_default().to_string();
        let r = RemoteFile { filename: s("filename"), url: s("url"), sha256: s("sha256").to_lowercase(), size: f["size"].as_u64().unwrap_or(0) };
        let hashed = r.sha256.len() == 64 && r.sha256.bytes().all(|b| b.is_ascii_hexdigit());
        (hashed && !r.url.is_empty()).then_some(r)
    };
    let needle = format!("-{date}-");
    let mut list: Vec<&Value> = builds.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    list.sort_by_key(|b| std::cmp::Reverse(b["datetime"].as_u64().unwrap_or(0)));
    for b in list {
        let files = b["files"].as_array().map(Vec::as_slice).unwrap_or_default();
        let Some(rom) = files.iter().find(|f| {
            f["filename"].as_str().is_some_and(|n| n.starts_with("lineage-") && n.ends_with("-signed.zip") && (date.is_empty() || n.contains(&needle)))
        }) else {
            continue;
        };
        let rom = file(rom).ok_or("The API lists no usable SHA-256 for the ROM zip.")?;
        let images = IMAGES
            .iter()
            .map(|&want| {
                files
                    .iter()
                    .find(|f| f["filename"].as_str() == Some(want))
                    .and_then(file)
                    .ok_or_else(|| format!("The API lists no usable SHA-256 for {want}."))
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Release { rom, images });
    }
    Err("Build not found in the API response.".into())
}

/// The kernel's "vermagic=6.6.x-..." string in a boot image (up to 40 bytes, stops at NUL). Was `/vermagic=[^\x00]{0,40}/`.
pub fn vermagic(data: &[u8]) -> Option<String> {
    let i = data.windows(9).position(|w| w == b"vermagic=")?;
    let tail = &data[i..];
    let n = tail[9..].iter().take(40).take_while(|&&b| b != 0).count();
    Some(tail[..9 + n].iter().map(|&b| b as char).collect()) // latin1, like the JS version
}

/// One `.env` line: KEY=value, optional quotes, `#` comments. Was `/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/i`.
pub fn env_line(line: &str) -> Option<(String, String)> {
    if line.trim().starts_with('#') {
        return None;
    }
    let (k, v) = line.split_once('=')?;
    let k = k.trim();
    if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let v = v.trim();
    let unq = [b'"', b'\''].iter().find_map(|&q| {
        let b = v.as_bytes();
        (b.len() >= 2 && b[0] == q && b[b.len() - 1] == q).then(|| &v[1..v.len() - 1])
    });
    Some((k.to_string(), unq.unwrap_or(v).to_string()))
}

/// Number of devices in `adb devices` / `fastboot devices` output (lines of `serial<TAB or spaces>state`, header and blanks skipped).
pub fn device_count(out: &str) -> usize {
    out.lines()
        .filter(|l| !l.starts_with(char::is_whitespace))
        .filter(|l| {
            let mut t = l.split_whitespace();
            t.next().is_some() && t.next().is_some() && !l.starts_with("List of devices") && !l.starts_with('*')
        })
        .count()
}

/// Whether `ro.product.device` (adb) or the bootloader's `product` (fastboot) belongs to a OnePlus 13. The phone
/// reports its OnePlus codename (the OTA's pre-device list) over adb, and the SoC board "sun" in the bootloader,
/// which other SM8750 phones share.
pub fn is_dodge(product: &str, fastboot: bool) -> bool {
    matches!(product, "dodge" | "OP5D55L1" | "OP5D0DL1") || (fastboot && product == "sun")
}

/// `fastboot getvar product` prints "product: sun" (to stderr) followed by "Finished. Total time: ...".
pub fn fastboot_product(out: &str) -> Option<String> {
    out.lines().find_map(|l| l.trim().strip_prefix("product:").map(|v| v.trim().to_string())).filter(|v| !v.is_empty())
}

/// The patched image KernelSU-Next wrote for this build: the newest candidate from `stat -c '%Y %n'` lines whose
/// mtime is at or after `pushed_at` (device clock, seconds). Older leftovers from earlier builds are refused.
pub fn newest_patched(listing: &str, pushed_at: u64) -> Result<String, String> {
    let mut all = 0;
    let mut best: Option<(u64, &str)> = None;
    for line in listing.lines() {
        let Some((mtime, path)) = line.trim().split_once(' ') else { continue };
        let (Ok(mtime), path) = (mtime.parse::<u64>(), path.trim()) else { continue };
        if path.is_empty() {
            continue;
        }
        all += 1;
        if mtime >= pushed_at && best.is_none_or(|(m, _)| mtime > m) {
            best = Some((mtime, path));
        }
    }
    match best {
        Some((_, path)) => Ok(path.to_string()),
        None if all == 0 => Err("No kernelsu_next_patched_*.img in /sdcard/Download.".into()),
        None => Err(format!(
            "Only kernelsu_next_patched_*.img files older than this build's init_boot push were found ({all}). Patch the pushed init_boot again."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adb() {
        assert_eq!(adb_state("List of devices attached\r\nabc123\tdevice\r\n\r\n").as_deref(), Some("device"));
        assert_eq!(adb_state("List of devices attached\nabc\tsideload\n").as_deref(), Some("sideload"));
        assert_eq!(adb_state("List of devices attached\n\n"), None);
        assert!(is_fastboot("abc123\t fastboot\n"));
        assert!(!is_fastboot("< waiting for any device >\n"));
        assert_eq!(adb_version("Android Debug Bridge version 1.0.41\nVersion 36.0.0-13206524\nInstalled as x").as_deref(), Some("Version 36.0.0-13206524"));
        assert_eq!(adb_version("nothing"), None);
    }

    #[test]
    fn filenames() {
        assert_eq!(lineage_version("lineage-23.0-20261105-nightly-dodge-signed.zip"), Some(("23".into(), "20261105".into())));
        assert_eq!(lineage_version("lineage-x.zip"), None);
    }

    #[test]
    fn command_line() {
        assert_eq!(split_line(r#"adb push "C:\a b\x.img" /sdcard"#), ["adb", "push", r"C:\a b\x.img", "/sdcard"]);
        assert_eq!(split_line(r#"  fastboot  getvar "unterminated  "#), ["fastboot", "getvar", "unterminated"]);
        assert_eq!(split_line(""), Vec::<String>::new());
    }

    #[test]
    fn custom_commands() {
        let check = |line: &str| {
            let parts = split_line(line);
            check_custom(&parts[0], &parts[1..])
        };
        for line in [
            "adb devices -l", "adb reboot", "adb reboot recovery", "adb shell getprop ro.product.device", "adb shell uname -r",
            "adb push x.img /sdcard/Download/", "adb pull /sdcard/Download/x.img", "adb install -r m.apk", "adb sideload rom.zip",
            "fastboot getvar current-slot", "fastboot reboot bootloader",
        ] {
            assert_eq!(check(line), Ok(false), "{line}");
        }
        assert_eq!(check("fastboot flash recovery r.img"), Ok(true));
        assert_eq!(check("fastboot flash init_boot i.img"), Ok(true));
        for line in [
            "fastboot flash userdata x.img", "fastboot flash metadata x.img", "fastboot flash super x.img", "fastboot flash cache x.img",
            "fastboot flash:raw userdata x.img", "fastboot --slot=all flash recovery r.img", "fastboot flash recovery -w",
            "fastboot flash recovery r.img -w", "fastboot -w", "fastboot --wipe", "fastboot erase userdata", "fastboot erase metadata",
            "fastboot format userdata", "fastboot format:ext4 metadata", "fastboot wipe-super s.img", "fastboot update x.zip",
            "fastboot flashall", "fastboot flashing lock", "fastboot flashing unlock", "fastboot oem lock",
            "fastboot delete-logical-partition system", "adb shell rm -rf /sdcard", "adb shell recovery --wipe_data",
            "adb shell getprop x;reboot", "adb shell getprop $(reboot)", "adb shell uname -r && reboot",
            "adb push x.img /dev/block/by-name/userdata", "adb push x.img /sdcard/../dev/block/x", "adb root", "adb -s abc reboot",
            "adb uninstall com.example",
        ] {
            assert!(check(line).is_err(), "{line} must be refused");
        }
    }

    #[test]
    fn sideload_output() {
        assert!(sideload_ok("serving: 'rom.zip'  (~99%)\r\nTotal xfer: 1.00x\n"));
        assert!(sideload_ok("serving: 'rom.zip'  (~47%)    adb: failed to read command: Success\n"));
        assert!(sideload_ok("serving: 'rom.zip'  (~46%)\rserving: 'rom.zip'  (~94%)\radb: failed to read command: No error"));
        assert!(!sideload_ok("serving: 'rom.zip'  (~12%)    adb: failed to read command: Success\n"));
        assert!(!sideload_ok("serving: 'rom.zip'  (~60%)    adb: failed to read command: Connection reset by peer\n"));
        assert!(!sideload_ok("adb: sideload connection failed: no devices/emulators found\n"));
        assert!(!sideload_ok(""));
    }

    #[test]
    fn releases() {
        let hash = "a".repeat(64);
        let f = |n: &str| serde_json::json!({ "filename": n, "url": format!("https://x/{n}"), "sha256": hash, "size": 1 });
        let build = |t: u64, d: &str| {
            let mut files = vec![f(&format!("lineage-24.0-{d}-nightly-dodge-signed.zip"))];
            files.extend(IMAGES.iter().map(|n| f(n)));
            serde_json::json!({ "datetime": t, "files": files })
        };
        let mut api = serde_json::json!([build(1, "20261110"), build(2, "20261117")]);
        assert!(pick_release(&api, "").unwrap().rom.filename.contains("20261117"));
        let old = pick_release(&api, "20261110").unwrap();
        assert!(old.rom.filename.contains("20261110"));
        assert_eq!(old.images.iter().map(|i| i.filename.as_str()).collect::<Vec<_>>(), IMAGES);
        assert!(pick_release(&api, "20260101").is_err());
        api[1]["files"][2]["sha256"] = "".into(); // init_boot.img without a hash: refuse rather than skip the check
        assert!(pick_release(&api, "").unwrap_err().contains("init_boot.img"));
    }

    #[test]
    fn misc() {
        assert_eq!(vermagic(b"xx\0vermagic=6.6.89-android15-8 SMP\0more").as_deref(), Some("vermagic=6.6.89-android15-8 SMP"));
        assert_eq!(vermagic(b"none"), None);
        assert_eq!(env_line(r#"DODGE_DATA_ROOT = "X:\Dodge" "#), Some(("DODGE_DATA_ROOT".into(), r"X:\Dodge".into())));
        assert_eq!(env_line("# DODGE_PLATFORM_TOOLS="), None);
        assert_eq!(env_line("bad key=1"), None);
    }

    #[test]
    fn identity() {
        assert!(is_dodge("OP5D55L1", false) && is_dodge("OP5D0DL1", false) && is_dodge("sun", true));
        assert!(!is_dodge("sun", false) && !is_dodge("pixel", true) && !is_dodge("", true));
    }

    #[test]
    fn guards() {
        assert_eq!(device_count("List of devices attached\nabc\tdevice\ndef\toffline\n\n"), 2);
        assert_eq!(device_count("List of devices attached\n\n"), 0);
        assert_eq!(device_count("* daemon started successfully\nList of devices attached\nabc\tdevice\n"), 1);
        assert_eq!(device_count("abc123\tfastboot\n"), 1);
        assert_eq!(fastboot_product("product: dodge\nFinished. Total time: 0.001s\n").as_deref(), Some("dodge"));
        assert_eq!(fastboot_product("product: \nFinished.\n"), None);
        assert_eq!(fastboot_product("< waiting for any device >"), None);
    }

    #[test]
    fn patched_image() {
        let l = "100 /sdcard/Download/kernelsu_next_patched_a.img\n250 /sdcard/Download/kernelsu_next_patched_b.img\n";
        assert_eq!(newest_patched(l, 200).as_deref(), Ok("/sdcard/Download/kernelsu_next_patched_b.img"));
        assert_eq!(newest_patched(l, 250).as_deref(), Ok("/sdcard/Download/kernelsu_next_patched_b.img"));
        assert!(newest_patched(l, 251).unwrap_err().contains("older"));
        assert!(newest_patched("", 0).unwrap_err().starts_with("No kernelsu"));
        assert!(newest_patched("stat: no such file\n", 0).is_err());
        assert_eq!(newest_patched("300 /sdcard/Download/a b.img\n", 1).as_deref(), Ok("/sdcard/Download/a b.img"));
    }
}
