use std::path::PathBuf;
use tauri::Manager;

fn resolve_download_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    // Try Tauri's download_dir first
    if let Ok(directory) = app.path().download_dir() {
        if directory.exists() || std::fs::create_dir_all(&directory).is_ok() {
            return Ok(directory);
        }
    }

    // Fallback: ~/Downloads
    if let Some(home) = std::env::var_os("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() || std::fs::create_dir_all(&downloads).is_ok() {
            return Ok(downloads);
        }
    }

    // Fallback: app data dir
    if let Ok(app_data) = app.path().app_data_dir() {
        if app_data.exists() || std::fs::create_dir_all(&app_data).is_ok() {
            return Ok(app_data);
        }
    }

    Err("Could not find a writable directory".to_string())
}

#[tauri::command]
pub fn save_to_downloads(
    app: tauri::AppHandle,
    filename: String,
    content: String,
) -> Result<String, String> {
    let download_dir = resolve_download_dir(&app)?;
    let file_path = download_dir.join(&filename);

    std::fs::write(&file_path, &content)
        .map_err(|error| format!("Failed to write file to {}: {}", file_path.display(), error))?;

    Ok(file_path.to_string_lossy().to_string())
}
