use std::fs;

use tauri::{command, State};
use uuid::Uuid;

use crate::models::error::{AppError, Void};
use crate::models::profiles::{self, Profile};
use crate::services::directory_manager::get_profiles_file;
use crate::services::utils::uuid_from_username;
use crate::AppState;

/// Return all profiles. Always succeeds (returns an empty array if the
/// file is missing or corrupt — recovery happens server-side).
#[command]
pub async fn get_profiles() -> Result<Vec<Profile>, AppError> {
    Ok(profiles::get_profiles())
}

/// Create a new offline profile and select it immediately so the user
/// can hit Play without an extra click.
#[command]
pub async fn create_offline_profile(
    state: State<'_, AppState>,
    username: String,
) -> Result<Profile, AppError> {
    let profile = profiles::create_new_profile(username, false)?;
    let mut cfg = state.config.write().await;
    cfg.launch_options.selected_profile = profile.uuid;
    let _ = cfg.write_to_file(); // best-effort persist
    Ok(profile)
}

/// Rename a profile. UUID is re-derived from the new name to stay
/// consistent with Minecraft's offline convention; the selected pointer
/// is updated if the renamed profile was the active one.
#[command]
pub async fn rename_profile(
    state: State<'_, AppState>,
    uuid: Uuid,
    new_username: String,
) -> Result<Profile, AppError> {
    let updated = profiles::rename_profile(uuid, new_username)?;
    let mut cfg = state.config.write().await;
    if cfg.launch_options.selected_profile == uuid {
        cfg.launch_options.selected_profile = updated.uuid;
    }
    let _ = cfg.write_to_file();
    Ok(updated)
}

/// Delete a profile. If the deleted profile was the selected one, the
/// launcher falls back to the most recent remaining profile (or `None`).
#[command]
pub async fn remove_profile(state: State<'_, AppState>, profile: Profile) -> Void {
    let removed_was_selected = {
        let cfg = state.config.read().await;
        cfg.launch_options.selected_profile == profile.uuid
    };

    profiles::delete_profile(profile.uuid)?;

    if removed_was_selected {
        let mut cfg = state.config.write().await;
        cfg.launch_options.selected_profile =
            profiles::fallback_profile().map(|p| p.uuid).unwrap_or_else(|| uuid_from_username("Player"));
        let _ = cfg.write_to_file();
    }
    Ok(())
}
