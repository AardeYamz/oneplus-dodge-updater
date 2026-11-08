// Everything that touches the phone, the network or the disk (main.js in the Electron version).
// Steps run on a worker thread; output reaches the UI as `log` and `progress` events.
use crate::parse;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        mpsc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

type R<T> = Result<T, String>;
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const RECOVERY_SIZE: u64 = 104_857_600;
const API: &str = "https://download.lineageos.org/api/v1";
const MIRROR: &str = "https://mirrorbits.lineageos.org/full";

/// Exit code (None when the process was killed or could not report one) and all of its output.
pub struct Out {
    code: Option<i32>,
    out: String,
}

/// The build picked in step 1, shared by the later steps.
struct Target {
    rel: Value,
    major: String,
    date: String,
    dir: PathBuf,
    zip: PathBuf,
}

pub struct Toolbox {
    app: AppHandle,
    settings_file: PathBuf,
    settings: Mutex<Map<String, Value>>,
    target: Mutex<Option<Target>>,
    running: Mutex<Option<Child>>, // the child process currently streaming to the log
    cancelled: AtomicBool,         // set by Cancel; checked by the wait loops and downloads
    http: ureq::Agent,
}

impl Toolbox {
    pub fn new(app: AppHandle) -> Self {
        // App/repo folder (holds scripts\): one up from toolbox\ in a dev build, next to the exe in a release build.
        let app_root = if cfg!(debug_assertions) {
            Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
        } else {
            std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default()
        };
        // Tiny .env reader (KEY=value lines) so the data location can be set without editing code. Real env vars win.
        let dotenv: Vec<(String, String)> =
            fs::read_to_string(app_root.join(".env")).unwrap_or_default().lines().filter_map(parse::env_line).collect();
        let var = |k: &str| std::env::var(k).ok().or_else(|| dotenv.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()));
        // Data root: where artifacts\ and platform-tools\ live (the X: drive). Set DODGE_DATA_ROOT in .env to change it.
        let data_root = var("DODGE_DATA_ROOT").unwrap_or_else(|| r"X:\Android Development\Devices\Dodge".into());
        let platform_tools =
            var("DODGE_PLATFORM_TOOLS").unwrap_or_else(|| Path::new(&data_root).join("platform-tools").display().to_string());
        let defaults = json!({
            "root": app_root.display().to_string(),
            "dataRoot": data_root,
            "platformTools": platform_tools,
            "device": "dodge",
            "gapps": r"artifacts\mindthegapps\MindTheGapps-17.0.0-arm64-20261006_111038.zip",
            "foxZip": r"artifacts\orangefox\OrangeFox-R12.0_2-dodge.zip",
            "kmi": "android15-6.6",
        });
        let mut settings = defaults.as_object().unwrap().clone();
        let (settings_file, saved) = settings_paths();
        if let Some(Value::Object(m)) = saved.iter().find_map(|f| serde_json::from_slice(&fs::read(f).ok()?).ok()) {
            settings.extend(m);
        }
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut http = ureq::AgentBuilder::new().try_proxy_from_env(true);
        #[cfg(windows)]
        if let Ok(tls) = native_tls::TlsConnector::new() {
            http = http.tls_connector(std::sync::Arc::new(tls));
        }
        Self {
            app,
            settings_file,
            settings: Mutex::new(settings),
            target: Mutex::new(None),
            running: Mutex::new(None),
            cancelled: AtomicBool::new(false),
            http: http.build(),
        }
    }

    // --- Settings ---
    pub fn settings(&self) -> Value {
        Value::Object(self.settings.lock().unwrap().clone())
    }

    pub fn set_settings(&self, s: Map<String, Value>) -> Value {
        let mut cur = self.settings.lock().unwrap();
        cur.extend(s);
        if let Some(dir) = self.settings_file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&self.settings_file, serde_json::to_string_pretty(&*cur).unwrap_or_default());
        Value::Object(cur.clone())
    }

    fn s(&self, key: &str) -> String {
        match self.settings.lock().unwrap().get(key) {
            Some(Value::String(v)) => v.clone(),
            Some(Value::Null) | None => String::new(),
            Some(v) => v.to_string(),
        }
    }

    pub fn abs(&self, p: &str) -> PathBuf {
        if Path::new(p).is_absolute() { PathBuf::from(p) } else { Path::new(&self.s("dataRoot")).join(p) }
    }

    /// Downloads and flash images live under artifacts\.
    fn artifact(&self, parts: &[&str]) -> PathBuf {
        let mut p = self.abs("artifacts");
        p.extend(parts);
        p
    }

    // --- Output to the UI ---
    fn log(&self, text: &str, kind: &str) {
        let _ = self.app.emit("log", json!({ "text": text, "kind": kind }));
    }

    fn progress(&self, p: Value) {
        let _ = self.app.emit("progress", p);
    }

    // --- Processes ---
    /// Prefer adb/fastboot from the chosen platform-tools folder; the old Essential ADB (1.0.39) on PATH fails on 3 GB sideloads.
    fn tool(&self, name: &str) -> PathBuf {
        let exe = Path::new(&self.s("platformTools")).join(if cfg!(windows) { format!("{name}.exe") } else { name.into() });
        if exe.exists() { exe } else { PathBuf::from(name) }
    }

    /// Run a command and stream its output to the log (unless quiet).
    fn run(&self, cmd: &Path, args: &[&str], quiet: bool) -> Out {
        if !quiet {
            let base = cmd.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            self.log(&format!("> {base} {}", args.join(" ")), "cmd");
        }
        let mut c = Command::new(cmd);
        c.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // CREATE_NO_WINDOW
        let mut child = match c.spawn() {
            Ok(ch) => ch,
            Err(e) => {
                let msg = format!("spawn {}: {e}", cmd.display());
                if !quiet {
                    self.log(&msg, "err");
                }
                return Out { code: Some(-1), out: msg };
            }
        };
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let pipes: [Option<Box<dyn Read + Send>>; 2] =
            [child.stdout.take().map(|p| Box::new(p) as _), child.stderr.take().map(|p| Box::new(p) as _)];
        for mut pipe in pipes.into_iter().flatten() {
            let tx = tx.clone();
            thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n @ 1..) = pipe.read(&mut buf) {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);

        // A quiet child stays local; a logged one is parked in `running` so Cancel can kill it.
        let mut local = None;
        if quiet { local = Some(child) } else { *self.running.lock().unwrap() = Some(child) }
        let mut with_child = |f: &mut dyn FnMut(&mut Child)| {
            if quiet {
                local.as_mut().map(|c| f(c));
            } else if let Some(c) = self.running.lock().unwrap().as_mut() {
                f(c);
            }
        };

        let mut out = String::new();
        let mut exited_at: Option<Instant> = None;
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(chunk) => {
                    let text = String::from_utf8_lossy(&chunk);
                    if !quiet {
                        self.log(&text, "out");
                    }
                    out.push_str(&text);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => match exited_at {
                    // Exited but a pipe is still open (a daemon it started inherited it): stop reading after a moment.
                    Some(t) if t.elapsed() > Duration::from_millis(500) => break,
                    Some(_) => {}
                    None => with_child(&mut |c| {
                        if matches!(c.try_wait(), Ok(Some(_))) {
                            exited_at = Some(Instant::now());
                        }
                    }),
                },
            }
        }
        let child = if quiet { local } else { self.running.lock().unwrap().take() };
        let code = child.and_then(|mut c| c.wait().ok()).and_then(|s| s.code());
        Out { code, out }
    }

    fn adb(&self, args: &[&str], quiet: bool) -> Out {
        self.run(&self.tool("adb"), args, quiet)
    }

    fn fastboot(&self, args: &[&str], quiet: bool) -> Out {
        self.run(&self.tool("fastboot"), args, quiet)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, SeqCst);
        if let Some(c) = self.running.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }

    fn check_cancel(&self) -> R<()> {
        if self.cancelled.load(SeqCst) { Err("Cancelled.".into()) } else { Ok(()) }
    }

    /// Sleep, but wake early on Cancel.
    fn sleep(&self, ms: u64) {
        let end = Instant::now() + Duration::from_millis(ms);
        while Instant::now() < end && !self.cancelled.load(SeqCst) {
            thread::sleep(Duration::from_millis(100));
        }
    }

    // --- Device state ---
    fn device_state(&self) -> String {
        let a = self.adb(&["devices"], true);
        let f = self.fastboot(&["devices"], true);
        if a.code == Some(0) {
            if let Some(s) = parse::adb_state(&a.out) {
                return s; // device | sideload | recovery | unauthorized
            }
        }
        if f.code == Some(0) && parse::is_fastboot(&f.out) {
            return "fastboot".into();
        }
        if a.code == Some(0) { "none".into() } else { "no adb".into() }
    }

    fn wait_for(&self, state: &str, timeout_sec: u64) -> R<()> {
        self.log(&format!("Waiting for the phone in '{state}' mode..."), "info");
        for _ in 0..timeout_sec / 3 {
            self.check_cancel()?;
            if self.device_state() == state {
                return Ok(());
            }
            self.sleep(3000);
        }
        self.check_cancel()?;
        Err(format!("Phone never reached '{state}' mode."))
    }

    fn wait_boot(&self) -> R<()> {
        self.wait_for("device", 900)?;
        self.log("Waiting for boot to complete...", "info");
        loop {
            self.check_cancel()?;
            if self.adb(&["shell", "getprop", "sys.boot_completed"], true).out.trim() == "1" {
                return Ok(());
            }
            self.sleep(5000);
        }
    }

    fn prop(&self, name: &str) -> String {
        self.adb(&["shell", "getprop", name], true).out.trim().to_string()
    }

    fn to_bootloader(&self) -> R<()> {
        if self.device_state() != "fastboot" {
            must(self.adb(&["reboot", "bootloader"], false))?;
            self.wait_for("fastboot", 60)?;
        }
        Ok(())
    }

    // --- Downloads ---
    fn download(&self, url: &str, out: &Path, label: &str) -> R<()> {
        if out.exists() {
            self.log(&format!("Already have {}", file_name(out)), "info");
            return Ok(());
        }
        self.log(&format!("Downloading {url}"), "info");
        let res = self.http.get(url).call().map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("Download failed ({code}): {url}"),
            e => e.to_string(),
        })?;
        let total: u64 = res.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let tmp = PathBuf::from(format!("{}.part", out.display()));
        let mut file = BufWriter::with_capacity(1 << 20, fs::File::create(&tmp).map_err(err)?);
        let mut body = res.into_reader();
        let mut buf = vec![0u8; 1 << 16];
        let (mut done, mut last) = (0u64, Instant::now() - Duration::from_secs(1));
        loop {
            self.check_cancel()?;
            let n = body.read(&mut buf).map_err(err)?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(err)?;
            done += n as u64;
            if last.elapsed() >= Duration::from_millis(250) {
                // keep IPC traffic down on multi-GB files
                last = Instant::now();
                self.progress(json!({ "label": label, "done": done, "total": total }));
            }
        }
        file.flush().map_err(err)?;
        drop(file);
        fs::rename(&tmp, out).map_err(err)?;
        self.progress(Value::Null);
        Ok(())
    }

    fn sha256(&self, file: &Path) -> R<String> {
        let mut f = fs::File::open(file).map_err(err)?;
        let mut h = Sha256::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            self.check_cancel()?;
            match f.read(&mut buf).map_err(err)? {
                0 => break,
                n => h.update(&buf[..n]),
            }
        }
        Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }

    // --- Build selection ---
    fn latest(&self, build: &str) -> R<Value> {
        let url = format!("{API}/{}/nightly/x", self.s("device"));
        let api: Value = serde_json::from_reader(self.http.get(&url).call().map_err(err)?.into_reader()).map_err(err)?;
        let mut list = api["response"].as_array().cloned().unwrap_or_default();
        list.sort_by(|a, b| b["datetime"].as_f64().unwrap_or(0.0).total_cmp(&a["datetime"].as_f64().unwrap_or(0.0)));
        let needle = format!("-{build}-");
        let rel = list
            .into_iter()
            .find(|r| build.is_empty() || r["filename"].as_str().is_some_and(|f| f.contains(&needle)))
            .ok_or("Build not found in the API response.")?;
        let filename = rel["filename"].as_str().unwrap_or_default().to_string();
        let (major, date) = parse::lineage_version(&filename).ok_or("Unexpected build filename in the API response.")?;
        let t = Target {
            dir: self.artifact(&["builds", &format!("{major}-{date}")]),
            zip: self.artifact(&["lineage", &filename]),
            rel: rel.clone(),
            major,
            date,
        };
        let r = json!({ "filename": filename, "size": rel["size"], "major": t.major });
        *self.target.lock().unwrap() = Some(t);
        Ok(r)
    }

    fn need_target<T>(&self, f: impl FnOnce(&Target) -> T) -> R<T> {
        self.target.lock().unwrap().as_ref().map(f).ok_or_else(|| "Pick a build first (step 1).".into())
    }

    // --- Steps. Each returns a short result string (or an object) for the UI. ---
    pub fn step(&self, name: &str, args: &Value) -> Value {
        if name != "status" {
            self.cancelled.store(false, SeqCst);
        }
        let arg = |k: &str| args[k].as_str().unwrap_or_default().to_string();
        let r = match name {
            "status" => Ok(self.status()),
            "pickBuild" => self.pick_build(&arg("build")),
            "download" => self.step_download().map(Value::from),
            "extractFox" => self.extract_fox().map(Value::from),
            "flashLineageRecovery" => self.flash_lineage_recovery().map(Value::from),
            "sideloadRom" => self.sideload_rom().map(Value::from),
            "sideloadGapps" => self.sideload_gapps().map(Value::from),
            "waitBoot" => self.wait_boot().map(|_| format!("Booted: {}", self.prop("ro.lineage.version")).into()),
            "flashFox" => self.flash_fox().map(Value::from),
            "pushInitBoot" => self.push_init_boot().map(Value::from),
            "pullAndFlashKsu" => self.pull_and_flash_ksu().map(Value::from),
            "rollbackRoot" => self.rollback_root().map(Value::from),
            "reboot" => self.reboot(&arg("to")).map(Value::from),
            "custom" => self.custom(&arg("line")).map(Value::from),
            _ => Err(format!("Unknown step: {name}")),
        };
        match r {
            Ok(result) => json!({ "ok": true, "result": result }),
            Err(e) => {
                self.log(&e, "err");
                self.progress(Value::Null);
                json!({ "ok": false, "error": e })
            }
        }
    }

    fn status(&self) -> Value {
        let state = self.device_state();
        let mut info = json!({ "state": state });
        if state == "device" {
            info["version"] = self.prop("ro.lineage.version").into();
            info["kernel"] = self.adb(&["shell", "uname", "-r"], true).out.trim().into();
            info["slot"] = self.prop("ro.boot.slot_suffix").into();
        }
        let v = self.adb(&["--version"], true);
        info["adbVersion"] = parse::adb_version(&v.out).unwrap_or_else(|| "adb not found".into()).into();
        info
    }

    fn pick_build(&self, build: &str) -> R<Value> {
        let t = self.latest(build)?;
        let current = if self.device_state() == "device" { self.prop("ro.lineage.version") } else { String::new() };
        let major = t["major"].as_str().unwrap_or_default();
        let major_change = !current.is_empty() && !current.starts_with(&format!("{major}."));
        Ok(json!({ "filename": t["filename"], "size": t["size"], "current": current, "majorChange": major_change }))
    }

    fn step_download(&self) -> R<&'static str> {
        let (url, id, filename, dir, zip, date) = self.need_target(|t| {
            let s = |k: &str| t.rel[k].as_str().unwrap_or_default().to_string();
            (s("url"), s("id"), s("filename"), t.dir.clone(), t.zip.clone(), t.date.clone())
        })?;
        fs::create_dir_all(&dir).map_err(err)?;
        fs::create_dir_all(zip.parent().unwrap()).map_err(err)?;
        self.download(&url, &zip, &filename)?;
        self.log("Checking SHA-256...", "info");
        if self.sha256(&zip)? != id.to_lowercase() {
            return Err("SHA-256 mismatch. Delete the zip and retry.".into());
        }
        self.log("SHA-256 OK", "ok");
        for img in ["recovery.img", "init_boot.img", "build-manifest.xml"] {
            self.download(&format!("{MIRROR}/{}/{date}/{img}", self.s("device")), &dir.join(img), img)?;
        }
        if !size_is(&dir.join("recovery.img"), RECOVERY_SIZE) {
            return Err("recovery.img size is wrong.".into());
        }
        Ok("Downloaded and verified.")
    }

    fn extract_fox(&self) -> R<&'static str> {
        let zip = self.abs(&self.s("foxZip"));
        if !zip.exists() {
            return Err(format!("Missing {}", zip.display()));
        }
        let dest = self.artifact(&["fox-extract"]);
        self.log(&format!("> extract {} -> {}", zip.display(), dest.display()), "cmd");
        let mut a = zip::ZipArchive::new(fs::File::open(&zip).map_err(err)?).map_err(err)?;
        for i in 0..a.len() {
            self.check_cancel()?;
            let mut e = a.by_index(i).map_err(err)?;
            let Some(rel) = e.enclosed_name() else { continue };
            let p = dest.join(rel);
            if e.is_dir() {
                fs::create_dir_all(&p).map_err(err)?;
                continue;
            }
            fs::create_dir_all(p.parent().unwrap()).map_err(err)?;
            let mut o = BufWriter::new(fs::File::create(&p).map_err(err)?);
            std::io::copy(&mut e, &mut o).map_err(err)?;
            o.flush().map_err(err)?;
        }
        if !size_is(&dest.join("recovery.img"), RECOVERY_SIZE) {
            return Err("OrangeFox recovery.img is truncated. Do not flash it.".into());
        }
        Ok("OrangeFox recovery.img ready (104857600 bytes).")
    }

    fn flash_lineage_recovery(&self) -> R<&'static str> {
        let img = self.need_target(|t| t.dir.join("recovery.img"))?;
        self.to_bootloader()?;
        must(self.fastboot(&["flash", "recovery", &path_str(&img)], false))?;
        must(self.fastboot(&["reboot", "recovery"], false))?;
        Ok("On the phone: Apply update -> Apply from ADB, then run the next step.")
    }

    fn sideload_rom(&self) -> R<&'static str> {
        let zip = self.need_target(|t| t.zip.clone())?;
        self.wait_for("sideload", 180)?;
        self.adb(&["sideload", &path_str(&zip)], false); // stopping near 47% with "Total xfer: 1.00x" is normal and can exit non-zero
        Ok("Wait for the phone to say it finished. Then on the phone: Advanced -> Reboot to recovery, then Apply update -> Apply from ADB.")
    }

    fn sideload_gapps(&self) -> R<&'static str> {
        let zip = self.abs(&self.s("gapps"));
        if !zip.exists() {
            return Err(format!("Missing {}", zip.display()));
        }
        self.wait_for("sideload", 180)?;
        self.adb(&["sideload", &path_str(&zip)], false);
        Ok("Answer Yes to the signature warning. Then tap Reboot system now.")
    }

    fn flash_fox(&self) -> R<&'static str> {
        let img = self.artifact(&["fox-extract", "recovery.img"]);
        if !size_is(&img, RECOVERY_SIZE) {
            return Err(r#"Run "Extract OrangeFox" first."#.into());
        }
        self.to_bootloader()?;
        must(self.fastboot(&["flash", "recovery", &path_str(&img)], false))?;
        must(self.fastboot(&["reboot"], false))?;
        self.wait_boot()?;
        Ok("OrangeFox flashed and the phone booted.")
    }

    fn push_init_boot(&self) -> R<String> {
        let (img, date) = self.need_target(|t| (t.dir.join("init_boot.img"), t.date.clone()))?;
        must(self.adb(&["push", &path_str(&img), &format!("/sdcard/Download/init_boot_{date}.img")], false))?;
        let kernel = self.adb(&["shell", "uname", "-r"], true).out.trim().to_string();
        Ok(format!(
            "Kernel {kernel}. In KernelSU-Next: Install -> Select and patch a file -> init_boot_{date}.img, and set the KMI to {} by hand.",
            self.s("kmi")
        ))
    }

    fn pull_and_flash_ksu(&self) -> R<&'static str> {
        let date = self.need_target(|t| t.date.clone())?;
        let ls = must(self.adb(&["shell", "ls -t /sdcard/Download/kernelsu_next_patched_*.img"], true))?;
        let remote = ls.lines().next().unwrap_or_default().trim().to_string();
        if remote.is_empty() {
            return Err("No kernelsu_next_patched_*.img in /sdcard/Download.".into());
        }
        let out = self.artifact(&["images", &format!("ksu-patched-init_boot-{date}.img")]);
        fs::create_dir_all(out.parent().unwrap()).map_err(err)?;
        must(self.adb(&["pull", &remote, &path_str(&out)], false))?;
        let ver = parse::vermagic(&fs::read(&out).map_err(err)?).unwrap_or_else(|| "vermagic not found".into());
        self.log(&ver, "info");
        let kmi = self.s("kmi");
        let want = kmi.split('-').nth(1).unwrap_or_default(); // android15-6.6 -> 6.6
        if !ver.starts_with(&format!("vermagic={want}.")) {
            return Err(format!("Wrong KMI in the patched image ({ver}). Re-patch with {kmi}."));
        }
        self.to_bootloader()?;
        must(self.fastboot(&["flash", "init_boot", &path_str(&out)], false))?;
        must(self.fastboot(&["reboot"], false))?;
        self.wait_boot()?;
        Ok("Root flashed. Open the KernelSU-Next manager and confirm it says Working.")
    }

    fn rollback_root(&self) -> R<&'static str> {
        let img = self.need_target(|t| t.dir.join("init_boot.img"))?;
        self.to_bootloader()?;
        must(self.fastboot(&["flash", "init_boot", &path_str(&img)], false))?;
        must(self.fastboot(&["reboot"], false))?;
        Ok("Stock init_boot restored.")
    }

    fn reboot(&self, to: &str) -> R<String> {
        let args: Vec<&str> = if to == "system" { vec!["reboot"] } else { vec!["reboot", to] };
        if self.device_state() == "fastboot" { must(self.fastboot(&args, false))? } else { must(self.adb(&args, false))? };
        Ok(format!("Rebooting to {to}."))
    }

    /// Free-form adb/fastboot command, with the wipe commands blocked.
    fn custom(&self, line: &str) -> R<&'static str> {
        let parts = parse::split_line(line);
        let Some((cmd, args)) = parts.split_first().filter(|(c, _)| *c == "adb" || *c == "fastboot") else {
            return Err("Only adb and fastboot commands are allowed here.".into());
        };
        if parse::is_wipe(args) {
            return Err("Wipe commands are blocked in the toolbox.".into());
        }
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&self.tool(cmd), &args, false);
        Ok("Done.")
    }

    // --- Other window actions ---
    pub fn open_root(&self) {
        let dir = self.s("dataRoot");
        let opener = if cfg!(windows) { "explorer.exe" } else { "xdg-open" };
        let _ = Command::new(opener).arg(dir).spawn();
    }

    /// Hand off to the original PowerShell script in its own console window.
    pub fn run_script(&self) {
        let root = PathBuf::from(self.s("root"));
        let script = root.join("scripts").join("update-lineage.ps1");
        let mut c = Command::new("cmd.exe");
        c.args(["/c", "start", "powershell", "-NoExit", "-ExecutionPolicy", "Bypass", "-File"]).arg(script);
        c.current_dir(&root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0000_0008); // DETACHED_PROCESS
        let _ = c.spawn();
    }
}

fn must(r: Out) -> R<String> {
    match r.code {
        Some(0) => Ok(r.out),
        Some(c) => Err(format!("Command failed (exit {c}).")),
        None => Err("Command failed (exit null).".into()),
    }
}

fn size_is(file: &Path, bytes: u64) -> bool {
    fs::metadata(file).is_ok_and(|m| m.len() == bytes)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn path_str(p: &Path) -> String {
    p.display().to_string()
}

/// Where settings.json is written, then the files to read it from. Same folder the Electron build used
/// (%APPDATA%\Dodge Toolbox), falling back to the one `npm start` used (%APPDATA%\dodge-toolbox).
fn settings_paths() -> (PathBuf, Vec<PathBuf>) {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
    }
    .unwrap_or_default();
    let main = base.join("Dodge Toolbox").join("settings.json");
    let legacy = base.join("dodge-toolbox").join("settings.json");
    (main.clone(), vec![main, legacy])
}
