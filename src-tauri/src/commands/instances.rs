use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use tauri::{command, AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::models::error::AppError;
use crate::services::directory_manager::{
    ensure_instance_dirs, get_falcon_launcher_directory, get_instance_directory,
    get_instances_directory, get_version_directory,
};
use crate::services::instance_manager::{
    copy_directory, directory_size, get_instance_settings, remove_instance_settings,
    save_instance_settings, validate_custom_instance_path, verify_instance_marker,
    write_instance_marker, InstanceSettings,
};
use crate::services::path_safety::{sanitize_archive_entry, sanitize_user_path_segment};
use crate::services::repair::{repair_version, RepairReport};
use crate::services::version_manager::reload_installed_versions;
use crate::{AppState, GLOBAL_CACHE};

const MIN_RAM_MB: u64 = 512;
const MAX_RAM_MB: u64 = 65_536;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSummary {
    id: String,
    display_name: String,
    path: String,
    game_version: String,
    loader: String,
    ram_min_mb: Option<u64>,
    ram_max_mb: Option<u64>,
    size_bytes: u64,
    mod_count: usize,
    world_count: usize,
    backup_count: usize,
    modpack_name: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSummary {
    name: String,
    path: String,
    size_bytes: u64,
    modified_at: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    name: String,
    path: String,
    size_bytes: u64,
    created_at: u64,
}

fn count_entries(path: &Path, extension: Option<&str>) -> usize {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    let path = entry.path();
                    if let Some(extension) = extension {
                        path.is_file()
                            && path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(|name| name.ends_with(extension))
                    } else {
                        path.is_dir()
                    }
                })
                .count()
        })
        .unwrap_or(0)
}

fn infer_game_and_loader(instance_id: &str) -> (String, String) {
    let lower = instance_id.to_ascii_lowercase();
    if lower.starts_with("fabric-loader-") {
        let rest = &instance_id["fabric-loader-".len()..];
        let game = rest.splitn(2, '-').nth(1).unwrap_or(rest);
        return (game.to_string(), "fabric".to_string());
    }
    for loader in ["neoforge", "forge"] {
        let marker = format!("-{loader}-");
        if lower.contains(&marker) {
            return (
                instance_id
                    .split(&marker)
                    .next()
                    .unwrap_or(instance_id)
                    .to_string(),
                loader.to_string(),
            );
        }
    }
    if lower.contains("optifine") {
        return (
            instance_id
                .split("-OptiFine_")
                .next()
                .unwrap_or(instance_id)
                .to_string(),
            "optifine".to_string(),
        );
    }
    (instance_id.to_string(), "vanilla".to_string())
}

fn read_modpack_name(instance_path: &Path) -> Option<String> {
    let text = fs::read_to_string(instance_path.join(".glitchy-modpack.json")).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    ["name", "title", "versionName", "projectId"]
        .iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::to_string)
}

fn modified_millis(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn backup_directory(instance_id: &str) -> Result<PathBuf, AppError> {
    sanitize_user_path_segment(
        &get_falcon_launcher_directory().join("backups"),
        instance_id,
    )
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), AppError> {
    for entry in
        fs::read_dir(directory).map_err(|error| AppError::FileReadFailed(error.to_string()))?
    {
        let entry = entry.map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_files(root, &entry.path(), files)?;
        } else if file_type.is_file() && entry.path().starts_with(root) {
            files.push(entry.path());
        }
    }
    Ok(())
}

#[command]
pub async fn list_instances() -> Result<Vec<InstanceSummary>, AppError> {
    let ids = {
        let cache = GLOBAL_CACHE.lock().await;
        cache
            .versions
            .iter()
            .filter(|version| version.is_installed())
            .map(|version| version.id.clone())
            .collect::<Vec<_>>()
    };
    let mut instances = Vec::with_capacity(ids.len());
    for id in ids {
        ensure_instance_dirs(&id).await?;
        let path = get_instance_directory(&id)?;
        let settings = get_instance_settings(&id);
        let (inferred_game_version, inferred_loader) = infer_game_and_loader(&id);
        let game_version = settings
            .game_version
            .clone()
            .unwrap_or(inferred_game_version);
        let loader = settings.loader.clone().unwrap_or(inferred_loader);
        instances.push(InstanceSummary {
            id: id.clone(),
            display_name: if settings.display_name.trim().is_empty() {
                id.clone()
            } else {
                settings.display_name
            },
            path: path.to_string_lossy().to_string(),
            game_version,
            loader,
            ram_min_mb: settings.ram_min_mb,
            ram_max_mb: settings.ram_max_mb,
            size_bytes: directory_size(&path),
            mod_count: count_entries(&path.join("mods"), Some(".jar"))
                + count_entries(&path.join("mods"), Some(".disabled")),
            world_count: count_entries(&path.join("saves"), None),
            backup_count: count_entries(&backup_directory(&id)?, Some(".zip")),
            modpack_name: read_modpack_name(&path),
        });
    }
    instances.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    Ok(instances)
}

#[command]
pub async fn update_instance_settings(
    instance_id: String,
    display_name: String,
    ram_min_mb: Option<u64>,
    ram_max_mb: Option<u64>,
) -> Result<(), AppError> {
    if display_name.trim().is_empty() || display_name.chars().count() > 80 {
        return Err(AppError::PathValidationFailed(
            "display name must be between 1 and 80 characters".to_string(),
        ));
    }
    if let (Some(minimum), Some(maximum)) = (ram_min_mb, ram_max_mb) {
        if minimum < MIN_RAM_MB || maximum > MAX_RAM_MB || minimum > maximum {
            return Err(AppError::PathValidationFailed(format!(
                "RAM must be between {MIN_RAM_MB} and {MAX_RAM_MB} MB, with minimum <= maximum"
            )));
        }
    }
    let mut settings = get_instance_settings(&instance_id);
    settings.display_name = display_name.trim().to_string();
    settings.ram_min_mb = ram_min_mb;
    settings.ram_max_mb = ram_max_mb;
    save_instance_settings(&instance_id, settings)
}

#[command]
pub async fn set_instance_path(
    instance_id: String,
    new_path: String,
    move_files: bool,
) -> Result<(), AppError> {
    if crate::services::game_launcher::is_instance_running(&instance_id) {
        return Err(AppError::UnknownError(format!(
            "اینستنس '{instance_id}' در حال حاضر در حال اجرا است. لطفاً ابتدا بازی را ببندید."
        )));
    }
    let old_path = get_instance_directory(&instance_id)?;
    let destination = validate_custom_instance_path(Path::new(new_path.trim()))?;
    if old_path == destination {
        return Ok(());
    }
    if move_files
        && destination.exists()
        && fs::read_dir(&destination)
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?
            .next()
            .is_some()
    {
        return Err(AppError::PathValidationFailed(
            "destination must be empty when moving instance files".to_string(),
        ));
    }
    if move_files {
        copy_directory(&old_path, &destination)?;
    } else {
        fs::create_dir_all(&destination)
            .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    }
    write_instance_marker(&destination, &instance_id)?;
    let mut settings = get_instance_settings(&instance_id);
    settings.custom_path = Some(destination.to_string_lossy().to_string());
    save_instance_settings(&instance_id, settings)?;
    if move_files && old_path.exists() {
        fs::remove_dir_all(&old_path)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    ensure_instance_dirs(&instance_id).await
}

#[command]
pub async fn reset_instance_path(instance_id: String) -> Result<(), AppError> {
    if crate::services::game_launcher::is_instance_running(&instance_id) {
        return Err(AppError::UnknownError(format!(
            "اینستنس '{instance_id}' در حال حاضر در حال اجرا است. لطفاً ابتدا بازی را ببندید."
        )));
    }
    let old_path = get_instance_directory(&instance_id)?;
    let destination = sanitize_user_path_segment(&get_instances_directory(), &instance_id)?;
    if old_path != destination {
        if destination.exists()
            && fs::read_dir(&destination)
                .map_err(|error| AppError::FileReadFailed(error.to_string()))?
                .next()
                .is_some()
        {
            return Err(AppError::PathValidationFailed(
                "default instance directory is not empty".to_string(),
            ));
        }
        copy_directory(&old_path, &destination)?;
        write_instance_marker(&destination, &instance_id)?;
    }
    let mut settings = get_instance_settings(&instance_id);
    settings.custom_path = None;
    save_instance_settings(&instance_id, settings)?;
    if old_path != destination && old_path.exists() {
        fs::remove_dir_all(old_path)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    ensure_instance_dirs(&instance_id).await
}

#[command]
pub async fn clone_instance(instance_id: String, new_instance_id: String) -> Result<(), AppError> {
    let new_id = new_instance_id.trim();
    sanitize_user_path_segment(&get_instances_directory(), new_id)?;
    let source_version = get_version_directory(&instance_id);
    let destination_version = get_version_directory(new_id);
    let source_instance = get_instance_directory(&instance_id)?;
    let destination_instance = sanitize_user_path_segment(&get_instances_directory(), new_id)?;
    if destination_version.exists() || destination_instance.exists() {
        return Err(AppError::PathValidationFailed(format!(
            "an instance named '{new_id}' already exists"
        )));
    }

    copy_directory(&source_version, &destination_version)?;
    copy_directory(&source_instance, &destination_instance)?;
    write_instance_marker(&destination_instance, new_id)?;

    let copied_manifest = destination_version.join(format!("{instance_id}.json"));
    let new_manifest = destination_version.join(format!("{new_id}.json"));
    if copied_manifest.exists() {
        let text = fs::read_to_string(&copied_manifest)
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        let mut json: Value = serde_json::from_str(&text)
            .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
        json["id"] = Value::String(new_id.to_string());
        fs::write(
            &new_manifest,
            serde_json::to_vec_pretty(&json)
                .map_err(|error| AppError::JsonParseFailed(error.to_string()))?,
        )
        .map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
        fs::remove_file(copied_manifest)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    let copied_jar = destination_version.join(format!("{instance_id}.jar"));
    if copied_jar.exists() {
        fs::rename(
            &copied_jar,
            destination_version.join(format!("{new_id}.jar")),
        )
        .map_err(|error| AppError::FileRenameFailed(error.to_string()))?;
    }

    let source_settings = get_instance_settings(&instance_id);
    save_instance_settings(
        new_id,
        InstanceSettings {
            display_name: new_id.to_string(),
            custom_path: None,
            ram_min_mb: source_settings.ram_min_mb,
            ram_max_mb: source_settings.ram_max_mb,
            game_version: Some(infer_game_and_loader(&instance_id).0),
            loader: Some(infer_game_and_loader(&instance_id).1),
        },
    )?;
    reload_installed_versions().await;
    Ok(())
}

#[command]
pub async fn delete_instance(instance_id: String) -> Result<(), AppError> {
    if crate::services::game_launcher::is_instance_running(&instance_id) {
        return Err(AppError::UnknownError(format!(
            "اینستنس '{instance_id}' در حال حاضر در حال اجرا است. لطفاً ابتدا بازی را ببندید."
        )));
    }
    let instance_path = get_instance_directory(&instance_id)?;
    let default_path = sanitize_user_path_segment(&get_instances_directory(), &instance_id)?;
    if instance_path != default_path {
        verify_instance_marker(&instance_path, &instance_id)?;
    }
    let version_path = get_version_directory(&instance_id);
    if instance_path.exists() {
        fs::remove_dir_all(&instance_path)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    if version_path.exists() {
        fs::remove_dir_all(&version_path)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    remove_instance_settings(&instance_id)?;
    reload_installed_versions().await;
    Ok(())
}

#[command]
pub async fn reinstall_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<RepairReport, AppError> {
    repair_version(&instance_id, true, &app, &state).await
}

#[command]
pub async fn list_instance_worlds(instance_id: String) -> Result<Vec<WorldSummary>, AppError> {
    let saves = get_instance_directory(&instance_id)?.join("saves");
    fs::create_dir_all(&saves).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let mut worlds = fs::read_dir(&saves)
        .map_err(|error| AppError::FileReadFailed(error.to_string()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().into_string().ok()?;
            Some(WorldSummary {
                name,
                path: path.to_string_lossy().to_string(),
                size_bytes: directory_size(&path),
                modified_at: modified_millis(&path),
            })
        })
        .collect::<Vec<_>>();
    worlds.sort_by(|left, right| right.modified_at.cmp(&left.modified_at));
    Ok(worlds)
}

#[command]
pub async fn delete_instance_world(
    instance_id: String,
    world_name: String,
) -> Result<(), AppError> {
    let saves = get_instance_directory(&instance_id)?.join("saves");
    let world = sanitize_user_path_segment(&saves, &world_name)?;
    if world.exists() {
        fs::remove_dir_all(world).map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    Ok(())
}

#[command]
pub async fn create_instance_backup(instance_id: String) -> Result<BackupSummary, AppError> {
    let instance_path = get_instance_directory(&instance_id)?;
    ensure_instance_dirs(&instance_id).await?;
    let backups = backup_directory(&instance_id)?;
    fs::create_dir_all(&backups).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let name = format!("instance-{timestamp}.zip");
    let path = backups.join(&name);
    let file =
        File::create(&path).map_err(|error| AppError::FileCreateFailed(error.to_string()))?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut files = Vec::new();
    collect_files(&instance_path, &instance_path, &mut files)?;
    let mut buffer = Vec::new();
    for source in files {
        let relative = source
            .strip_prefix(&instance_path)
            .map_err(|error| AppError::PathValidationFailed(error.to_string()))?;
        let entry_name = relative.to_string_lossy().replace('\\', "/");
        archive
            .start_file(entry_name, options)
            .map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;
        buffer.clear();
        File::open(&source)
            .and_then(|mut input| input.read_to_end(&mut buffer))
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        archive
            .write_all(&buffer)
            .map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
    }
    archive
        .finish()
        .map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;
    Ok(BackupSummary {
        name,
        path: path.to_string_lossy().to_string(),
        size_bytes: fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        created_at: timestamp,
    })
}

#[command]
pub async fn list_instance_backups(instance_id: String) -> Result<Vec<BackupSummary>, AppError> {
    let backups = backup_directory(&instance_id)?;
    fs::create_dir_all(&backups).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let mut results = fs::read_dir(&backups)
        .map_err(|error| AppError::FileReadFailed(error.to_string()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !name.ends_with(".zip") {
                return None;
            }
            let path = entry.path();
            Some(BackupSummary {
                name,
                path: path.to_string_lossy().to_string(),
                size_bytes: fs::metadata(&path)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
                created_at: modified_millis(&path),
            })
        })
        .collect::<Vec<_>>();
    results.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(results)
}

#[command]
pub async fn restore_instance_backup(
    instance_id: String,
    backup_name: String,
) -> Result<(), AppError> {
    let backups = backup_directory(&instance_id)?;
    let backup = sanitize_user_path_segment(&backups, &backup_name)?;
    if !backup.is_file() {
        return Err(AppError::FileNotFound(backup.display().to_string()));
    }
    let instance_path = get_instance_directory(&instance_id)?;
    let parent = instance_path
        .parent()
        .ok_or_else(|| AppError::PathValidationFailed("instance path has no parent".to_string()))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let extracted = parent.join(format!(".glitchy-restore-{stamp}"));
    let rollback = parent.join(format!(".glitchy-rollback-{stamp}"));
    fs::create_dir_all(&extracted).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;

    let file = File::open(&backup).map_err(|error| AppError::FileReadFailed(error.to_string()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| AppError::ZipParseFailed(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::ZipParseFailed(error.to_string()))?;
        let destination = sanitize_archive_entry(&extracted, entry.name())?;
        if entry.is_dir() {
            fs::create_dir_all(destination)
                .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
        }
        let mut output = File::create(destination)
            .map_err(|error| AppError::FileCreateFailed(error.to_string()))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| AppError::FileCopyFailed(error.to_string()))?;
    }

    if instance_path.exists() {
        fs::rename(&instance_path, &rollback)
            .map_err(|error| AppError::FileRenameFailed(error.to_string()))?;
    }
    if let Err(error) = fs::rename(&extracted, &instance_path) {
        if rollback.exists() {
            let _ = fs::rename(&rollback, &instance_path);
        }
        return Err(AppError::FileRenameFailed(error.to_string()));
    }
    write_instance_marker(&instance_path, &instance_id)?;
    if rollback.exists() {
        fs::remove_dir_all(rollback)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    Ok(())
}

#[command]
pub async fn delete_instance_backup(
    instance_id: String,
    backup_name: String,
) -> Result<(), AppError> {
    let backup = sanitize_user_path_segment(&backup_directory(&instance_id)?, &backup_name)?;
    if backup.exists() {
        fs::remove_file(backup).map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    Ok(())
}

#[command]
pub async fn open_instance_folder(
    app: AppHandle,
    instance_id: String,
    folder: String,
) -> Result<(), AppError> {
    let instance_path = get_instance_directory(&instance_id)?;
    let path = match folder.as_str() {
        "root" => instance_path,
        "mods" | "resourcepacks" | "shaderpacks" | "saves" => instance_path.join(folder),
        "backups" => backup_directory(&instance_id)?,
        _ => {
            return Err(AppError::PathValidationFailed(format!(
                "unsupported instance folder: {folder}"
            )))
        }
    };
    fs::create_dir_all(&path).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| AppError::UnknownError(format!("failed to open folder: {error}")))
}

#[cfg(test)]
mod tests {
    use super::infer_game_and_loader;

    #[test]
    fn infers_common_loader_version_ids() {
        assert_eq!(
            infer_game_and_loader("fabric-loader-0.16.14-1.21.5"),
            ("1.21.5".to_string(), "fabric".to_string())
        );
        assert_eq!(
            infer_game_and_loader("1.20.1-forge-47.4.0"),
            ("1.20.1".to_string(), "forge".to_string())
        );
        assert_eq!(
            infer_game_and_loader("1.21.1-neoforge-21.1.172"),
            ("1.21.1".to_string(), "neoforge".to_string())
        );
        assert_eq!(
            infer_game_and_loader("1.20.1-OptiFine_HD_U_I6"),
            ("1.20.1".to_string(), "optifine".to_string())
        );
        assert_eq!(
            infer_game_and_loader("1.21.5"),
            ("1.21.5".to_string(), "vanilla".to_string())
        );
    }
}
