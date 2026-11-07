// Small text parsers for adb/fastboot output, build filenames and the custom command line.
// Each mirrors a regex the Electron version used (noted above each function), without pulling in a regex engine.

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

/// The wipe commands blocked in the custom command box. Was `/(^|\s)-w(\s|$)|erase\s+userdata|format|wipe/`.
pub fn is_wipe(args: &[String]) -> bool {
    let joined = args.join(" ").to_lowercase();
    let words: Vec<&str> = joined.split_whitespace().collect();
    words.contains(&"-w") || words.join(" ").contains("erase userdata") || joined.contains("format") || joined.contains("wipe")
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
        let w = |s: &str| is_wipe(&split_line(s));
        assert!(w("-w") && w("erase   userdata") && w("format:ext4 x") && w("shell WIPE"));
        assert!(!w("getvar current-slot") && !w("-wx") && !w("devices -l"));
    }

    #[test]
    fn misc() {
        assert_eq!(vermagic(b"xx\0vermagic=6.6.89-android15-8 SMP\0more").as_deref(), Some("vermagic=6.6.89-android15-8 SMP"));
        assert_eq!(vermagic(b"none"), None);
        assert_eq!(env_line(r#"DODGE_DATA_ROOT = "X:\Dodge" "#), Some(("DODGE_DATA_ROOT".into(), r"X:\Dodge".into())));
        assert_eq!(env_line("# DODGE_PLATFORM_TOOLS="), None);
        assert_eq!(env_line("bad key=1"), None);
    }
}
