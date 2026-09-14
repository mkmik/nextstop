//! File-type → icon mapping (§12) and human "kind" strings.
pub fn ext(name: &str) -> String {
    match name.rfind('.') { Some(i) if i > 0 => name[i + 1..].to_lowercase(), _ => String::new() }
}

const MAP: &[(&str, &str)] = &[
    ("txt md log csv json yaml yml toml ini", "file-text"),
    ("png jpg jpeg gif bmp webp svg icns ico", "file-image"),
    ("zip tar gz tgz bz2 xz 7z rar dmg", "file-archive"),
    ("c h cpp go rs py js ts sh zig rb java swift m", "file-code"),
    ("pdf", "file-pdf"),
    ("mp3 wav flac aac m4a ogg", "file-audio"),
    ("mp4 mov mkv avi webm", "file-video"),
    ("app exe lnk", "application"),
];

pub fn file_icon(name: &str) -> &'static str {
    let e = ext(name);
    MAP.iter().find(|(exts, _)| exts.split(' ').any(|x| x == e)).map(|(_, n)| *n).unwrap_or("file-generic")
}
/// 48×48 icon name for an entry.
pub fn icon_for(name: &str, is_dir: bool, is_app: bool) -> &'static str {
    if is_dir { "folder" } else if is_app { "application" } else { file_icon(name) }
}
/// 16×16 cell icon name (only some types have a small variant).
pub fn small_icon_for(name: &str, is_dir: bool, is_app: bool) -> &'static str {
    match icon_for(name, is_dir, is_app) {
        "folder" => "folder-16", "application" => "application-16", "file-text" => "file-text-16",
        "file-image" => "file-image-16", "file-code" => "file-code-16", _ => "file-generic-16",
    }
}
pub fn is_app_path(name: &str) -> bool { matches!(ext(name).as_str(), "app" | "exe" | "lnk") }
pub fn kind_for(name: &str, is_dir: bool, is_app: bool) -> String {
    if is_dir { return "Folder".into(); }
    if is_app { return "Application".into(); }
    let e = ext(name);
    if e.is_empty() { return "Document".into(); }
    let w = match file_icon(name) {
        "file-text" => "text document", "file-image" => "image", "file-archive" => "archive", "file-code" => "source code",
        "file-pdf" => "document", "file-audio" => "audio", "file-video" => "video", _ => "file",
    };
    format!("{} {w}", e.to_uppercase())
}
pub fn is_text_like(name: &str) -> bool { matches!(file_icon(name), "file-text" | "file-code") }
pub fn image_mime(name: &str) -> bool { matches!(ext(name).as_str(), "png") }

// ---- path helpers for "/" and "C:\" roots
pub fn is_root(p: &str) -> bool { p == "/" || (p.len() == 3 && p.as_bytes()[1] == b':' && p.ends_with('\\')) }
pub fn sep(p: &str) -> char { if p.contains('\\') && !p.contains('/') { '\\' } else { '/' } }
pub fn basename(p: &str) -> String {
    if is_root(p) { return p.to_string(); }
    let t = p.trim_end_matches(['/', '\\']);
    t.rsplit(['/', '\\']).next().unwrap_or(t).to_string()
}
pub fn dirname(p: &str) -> Option<String> {
    if is_root(p) { return None; }
    let t = p.trim_end_matches(['/', '\\']);
    let i = t.rfind(['/', '\\'])?;
    if sep(p) == '\\' { Some(if i <= 2 { t[..3].to_string() } else { t[..i].to_string() }) } else { Some(if i == 0 { "/".into() } else { t[..i].to_string() }) }
}
pub fn join(dir: &str, name: &str) -> String { if is_root(dir) { format!("{dir}{name}") } else { format!("{dir}{}{name}", sep(dir)) } }
/// Directories listed by each Browser column to reach `p`; on Windows the first is `None` (synthetic "Computer").
pub fn ancestors(p: &str, windows: bool) -> Vec<Option<String>> {
    let mut out = vec![];
    let mut cur = Some(p.to_string());
    while let Some(c) = cur { cur = dirname(&c); out.push(Some(c)); }
    out.reverse();
    if windows { out.insert(0, None); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths() {
        assert_eq!(basename("/Users/marko/"), "marko");
        assert_eq!(basename("/"), "/");
        assert_eq!(dirname("/Users"), Some("/".into()));
        assert_eq!(dirname("/Users/marko/Documents"), Some("/Users/marko".into()));
        assert_eq!(dirname("/"), None);
        assert_eq!(join("/", "Users"), "/Users");
        assert_eq!(join("/Users", "marko"), "/Users/marko");
        assert_eq!(ancestors("/Users/marko", false), vec![Some("/".to_string()), Some("/Users".into()), Some("/Users/marko".into())]);
        assert_eq!(dirname("C:\\Users\\me"), Some("C:\\Users".into()));
        assert_eq!(dirname("C:\\Users"), Some("C:\\".into()));
        assert!(is_root("C:\\"));
        assert_eq!(join("C:\\", "Users"), "C:\\Users");
    }
    #[test]
    fn kinds() {
        assert_eq!(file_icon("a.PNG"), "file-image");
        assert_eq!(kind_for("x.pdf", false, false), "PDF document");
        assert_eq!(small_icon_for("main.rs", false, false), "file-code-16");
    }
}
