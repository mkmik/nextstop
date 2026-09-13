mod apps;
mod fs;
mod state;

use tauri::Manager;

#[tauri::command]
fn log(line: String) {
    eprintln!("{line}");
}

#[tauri::command]
fn platform() -> String {
    let os = match std::env::consts::OS { "macos" => "macOS", "windows" => "Windows", o => o };
    let arch = match std::env::consts::ARCH { "aarch64" => "arm64", "x86_64" => "x64", a => a };
    format!("{os} {arch}")
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let p = fs::check(&path)?;
    tauri_plugin_opener::open_path(p, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            if let Some((w, h)) = state::saved_window_size() {
                let _ = win.set_size(tauri::LogicalSize::new(w, h));
                let _ = win.center();
            }
            win.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            log, platform, quit, open_path,
            fs::list_dir, fs::home_dir, fs::root_dirs, fs::filter_existing, fs::file_meta, fs::read_text_head, fs::read_file_b64,
            fs::dir_size, fs::new_folder, fs::move_paths, fs::copy_paths, fs::duplicate_paths, fs::destroy_paths,
            fs::trash_paths, fs::trash_list, fs::trash_is_empty, fs::empty_trash,
            apps::launch_app, apps::open_with, apps::default_dock, apps::app_icon_png, apps::open_recycle_bin,
            state::load_state, state::save_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
