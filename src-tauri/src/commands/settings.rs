use tauri::{command, State};

use crate::models::config::Config;
use crate::models::error::AppError;
use crate::models::profiles::{get_profile, Profile};
use crate::AppState;

#[command]
pub async fn set_maximum_ram_usage(
    state: State<'_, AppState>,
    ram_usage: u64,
) -> Result<(), AppError> {
    let mut config = state.config.write().await;
    config.launch_options.ram_usage_max = ram_usage;
    Ok(())
}

#[command]
pub async fn get_maximum_ram_usage(state: State<'_, AppState>) -> Result<u64, AppError> {
    Ok(state.config.read().await.launch_options.ram_usage_max)
}

#[command]
pub async fn set_minimum_ram_usage(
    state: State<'_, AppState>,
    ram_usage: u64,
) -> Result<(), AppError> {
    let mut config = state.config.write().await;
    config.launch_options.ram_usage_min = ram_usage;
    Ok(())
}

#[command]
pub async fn get_minimum_ram_usage(state: State<'_, AppState>) -> Result<u64, AppError> {
    Ok(state.config.read().await.launch_options.ram_usage_min)
}

#[command]
pub async fn get_language(state: State<'_, AppState>) -> Result<String, AppError> {
    Ok(state.config.read().await.launcher_settings.language.clone())
}

#[command]
pub async fn set_language(state: State<'_, AppState>, lang: String) -> Result<(), AppError> {
    let mut config = state.config.write().await;
    config.launcher_settings.language = lang;
    Ok(())
}

#[command]
pub async fn should_exit_on_launch(state: State<'_, AppState>) -> Result<bool, AppError> {
    Ok(state.config.read().await.launcher_settings.exit_on_launch)
}

#[command]
pub async fn set_exit_on_launch(
    state: State<'_, AppState>,
    toggle: bool,
) -> Result<(), AppError> {
    let mut config = state.config.write().await;
    config.launcher_settings.exit_on_launch = toggle;
    Ok(())
}

#[command]
pub async fn save(state: State<'_, AppState>) -> Result<(), AppError> {
    let cfg = state.config.read().await;
    cfg.write_to_file()
}

#[command]
pub async fn get_total_ram() -> Result<u64, AppError> {
    let ram = sys_info::mem_info()
        .map_err(|e| AppError::UnknownError(format!("mem_info failed: {e}")))?;
    Ok(ram.total)
}

#[command]
pub async fn set_config(state: State<'_, AppState>, config: Config) -> Result<(), AppError> {
    let mut cfg = state.config.write().await;
    cfg.launch_options = config.launch_options;
    cfg.launcher_settings = config.launcher_settings;
    cfg.write_to_file()
}

/// Get the currently-selected profile, or `null` when none exists.
#[command]
pub async fn get_selected_profile(
    state: State<'_, AppState>,
) -> Result<Option<Profile>, AppError> {
    let cfg = state.config.read().await;
    let uuid = cfg.launch_options.selected_profile;
    Ok(get_profile(&uuid))
}

#[command]
pub async fn set_selected_profile(
    state: State<'_, AppState>,
    profile: Profile,
) -> Result<(), AppError> {
    let mut cfg = state.config.write().await;
    cfg.launch_options.selected_profile = profile.uuid;
    Ok(())
}
