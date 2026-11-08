// Fetches the shared yl-theme files (ui/theme/yl-theme.css, ui/theme/yl-theme.js, favicon.svg) at the version pinned
// in theme.json and checks their SHA-256, so a changed or corrupted file fails the build instead of shipping.
// Then renders favicon.svg to icons/icon.png + icons/icon.ico (window and exe icon) and runs tauri-build,
// which embeds ui/ into the exe.
//   DODGE_SKIP_THEME=1  build without fetching (offline checks only: the app then has no theme)
use sha2::{Digest, Sha256};
use std::{env, fs, io::Read, path::Path};

const BASE: &str = "https://theme.yannislam.org";
const FILES: [&str; 3] = ["yl-theme.css", "yl-theme.js", "favicon.svg"];
// Used when favicon.svg is not there (DODGE_SKIP_THEME), so the build still has an icon.
const FALLBACK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect width="16" height="16" rx="4" fill="#888"/></svg>"##;

fn main() {
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&root);
    println!("cargo:rerun-if-changed=theme.json");
    println!("cargo:rerun-if-env-changed=DODGE_SKIP_THEME");
    for f in FILES {
        println!("cargo:rerun-if-changed=ui/theme/{f}");
    }

    let theme = root.join("ui").join("theme");
    if env::var_os("DODGE_SKIP_THEME").is_some() {
        println!("cargo:warning=DODGE_SKIP_THEME is set: building without the yl-theme files");
    } else if let Err(e) = fetch_pinned(&root.join("theme.json"), &theme) {
        panic!("theme fetch failed: {e}");
    }
    if let Err(e) = render_icons(&theme.join("favicon.svg"), &root.join("icons")) {
        panic!("icon render failed: {e}");
    }
    tauri_build::build();
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

fn fetch_pinned(pin: &Path, dir: &Path) -> Result<(), String> {
    let pin: serde_json::Value = serde_json::from_slice(&fs::read(pin).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let version = pin["version"].as_str().ok_or("theme.json has no version")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for name in FILES {
        let want = pin["files"][name].as_str().ok_or(format!("theme.json has no hash for {name}"))?;
        let file = dir.join(name);
        if fs::read(&file).is_ok_and(|d| sha256(&d) == want) {
            continue;
        }
        let url = format!("{BASE}/{version}/{name}");
        let mut data = Vec::new();
        ureq::get(&url)
            .call()
            .map_err(|e| format!("{url}: {e}"))?
            .into_reader()
            .read_to_end(&mut data)
            .map_err(|e| format!("{url}: {e}"))?;
        if sha256(&data) != want {
            return Err(format!("{name} at {version} does not match its hash in theme.json"));
        }
        fs::write(&file, data).map_err(|e| e.to_string())?;
        println!("cargo:warning=fetched {name} ({version})");
    }
    Ok(())
}

// The theme's favicon.ico stops at 48px, but the exe icon needs 256px, so render the SVG at each size.
fn render_icons(svg: &Path, out: &Path) -> Result<(), String> {
    let png = out.join("icon.png");
    let ico = out.join("icon.ico");
    let fresh = |f: &Path| match (fs::metadata(f).and_then(|m| m.modified()), fs::metadata(svg).and_then(|m| m.modified())) {
        (Ok(o), Ok(s)) => o >= s,
        (Ok(_), Err(_)) => true, // no svg: keep whatever icon is there
        _ => false,
    };
    if fresh(&png) && fresh(&ico) {
        return Ok(());
    }
    let data = fs::read(svg).unwrap_or_else(|_| FALLBACK_SVG.as_bytes().to_vec());
    let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).map_err(|e| e.to_string())?;
    let render = |n: u32| -> Result<resvg::tiny_skia::Pixmap, String> {
        let mut pm = resvg::tiny_skia::Pixmap::new(n, n).ok_or("bad icon size")?;
        let (w, h) = (tree.size().width(), tree.size().height());
        let s = (n as f32 / w).min(n as f32 / h); // fit: contain, transparent background
        let t = resvg::tiny_skia::Transform::from_row(s, 0.0, 0.0, s, (n as f32 - w * s) / 2.0, (n as f32 - h * s) / 2.0);
        resvg::render(&tree, t, &mut pm.as_mut());
        Ok(pm)
    };
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    fs::write(&png, render(512)?.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for n in [256, 64, 48, 32, 16] {
        let pm = render(n)?;
        let rgba = pm.pixels().iter().flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        });
        let img = ico::IconImage::from_rgba_data(n, n, rgba.collect());
        dir.add_entry(ico::IconDirEntry::encode(&img).map_err(|e| e.to_string())?);
    }
    dir.write(fs::File::create(&ico).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    println!("cargo:warning=rendered icons/icon.png and icons/icon.ico");
    Ok(())
}
