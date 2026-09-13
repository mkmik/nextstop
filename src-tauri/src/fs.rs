//! Filesystem commands (§8). All paths are absolute; `..` segments are rejected.
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Serialize, Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_app: bool,
    pub size: u64,
    pub modified: i64,
    pub hidden: bool,
}

#[derive(Serialize)]
pub struct Meta {
    pub created: Option<i64>,
    pub modified: i64,
    pub size: u64,
    pub mode: String,
    pub owner: Option<String>,
    pub is_dir: bool,
}

type R<T> = Result<T, String>;
fn err<E: std::fmt::Display>(ctx: &Path) -> impl Fn(E) -> String + '_ {
    move |e| format!("{}: {}", ctx.display(), e)
}

/// Reject relative paths and anything with a `..` segment.
pub fn check(path: &str) -> R<PathBuf> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("not an absolute path: {path}"));
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("path contains '..': {path}"));
    }
    Ok(p.to_path_buf())
}

/// Roots that destroy/empty must never touch (§8 security).
pub fn protected(p: &Path) -> bool {
    let fixed = ["/", "/Users", "/System", "/Applications", "C:\\", "C:\\Windows"];
    if fixed.iter().any(|f| Path::new(f) == p) {
        return true;
    }
    if dirs::home_dir().is_some_and(|h| h == p) {
        return true;
    }
    let s = p.to_string_lossy().to_lowercase();
    let s = s.trim_end_matches(['\\', '/']);
    s.starts_with("c:\\program files") && !s["c:\\program files".len()..].contains('\\')
}

fn checked_unprotected(path: &str) -> R<PathBuf> {
    let p = check(path)?;
    if protected(&p) {
        return Err(format!("refusing to operate on protected path {}", p.display()));
    }
    Ok(p)
}

fn secs(t: std::io::Result<std::time::SystemTime>) -> Option<i64> {
    t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64)
}

#[cfg(windows)]
fn os_hidden(m: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    m.file_attributes() & 0x2 != 0
}
#[cfg(not(windows))]
fn os_hidden(_: &fs::Metadata) -> bool {
    false
}

fn ext_of(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

fn is_app(name: &str, is_dir: bool) -> bool {
    let e = ext_of(name);
    if cfg!(target_os = "macos") {
        is_dir && e == "app"
    } else if cfg!(windows) {
        !is_dir && (e == "exe" || e == "lnk")
    } else {
        false
    }
}

pub fn entry(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let lmeta = fs::symlink_metadata(path).ok()?;
    let is_symlink = lmeta.file_type().is_symlink();
    // For symlinks we report the *target's* type so the Browser can show ▸ and navigate (§9.12);
    // a broken link falls back to the link's own metadata.
    let meta = if is_symlink { fs::metadata(path).unwrap_or_else(|_| lmeta.clone()) } else { lmeta.clone() };
    let app = is_app(&name, meta.is_dir());
    Some(Entry {
        hidden: name.starts_with('.') || os_hidden(&lmeta),
        path: path.to_string_lossy().into_owned(),
        is_dir: meta.is_dir() && !app,
        is_symlink,
        is_app: app,
        size: meta.len(),
        modified: secs(meta.modified()).unwrap_or(0),
        name,
    })
}

/// Directories first, then files; each group case-insensitively alphabetical.
pub fn sort_entries(v: &mut [Entry]) {
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

pub fn read_entries(dir: &Path, show_hidden: bool) -> R<Vec<Entry>> {
    let rd = fs::read_dir(dir).map_err(err(dir))?;
    let mut v: Vec<Entry> = rd.filter_map(|e| e.ok()).filter_map(|e| entry(&e.path())).filter(|e| show_hidden || !e.hidden).collect();
    sort_entries(&mut v);
    Ok(v)
}

#[tauri::command]
pub async fn list_dir(path: String, show_hidden: bool) -> R<Vec<Entry>> {
    read_entries(&check(&path)?, show_hidden)
}

#[tauri::command]
pub fn home_dir() -> R<String> {
    dirs::home_dir().map(|p| p.to_string_lossy().into_owned()).ok_or_else(|| "no home directory".into())
}

#[tauri::command]
pub fn root_dirs() -> Vec<String> {
    if cfg!(windows) {
        (b'A'..=b'Z').map(|c| format!("{}:\\", c as char)).filter(|d| Path::new(d).exists()).collect()
    } else {
        vec!["/".into()]
    }
}

#[tauri::command]
pub fn filter_existing(paths: Vec<String>) -> Vec<String> {
    paths.into_iter().filter(|p| Path::new(p).exists()).collect()
}

#[cfg(unix)]
fn mode_string(m: &fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = m.permissions().mode();
    let t = if m.file_type().is_symlink() { 'l' } else if m.is_dir() { 'd' } else { '-' };
    let mut s = String::from(t);
    for shift in [6, 3, 0] {
        let b = (mode >> shift) & 7;
        s.push(if b & 4 != 0 { 'r' } else { '-' });
        s.push(if b & 2 != 0 { 'w' } else { '-' });
        s.push(if b & 1 != 0 { 'x' } else { '-' });
    }
    s
}
#[cfg(not(unix))]
fn mode_string(m: &fs::Metadata) -> String {
    if m.permissions().readonly() { "Read-only".into() } else { "Read/Write".into() }
}

#[cfg(unix)]
fn owner_name(m: &fs::Metadata) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: getpwuid returns a pointer to static storage or null; we copy the name out immediately.
    unsafe {
        let pw = libc::getpwuid(m.uid());
        if pw.is_null() {
            return Some(m.uid().to_string());
        }
        Some(std::ffi::CStr::from_ptr((*pw).pw_name).to_string_lossy().into_owned())
    }
}
#[cfg(not(unix))]
fn owner_name(_: &fs::Metadata) -> Option<String> {
    None
}

#[tauri::command]
pub async fn file_meta(path: String) -> R<Meta> {
    let p = check(&path)?;
    let m = fs::symlink_metadata(&p).map_err(err(&p))?;
    Ok(Meta {
        created: secs(m.created()),
        modified: secs(m.modified()).unwrap_or(0),
        size: m.len(),
        mode: mode_string(&m),
        owner: owner_name(&m),
        is_dir: m.is_dir(),
    })
}

#[tauri::command]
pub async fn read_text_head(path: String, max_bytes: usize) -> R<String> {
    use std::io::Read;
    let p = check(&path)?;
    let mut buf = Vec::with_capacity(max_bytes.min(1 << 20));
    fs::File::open(&p).map_err(err(&p))?.take(max_bytes as u64).read_to_end(&mut buf).map_err(err(&p))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[tauri::command]
pub async fn read_file_b64(path: String, max_bytes: u64) -> R<String> {
    use base64::Engine;
    let p = check(&path)?;
    let len = fs::metadata(&p).map_err(err(&p))?.len();
    if len > max_bytes {
        return Err(format!("{}: file is too large ({len} bytes)", p.display()));
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(fs::read(&p).map_err(err(&p))?))
}

#[tauri::command]
pub async fn dir_size(path: String) -> R<u64> {
    fn walk(p: &Path) -> u64 {
        let Ok(rd) = fs::read_dir(p) else { return 0 };
        rd.filter_map(|e| e.ok()).map(|e| match e.metadata() {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => walk(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        }).sum()
    }
    Ok(walk(&check(&path)?))
}

/// `base` → `base`, `base 2`, `base 3`… (folders) or `stem copy.ext`, `stem copy 2.ext`… (copies).
pub fn unique_name(dir: &Path, name: &str, copy: bool) -> String {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 && !copy_is_dir_like(dir, name) => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let candidate = |n: u32| match (copy, n) {
        (false, 1) => name.to_string(),
        (false, n) => format!("{name} {n}"),
        (true, 1) => format!("{stem} copy{ext}"),
        (true, n) => format!("{stem} copy {n}{ext}"),
    };
    (1..).map(candidate).find(|c| !dir.join(c).exists()).unwrap()
}
fn copy_is_dir_like(dir: &Path, name: &str) -> bool {
    let p = dir.join(name);
    p.is_dir() && !is_app(name, true)
}

#[tauri::command]
pub async fn new_folder(parent: String) -> R<String> {
    let dir = check(&parent)?;
    let p = dir.join(unique_name(&dir, "New Folder", false));
    fs::create_dir(&p).map_err(err(&p))?;
    Ok(p.to_string_lossy().into_owned())
}

fn copy_recursive(src: &Path, dst: &Path) -> R<()> {
    let m = fs::symlink_metadata(src).map_err(err(src))?;
    if m.is_dir() {
        fs::create_dir(dst).map_err(err(dst))?;
        for e in fs::read_dir(src).map_err(err(src))?.filter_map(|e| e.ok()) {
            copy_recursive(&e.path(), &dst.join(e.file_name()))?;
        }
    } else {
        fs::copy(src, dst).map_err(err(src))?;
    }
    Ok(())
}

fn remove_any(p: &Path) -> R<()> {
    let m = fs::symlink_metadata(p).map_err(err(p))?;
    if m.is_dir() { fs::remove_dir_all(p) } else { fs::remove_file(p) }.map_err(err(p))
}

fn name_of(p: &Path) -> R<String> {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| format!("{}: no file name", p.display()))
}

#[tauri::command]
pub async fn move_paths(paths: Vec<String>, dest_dir: String) -> R<()> {
    let dir = check(&dest_dir)?;
    for s in paths {
        let src = check(&s)?;
        let dst = dir.join(name_of(&src)?);
        if src == dst || src.parent() == Some(dir.as_path()) {
            continue;
        }
        if dir.starts_with(&src) {
            return Err(format!("cannot move {} into itself", src.display()));
        }
        if dst.exists() {
            return Err(format!("{} already exists", dst.display()));
        }
        if fs::rename(&src, &dst).is_err() {
            copy_recursive(&src, &dst)?; // cross-device: copy then delete
            remove_any(&src)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn copy_paths(paths: Vec<String>, dest_dir: String) -> R<()> {
    let dir = check(&dest_dir)?;
    for s in paths {
        let src = check(&s)?;
        if dir.starts_with(&src) {
            return Err(format!("cannot copy {} into itself", src.display()));
        }
        let name = name_of(&src)?;
        let dst = if dir.join(&name).exists() { dir.join(unique_name(&dir, &name, true)) } else { dir.join(&name) };
        copy_recursive(&src, &dst)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn duplicate_paths(paths: Vec<String>) -> R<Vec<String>> {
    let mut out = vec![];
    for s in paths {
        let src = check(&s)?;
        let dir = src.parent().ok_or("no parent")?.to_path_buf();
        let dst = dir.join(unique_name(&dir, &name_of(&src)?, true));
        copy_recursive(&src, &dst)?;
        out.push(dst.to_string_lossy().into_owned());
    }
    Ok(out)
}

#[tauri::command]
pub async fn destroy_paths(paths: Vec<String>) -> R<()> {
    for s in &paths {
        checked_unprotected(s)?; // validate everything before deleting anything
    }
    for s in paths {
        remove_any(&check(&s)?)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn trash_paths(paths: Vec<String>) -> R<()> {
    let ps: Vec<PathBuf> = paths.iter().map(|s| checked_unprotected(s)).collect::<R<_>>()?;
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager); // no Finder automation prompt
        ctx.delete_all(&ps).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "macos"))]
    trash::delete_all(&ps).map_err(|e| e.to_string())
}

fn mac_trash() -> R<PathBuf> {
    if !cfg!(target_os = "macos") {
        return Err("unsupported on this platform".into());
    }
    dirs::home_dir().map(|h| h.join(".Trash")).ok_or_else(|| "no home directory".into())
}

#[tauri::command]
pub async fn trash_list() -> R<Vec<Entry>> {
    read_entries(&mac_trash()?, true).map(|v| v.into_iter().filter(|e| e.name != ".DS_Store").collect())
}

#[tauri::command]
pub async fn trash_is_empty() -> R<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(trash_list().await?.is_empty())
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::os_limited::list().map(|l| l.is_empty()).map_err(|e| e.to_string())
    }
}

/// macOS only: permanently delete everything in `~/.Trash`. The frontend must have confirmed (§7.7).
#[tauri::command]
pub async fn empty_trash() -> R<()> {
    let t = mac_trash()?;
    if protected(&t) {
        return Err("refusing".into());
    }
    for e in fs::read_dir(&t).map_err(err(&t))?.filter_map(|e| e.ok()) {
        remove_any(&e.path())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reworkspace-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rejects_relative_and_dotdot() {
        assert!(check("relative/path").is_err());
        assert!(check("/tmp/../etc").is_err());
        assert!(check("/tmp/ok").is_ok());
    }

    #[test]
    fn protected_roots() {
        for p in ["/", "/Users", "/System", "/Applications", "C:\\", "C:\\Windows", "C:\\Program Files", "C:\\Program Files (x86)"] {
            assert!(protected(Path::new(p)), "{p}");
        }
        assert!(protected(&dirs::home_dir().unwrap()));
        assert!(!protected(Path::new("/Users/nobody/Documents")));
        assert!(!protected(Path::new("C:\\Program Files\\App")));
        assert!(checked_unprotected("/").is_err());
    }

    #[test]
    fn sorting_and_hidden() {
        let d = tmp("sort");
        for f in ["b.txt", "A.txt", ".hidden"] { fs::write(d.join(f), "x").unwrap(); }
        for f in ["zdir", "Adir"] { fs::create_dir(d.join(f)).unwrap(); }
        let names = |v: Vec<Entry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names(read_entries(&d, false).unwrap()), ["Adir", "zdir", "A.txt", "b.txt"]);
        assert_eq!(names(read_entries(&d, true).unwrap()), ["Adir", "zdir", ".hidden", "A.txt", "b.txt"]);
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn unique_names() {
        let d = tmp("unique");
        assert_eq!(unique_name(&d, "New Folder", false), "New Folder");
        fs::create_dir(d.join("New Folder")).unwrap();
        assert_eq!(unique_name(&d, "New Folder", false), "New Folder 2");
        fs::write(d.join("a.txt"), "").unwrap();
        assert_eq!(unique_name(&d, "a.txt", true), "a copy.txt");
        fs::write(d.join("a copy.txt"), "").unwrap();
        assert_eq!(unique_name(&d, "a.txt", true), "a copy 2.txt");
        fs::create_dir(d.join("my.dir")).unwrap();
        assert_eq!(unique_name(&d, "my.dir", true), "my.dir copy");
        fs::remove_dir_all(d).unwrap();
    }
}
