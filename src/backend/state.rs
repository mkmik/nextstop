//! `state.json` persistence (§10, §11).
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const VERSION: u64 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct XY { pub x: i32, pub y: i32 }
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WH { pub w: i32, pub h: i32 }
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TornMenu { pub path: Vec<String>, pub x: i32, pub y: i32 }

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct WinState { pub x: i32, pub y: i32, pub w: i32, pub h: i32, pub open: bool, #[serde(skip_serializing_if = "String::is_empty")] pub path: String }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Windows { pub file_viewer: WinState, pub inspector: WinState, pub console: WinState, pub recycler: WinState, pub mandelbrot: WinState, pub shell: WinState, pub concurrence: WinState, pub librarian: WinState }
impl Default for Windows {
    fn default() -> Self {
        Windows {
            file_viewer: WinState { x: 120, y: 40, w: 640, h: 480, open: true, path: String::new() },
            inspector: WinState { x: 780, y: 40, w: 272, h: 400, open: false, path: String::new() },
            console: WinState { x: 200, y: 500, w: 480, h: 240, open: false, path: String::new() },
            recycler: WinState { x: 300, y: 300, w: 320, h: 240, open: false, path: String::new() },
            mandelbrot: WinState { x: 200, y: 80, w: 520, h: 528, open: false, path: String::new() },
            shell: WinState { x: 160, y: 120, w: 591, h: 374, open: false, path: String::new() },
            concurrence: WinState { x: 140, y: 60, w: 560, h: 440, open: false, path: String::new() },
            librarian: WinState { x: 240, y: 100, w: 560, h: 420, open: false, path: String::new() },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct State {
    pub version: u64,
    pub os_window: WH,
    pub scale: f32,
    pub show_hidden: bool,
    pub animations: bool,
    pub backdrop: bool,
    pub dock_visible: bool,
    pub miniwindows_visible: bool,
    pub recycler_visible: bool,
    pub menu_pos: XY,
    pub torn_menus: Vec<TornMenu>,
    pub dock: Vec<String>,
    pub shelf: Vec<String>,
    /// The Concurrence outline, one tab-indented line per topic.
    pub concurrence: Vec<String>,
    pub windows: Windows,
}
impl Default for State {
    fn default() -> Self {
        State { version: VERSION, os_window: WH { w: 1120, h: 832 }, scale: 1.0, show_hidden: false, animations: true, backdrop: false, dock_visible: false, miniwindows_visible: false, recycler_visible: false, menu_pos: XY { x: 0, y: 0 }, torn_menus: vec![], dock: vec![], shelf: vec![], concurrence: vec![], windows: Windows::default() }
    }
}

pub fn state_path() -> PathBuf { crate::backend::apps::config_dir().join("state.json") }

/// Read and validate. A corrupt or unknown-version file is renamed to `state.json.bad`
/// and `None` is returned together with a message for the Console. Never panics.
pub fn read_state(path: &Path) -> (Option<State>, Option<String>) {
    let Ok(text) = fs::read_to_string(path) else { return (None, None) };
    let bad = |why: String| {
        let _ = fs::rename(path, path.with_extension("json.bad"));
        (None, Some(format!("{}: {why}; renamed to state.json.bad and using defaults", path.display())))
    };
    let v: Value = match serde_json::from_str(&text) { Ok(v) => v, Err(e) => return bad(format!("invalid JSON ({e})")) };
    match v.get("version").and_then(Value::as_u64) {
        Some(VERSION) => match serde_json::from_value::<State>(v) { Ok(s) => (Some(s), None), Err(e) => bad(format!("unexpected shape ({e})")) },
        other => bad(format!("unknown state version {other:?}")),
    }
}

pub fn save_state(state: &State) -> Result<(), String> {
    let p = state_path();
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("reworkspace-state-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d.join("state.json")
    }
    #[test]
    fn valid_state_loads_with_defaults_for_missing_fields() {
        let p = tmp("ok");
        fs::write(&p, r#"{"version":1,"scale":2}"#).unwrap();
        let (s, msg) = read_state(&p);
        let s = s.unwrap();
        assert_eq!(s.scale, 2.0);
        assert_eq!(s.windows.file_viewer.w, 640);
        assert!(msg.is_none());
    }
    #[test]
    fn corrupt_state_is_renamed() {
        let p = tmp("corrupt");
        fs::write(&p, "{ not json").unwrap();
        let (s, msg) = read_state(&p);
        assert!(s.is_none());
        assert!(msg.unwrap().contains("state.json.bad"));
        assert!(!p.exists() && p.with_extension("json.bad").exists());
    }
    #[test]
    fn unknown_version_is_renamed() {
        let p = tmp("version");
        fs::write(&p, r#"{"version":99}"#).unwrap();
        let (s, msg) = read_state(&p);
        assert!(s.is_none() && msg.is_some() && p.with_extension("json.bad").exists());
    }
    #[test]
    fn roundtrip() {
        let s = State::default();
        let v = serde_json::to_string(&s).unwrap();
        let back: State = serde_json::from_str(&v).unwrap();
        assert_eq!(back.windows.console.h, 240);
    }
}
