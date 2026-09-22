use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::models::error::AppError;
use crate::models::java::Java;
use crate::services::directory_manager::auto_detect_javas;
use crate::AppState;

/// A user-visible Java installation with its detected major version.
///
/// `source` is `"launcher"` for runtimes managed by Glitchy, or
/// `"system"` for runtimes detected on the user's PATH / common
/// install locations.
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectedJava {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub source: String,
}

/// Detect every Java runtime visible to the launcher.
///
/// Combines launcher-managed runtimes (under `<mc>/runtime/`) with
/// system-installed JREs in well-known paths. The frontend uses this
/// to populate the manual Java picker in Settings.
#[command]
pub async fn list_detected_javas() -> Result<Vec<DetectedJava>, AppError> {
    let mut out = Vec::new();

    // Launcher-managed runtimes (downloaded by the JDK manager).
    let runtime_root = crate::services::directory_manager::get_launcher_java_directory();
    if runtime_root.exists() {
        if let Ok(read_dir) = std::fs::read_dir(&runtime_root) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let java = Java::new(path);
                let major = java.version_major_from_release().unwrap_or(0);
                out.push(DetectedJava {
                    path: java.path.to_string_lossy().to_string(),
                    version: java.version.clone(),
                    major,
                    source: "launcher".to_string(),
                });
            }
        }
    }

    // System-detected runtimes.
    let system = auto_detect_javas()?;
    for java in system {
        let major = java.version_major_from_release().unwrap_or(0);
        out.push(DetectedJava {
            path: java.path.to_string_lossy().to_string(),
            version: java.version.clone(),
            major,
            source: "system".to_string(),
        });
    }

    Ok(out)
}

/// Currently configured Java override (when the user picks one
/// explicitly). `null` means "automatic selection based on version
/// manifest".
#[command]
pub async fn get_selected_java(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    let cfg = state.config.read().await;
    Ok(cfg.launch_options.java_override.clone())
}

/// Set or clear the manual Java override. Pass `null` to revert to
/// automatic selection.
#[command]
pub async fn set_selected_java(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<(), AppError> {
    let mut cfg = state.config.write().await;
    cfg.launch_options.java_override = path;
    let _ = cfg.write_to_file();
    Ok(())
}
