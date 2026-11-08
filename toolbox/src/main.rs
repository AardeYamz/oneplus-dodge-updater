// Dodge Toolbox: front end for the manual flash instructions in README.md / scripts/update-lineage.ps1.
// ui/ is the page (rendered by the system WebView); src/toolbox.rs owns everything that touches the phone, the network or the disk.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod parse;
mod toolbox;

use serde_json::{json, Map, Value};
use std::sync::Arc;
use tauri::{Manager, State};
use toolbox::Toolbox;

type Tb<'a> = State<'a, Arc<Toolbox>>;

/// Runs one step on a worker thread; resolves with { ok, result } or { ok: false, error }.
#[tauri::command]
async fn step(tb: Tb<'_>, name: String, args: Option<Value>) -> Result<Value, ()> {
    let tb = tb.inner().clone();
    let args = args.unwrap_or_default();
    Ok(tauri::async_runtime::spawn_blocking(move || tb.step(&name, &args))
        .await
        .unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() })))
}

#[tauri::command]
fn cancel(tb: Tb<'_>) {
    tb.cancel();
}

#[tauri::command]
fn settings_get(tb: Tb<'_>) -> Value {
    tb.settings()
}

#[tauri::command]
fn settings_set(tb: Tb<'_>, s: Map<String, Value>) -> Value {
    tb.set_settings(s)
}

#[tauri::command]
async fn pick(window: tauri::WebviewWindow, tb: Tb<'_>, folder: bool, current: Option<String>) -> Result<Option<String>, ()> {
    let start = match current.filter(|c| !c.is_empty()) {
        Some(c) => tb.abs(&c),
        None => tb.abs(""),
    };
    let mut d = rfd::AsyncFileDialog::new().set_parent(&window);
    d = if start.is_file() {
        let d = d.set_file_name(start.file_name().unwrap_or_default().to_string_lossy());
        match start.parent() { Some(p) => d.set_directory(p), None => d }
    } else {
        d.set_directory(&start)
    };
    let picked = if folder { d.pick_folder().await } else { d.pick_file().await };
    Ok(picked.map(|f| f.path().display().to_string()))
}

#[tauri::command]
fn open_root(tb: Tb<'_>) {
    tb.open_root();
}

#[tauri::command]
fn run_script(tb: Tb<'_>) {
    tb.run_script();
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Arc::new(Toolbox::new(app.handle().clone())));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![step, cancel, settings_get, settings_set, pick, open_root, run_script])
        .run(tauri::generate_context!())
        .expect("failed to start the Dodge Toolbox window");
}
