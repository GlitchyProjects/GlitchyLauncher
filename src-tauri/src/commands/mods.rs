use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::warn;
use serde::Serialize;
use tauri::{command, AppHandle};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::fs::copy;
use zip::ZipArchive;

use crate::models::error::AppError;
use crate::models::mods::{BackpackCategory, ModInfo};
use crate::services::directory_manager::{ensure_instance_dirs, get_category_folder};
use crate::services::mod_manager::{load_mod, load_pack, set_item_enabled};
use crate::services::instance_manager::is_managed_instance_path;
use crate::services::glitchy_store::{load_journey, load_profile};

/// The Minecraft version + mod loader encoded in an installed version id.
/// Returned to the frontend so the Modrinth browser can filter search
/// results to items that are actually compatible with the instance.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub game_version: String,
    /// Modrinth loader slug ("fabric", "forge", "neoforge") or an empty
    /// string for vanilla instances (which cannot filter by loader).
    pub loader: String,
}

/// Split an installed version id into its Minecraft version and loader.
/// Recognized shapes produced by this launcher's downloader:
///   `1.20.1`                      -> ("1.20.1", "")
///   `1.18.2-forge-40.2.0`         -> ("1.18.2", "forge")
///   `fabric-loader-0.16.9-1.20.1` -> ("1.20.1", "fabric")
#[command]
pub async fn get_instance_info(instance_id: String) -> Result<InstanceInfo, AppError> {
    let settings = crate::services::instance_manager::get_instance_settings(&instance_id);
    if let Some(game_version) = settings.game_version {
        return Ok(InstanceInfo {
            game_version,
            loader: settings.loader.unwrap_or_default(),
        });
    }
    let lower = instance_id.to_lowercase();
    if lower.starts_with("fabric-loader-") {
        let rest = &instance_id["fabric-loader-".len()..];
        // rest is `<loader-ver>-<mc-version>`; keep everything after the
        // first dash so MC versions like `1.20.2-rc1` stay intact.
        let mc = rest.splitn(2, '-').nth(1).unwrap_or(rest);
        return Ok(InstanceInfo {
            game_version: mc.to_string(),
            loader: "fabric".to_string(),
        });
    }
    if lower.contains("neoforge") {
        let mc = instance_id.split("-neoforge-").next().unwrap_or(&instance_id);
        return Ok(InstanceInfo {
            game_version: mc.to_string(),
            loader: "neoforge".to_string(),
        });
    }
    if lower.contains("optifine") {
        let mc = lower.split("-optifine").next().unwrap_or(&instance_id);
        let original_mc = &instance_id[..mc.len()];
        return Ok(InstanceInfo {
            game_version: original_mc.to_string(),
            loader: "optifine".to_string(),
        });
    }
    if lower.contains("forge") {
        let mc = instance_id.split("-forge-").next().unwrap_or(&instance_id);
        return Ok(InstanceInfo {
            game_version: mc.to_string(),
            loader: "forge".to_string(),
        });
    }
    Ok(InstanceInfo {
        game_version: instance_id,
        loader: String::new(),
    })
}

/// Guard for destructive per-item operations: the target file must live
/// inside the instances root (following symlinks) so a crafted path can
/// never rename or delete files outside an instance folder.
fn validate_item_path(path: &Path) -> Result<(), AppError> {
    if is_managed_instance_path(path) {
        Ok(())
    } else {
        Err(AppError::PathValidationFailed(format!(
            "path is outside the instances directory: {}",
            path.display()
        )))
    }
}

#[command]
pub async fn toggle_backpack_item(
    item: ModInfo,
    toggle: bool,
    category: BackpackCategory,
) -> Result<(), AppError> {
    let path = PathBuf::from(&item.path);
    validate_item_path(&path)?;
    set_item_enabled(&path, toggle, category)
}

/// List every item in the instance's own folder for the given category.
/// Mods are parsed from their JAR metadata; packs are listed as zip
/// files with a best-effort `pack.mcmeta` description. Items that fail
/// to parse are skipped (with a warning) rather than aborting the list.
#[command]
pub async fn get_backpack_items(
    version_id: String,
    category: BackpackCategory,
) -> Result<Vec<ModInfo>, AppError> {
    ensure_instance_dirs(&version_id).await?;
    let directory = get_category_folder(&version_id, category.folder_name())?;
    let mut items: Vec<ModInfo> = Vec::new();
    let read_dir = match std::fs::read_dir(&directory) {
        Ok(d) => d,
        Err(e) => {
            return Err(AppError::FileReadFailed(format!(
                "{} folder unreadable: {e}",
                category.folder_name()
            )))
        }
    };

    let (enabled_ext, disabled_ext) = match category {
        BackpackCategory::Mods => ("jar", "disabled"),
        BackpackCategory::ResourcePacks | BackpackCategory::ShaderPacks => ("zip", "zip.disabled"),
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else { continue; };
        let lower = name.to_lowercase();
        let enabled = lower.ends_with(&format!(".{enabled_ext}"))
            && !lower.ends_with(&format!(".{disabled_ext}"));
        let disabled = lower.ends_with(&format!(".{disabled_ext}"));
        if !enabled && !disabled { continue; }
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };

        match category {
            BackpackCategory::Mods => {
                let file = match File::open(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("failed to open mod {}: {e}", path.display());
                        continue;
                    }
                };
                let zip = match ZipArchive::new(file) {
                    Ok(z) => z,
                    Err(e) => {
                        warn!("invalid zip {}: {e}", path.display());
                        continue;
                    }
                };
                match load_mod(Mutex::new(zip), path_str) {
                    Ok(info) => items.push(info),
                    Err(e) => {
                        warn!("failed to parse mod metadata for {}: {e:?}", path.display());
                        continue;
                    }
                }
            }
            BackpackCategory::ResourcePacks | BackpackCategory::ShaderPacks => {
                let file = match File::open(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("failed to open pack {}: {e}", path.display());
                        continue;
                    }
                };
                let mut zip = match ZipArchive::new(file) {
                    Ok(z) => z,
                    Err(e) => {
                        warn!("invalid zip {}: {e}", path.display());
                        continue;
                    }
                };
                items.push(load_pack(&mut zip, &path_str, enabled));
            }
        }
    }
    Ok(items)
}

#[command]
pub async fn import_backpack_item(
    app: AppHandle,
    version_id: String,
    category: BackpackCategory,
) -> Result<(), AppError> {
    ensure_instance_dirs(&version_id).await?;
    let dest_folder = get_category_folder(&version_id, category.folder_name())?;
    let paths = app
        .dialog()
        .file()
        .add_filter(
            format!("Minecraft {}", category.label()),
            category.import_extensions(),
        )
        .blocking_pick_files()
        .unwrap_or_default();
    for path in paths.iter().filter_map(|x| x.as_path()) {
        let Some(file_name) = path.file_name() else { continue; };
        let new_path = dest_folder.join(file_name);
        copy(path, new_path)
            .await
            .map_err(|e| AppError::FileCopyFailed(e.to_string()))?;
    }
    if !paths.is_empty() {
        recheck_achievements(&app)?;
    }
    Ok(())
}

#[command]
pub async fn delete_backpack_item(app: AppHandle, item: ModInfo) -> Result<(), AppError> {
    let path = PathBuf::from(&item.path);
    validate_item_path(&path)?;
    std::fs::remove_file(&path).map_err(|e| AppError::FileDeleteFailed(e.to_string()))?;
    recheck_achievements(&app)?;
    Ok(())
}

/// Open the instance's category folder in the OS file manager. Uses the
/// Tauri `opener` plugin which calls the native file explorer (Explorer
/// on Windows, Finder on macOS, xdg-open on Linux).
#[command]
pub async fn open_backpack_folder(
    app: AppHandle,
    version_id: String,
    category: BackpackCategory,
) -> Result<(), AppError> {
    ensure_instance_dirs(&version_id).await?;
    let dir = get_category_folder(&version_id, category.folder_name())?;
    let path_str = dir.to_string_lossy().to_string();
    app.opener()
        .open_path(path_str, None::<&str>)
        .map_err(|e| AppError::UnknownError(format!("failed to open folder: {e}")))?;
    Ok(())
}

/// Re-evaluate achievements/badges after backpack items change so badges
/// like "Modder" unlock immediately without waiting for the next game
/// launch.
pub(crate) fn recheck_achievements(app: &AppHandle) -> Result<(), AppError> {
    let mut profile = load_profile();
    let mut journey = load_journey();
    crate::commands::glitchy::refresh_and_emit(app, &mut profile, &mut journey)?;
    Ok(())
}
