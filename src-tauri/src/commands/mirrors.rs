use tauri::{command, AppHandle, State};

use crate::models::error::AppError;
use crate::models::mirror::{list_mirrors, Mirror};
use crate::services::directory_manager::get_mirrors_dir;
use crate::AppState;

/// List every mirror available to the launcher (built-in + user-imported).
#[command]
pub async fn get_available_mirrors() -> Result<Vec<Mirror>, AppError> {
    list_mirrors()
}

/// Select the active mirror and persist the change to disk so the
/// choice survives restarts. The previous implementation forgot to
/// call `save()`.
#[command]
pub async fn set_mirror(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    mirror: Mirror,
) -> Result<(), AppError> {
    {
        let mut cfg = state.config.write().await;
        cfg.download_settings.mirror = mirror;
        cfg.write_to_file()?;
    }
    // Mirror is persisted as its own JSON file so the user can share it.
    let _ = app_handle;
    Ok(())
}

/// Return the currently-active mirror.
#[command]
pub async fn get_mirror(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Mirror, AppError> {
    let _ = app_handle;
    Ok(state.config.read().await.download_settings.mirror.clone())
}

/// Import a mirror definition from a JSON string. Persists it to the
/// mirrors directory under `<name>.json` so it survives restarts.
///
/// Returns the updated full list of mirrors so the frontend can
/// refresh its picker without a follow-up `get_available_mirrors` call.
#[command]
pub async fn import_mirror(json: String) -> Result<Vec<Mirror>, AppError> {
    let mirror: Mirror = serde_json::from_str(json.as_str())
        .map_err(|e| AppError::JsonParseFailed(format!("invalid mirror json: {e}")))?;
    let dir = get_mirrors_dir();
    std::fs::create_dir_all(&dir).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    let file = dir.join(format!("{}.json", mirror.name.to_lowercase()));
    std::fs::write(&file, json).map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
    list_mirrors()
}
