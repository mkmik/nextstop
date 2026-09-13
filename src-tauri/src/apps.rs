//! Launching host applications and extracting their icons (§7.5, §8).
use std::path::{Path, PathBuf};
use std::process::Command;

type R<T> = Result<T, String>;

fn run(cmd: &mut Command) -> R<()> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

#[tauri::command]
pub fn launch_app(app: String) -> R<()> {
    let p = crate::fs::check(&app)?;
    if cfg!(target_os = "macos") {
        run(Command::new("open").arg(&p))
    } else if cfg!(windows) {
        run(Command::new("cmd").args(["/C", "start", "", &p.to_string_lossy()]))
    } else {
        run(Command::new("xdg-open").arg(&p))
    }
}

#[tauri::command]
pub fn open_with(app: String, path: String) -> R<()> {
    let a = crate::fs::check(&app)?;
    let p = crate::fs::check(&path)?;
    if cfg!(target_os = "macos") {
        run(Command::new("open").args(["-a".as_ref(), a.as_os_str(), p.as_os_str()]))
    } else if cfg!(windows) {
        run(Command::new("cmd").args(["/C", "start", "", &a.to_string_lossy(), &p.to_string_lossy()]))
    } else {
        Err("unsupported".into())
    }
}

/// Default Dock apps that exist on this machine (§7.5).
#[tauri::command]
pub fn default_dock() -> Vec<String> {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["/System/Applications/Utilities/Terminal.app", "/System/Applications/TextEdit.app", "/Applications/Safari.app"]
    } else if cfg!(windows) {
        &[
            "C:\\Windows\\System32\\notepad.exe",
            "C:\\Windows\\System32\\calc.exe",
            "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
            "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
        ]
    } else {
        &[]
    };
    candidates.iter().filter(|c| Path::new(c).exists()).map(|c| c.to_string()).collect()
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(std::env::temp_dir).join("ReWorkspace")
}

fn cache_path(app: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    app.hash(&mut h);
    config_dir().join("icons").join(format!("{:016x}.png", h.finish()))
}

#[cfg(target_os = "macos")]
fn extract_icon(app: &Path, out: &Path) -> R<()> {
    let plist = app.join("Contents/Info.plist");
    let name = Command::new("plutil").args(["-extract", "CFBundleIconFile", "raw", "-o", "-"]).arg(&plist).output()
        .ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "AppIcon".into());
    let res = app.join("Contents/Resources");
    let icns = [res.join(&name), res.join(format!("{name}.icns"))].into_iter().find(|p| p.is_file())
        .ok_or_else(|| format!("no .icns for {}", app.display()))?;
    run(Command::new("sips").args(["-s", "format", "png", "-Z", "48"]).arg(&icns).arg("--out").arg(out))
}

#[cfg(windows)]
fn extract_icon(app: &Path, out: &Path) -> R<()> {
    // ponytail: PowerShell + System.Drawing instead of a GDI crate; icons come out 32×32 and are scaled by CSS.
    let script = format!(
        "Add-Type -AssemblyName System.Drawing; [System.Drawing.Icon]::ExtractAssociatedIcon('{}').ToBitmap().Save('{}', [System.Drawing.Imaging.ImageFormat]::Png)",
        app.display().to_string().replace('\'', "''"), out.display().to_string().replace('\'', "''"));
    run(Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn extract_icon(_app: &Path, _out: &Path) -> R<()> {
    Err("unsupported".into())
}

/// 48×48 PNG of the app's icon as base64, cached under the config dir.
#[tauri::command]
pub async fn app_icon_png(app: String) -> R<String> {
    use base64::Engine;
    let p = crate::fs::check(&app)?;
    let cache = cache_path(&p);
    if !cache.is_file() {
        std::fs::create_dir_all(cache.parent().unwrap()).map_err(|e| e.to_string())?;
        extract_icon(&p, &cache)?;
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(std::fs::read(&cache).map_err(|e| e.to_string())?))
}

/// Windows only: show the Recycle Bin in Explorer (§7.7).
#[tauri::command]
pub fn open_recycle_bin() -> R<()> {
    if !cfg!(windows) {
        return Err("unsupported".into());
    }
    Command::new("explorer.exe").arg("shell:RecycleBinFolder").spawn().map(|_| ()).map_err(|e| e.to_string())
}
