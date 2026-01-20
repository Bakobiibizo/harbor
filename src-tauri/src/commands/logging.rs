use crate::logging;
use crate::LogDirectory;
use tauri::State;

#[tauri::command]
pub fn export_logs(log_dir: State<LogDirectory>) -> Result<String, String> {
    logging::export_logs(&log_dir.0).map_err(|e| format!("Failed to export logs: {}", e))
}

#[tauri::command]
pub fn get_log_path(log_dir: State<LogDirectory>) -> String {
    log_dir.0.to_string_lossy().to_string()
}

#[tauri::command]
pub fn cleanup_logs(log_dir: State<LogDirectory>, max_files: usize) -> Result<(), String> {
    logging::cleanup_old_logs(&log_dir.0, max_files)
        .map_err(|e| format!("Failed to cleanup logs: {}", e))
}
