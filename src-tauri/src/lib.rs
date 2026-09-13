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
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            win.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![log, platform, quit])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
