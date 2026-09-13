//! `state.json` persistence (§10, §11).
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const VERSION: u64 = 1;

#[derive(Serialize)]
pub struct Loaded {
    pub state: Option<Value>,
    pub no_anim: bool,
    pub message: Option<String>,
}

pub fn state_path() -> PathBuf {
    crate::apps::config_dir().join("state.json")
}

/// Read and validate. A corrupt or unknown-version file is renamed to `state.json.bad`
/// and `None` is returned together with a message for the Console. Never panics.
pub fn read_state(path: &Path) -> (Option<Value>, Option<String>) {
    let Ok(text) = fs::read_to_string(path) else { return (None, None) };
    let bad = |why: String| {
        let _ = fs::rename(path, path.with_extension("json.bad"));
        (None, Some(format!("{}: {why}; renamed to state.json.bad and using defaults", path.display())))
    };
    match serde_json::from_str::<Value>(&text) {
        Err(e) => bad(format!("invalid JSON ({e})")),
        Ok(v) => match v.get("version").and_then(Value::as_u64) {
            Some(VERSION) => (Some(v), None),
            other => bad(format!("unknown state version {other:?}")),
        },
    }
}

/// OS window size saved last time, read leniently (no renaming) for use before the webview exists.
pub fn saved_window_size() -> Option<(f64, f64)> {
    let v: Value = serde_json::from_str(&fs::read_to_string(state_path()).ok()?).ok()?;
    let w = v.get("os_window")?;
    Some((w.get("w")?.as_f64()?, w.get("h")?.as_f64()?))
}

#[tauri::command]
pub fn load_state() -> Loaded {
    let (state, message) = read_state(&state_path());
    Loaded { state, no_anim: std::env::args().any(|a| a == "--no-anim"), message }
}

#[tauri::command]
pub fn save_state(state: Value) -> Result<(), String> {
    let p = state_path();
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    let tmp = p.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
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
    fn valid_state_loads() {
        let p = tmp("ok");
        fs::write(&p, r#"{"version":1,"scale":2}"#).unwrap();
        let (s, msg) = read_state(&p);
        assert_eq!(s.unwrap()["scale"], 2);
        assert!(msg.is_none());
    }

    #[test]
    fn corrupt_state_is_renamed() {
        let p = tmp("corrupt");
        fs::write(&p, "{ not json").unwrap();
        let (s, msg) = read_state(&p);
        assert!(s.is_none());
        assert!(msg.unwrap().contains("state.json.bad"));
        assert!(!p.exists());
        assert!(p.with_extension("json.bad").exists());
    }

    #[test]
    fn unknown_version_is_renamed() {
        let p = tmp("version");
        fs::write(&p, r#"{"version":99}"#).unwrap();
        let (s, msg) = read_state(&p);
        assert!(s.is_none());
        assert!(msg.is_some());
        assert!(p.with_extension("json.bad").exists());
    }

    #[test]
    fn missing_file_is_silent() {
        let (s, msg) = read_state(Path::new("/nonexistent/state.json"));
        assert!(s.is_none() && msg.is_none());
    }
}
