use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};

use crate::models::error::AppError;
use crate::services::directory_manager::{get_falcon_launcher_directory, get_instances_directory};
use crate::services::path_safety::{is_within_existing, sanitize_user_path_segment};

static REGISTRY_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSettings {
    pub display_name: String,
    pub custom_path: Option<String>,
    pub ram_min_mb: Option<u64>,
    pub ram_max_mb: Option<u64>,
    #[serde(default)]
    pub game_version: Option<String>,
    #[serde(default)]
    pub loader: Option<String>,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceRegistry {
    #[serde(default)]
    instances: HashMap<String, InstanceSettings>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceMarker {
    instance_id: String,
}

const INSTANCE_MARKER_FILE: &str = ".glitchy-instance.json";

fn registry_path() -> PathBuf {
    get_falcon_launcher_directory().join("instances.json")
}

fn load_registry_unlocked() -> InstanceRegistry {
    let path = registry_path();
    let Ok(text) = fs::read_to_string(path) else {
        return InstanceRegistry::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_registry_unlocked(registry: &InstanceRegistry) -> Result<(), AppError> {
    let path = registry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| AppError::DirCreateFailed(format!("{}: {error}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(registry)
        .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, json).map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
    fs::rename(&temporary, &path).map_err(|error| AppError::FileRenameFailed(error.to_string()))
}

pub fn get_instance_settings(instance_id: &str) -> InstanceSettings {
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    load_registry_unlocked()
        .instances
        .get(instance_id)
        .cloned()
        .unwrap_or_else(|| InstanceSettings {
            display_name: instance_id.to_string(),
            ..InstanceSettings::default()
        })
}

pub fn save_instance_settings(
    instance_id: &str,
    settings: InstanceSettings,
) -> Result<(), AppError> {
    sanitize_user_path_segment(&get_instances_directory(), instance_id)?;
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut registry = load_registry_unlocked();
    registry.instances.insert(instance_id.to_string(), settings);
    write_registry_unlocked(&registry)
}

pub fn remove_instance_settings(instance_id: &str) -> Result<(), AppError> {
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut registry = load_registry_unlocked();
    registry.instances.remove(instance_id);
    write_registry_unlocked(&registry)
}

pub fn configured_instance_path(instance_id: &str) -> Option<PathBuf> {
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    load_registry_unlocked()
        .instances
        .get(instance_id)
        .and_then(|settings| settings.custom_path.as_deref())
        .map(PathBuf::from)
}

pub fn validate_custom_instance_path(path: &Path) -> Result<PathBuf, AppError> {
    if !path.is_absolute() || path.parent().is_none() || path.file_name().is_none() {
        return Err(AppError::PathValidationFailed(
            "instance path must be an absolute, non-root directory".to_string(),
        ));
    }
    if path.exists() && !path.is_dir() {
        return Err(AppError::PathValidationFailed(format!(
            "instance path is not a directory: {}",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}

pub fn write_instance_marker(path: &Path, instance_id: &str) -> Result<(), AppError> {
    sanitize_user_path_segment(&get_instances_directory(), instance_id)?;
    fs::create_dir_all(path).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let data = serde_json::to_vec(&InstanceMarker {
        instance_id: instance_id.to_string(),
    })
    .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
    fs::write(path.join(INSTANCE_MARKER_FILE), data)
        .map_err(|error| AppError::FileWriteFailed(error.to_string()))
}

pub fn verify_instance_marker(path: &Path, instance_id: &str) -> Result<(), AppError> {
    let marker_path = path.join(INSTANCE_MARKER_FILE);
    let text = fs::read_to_string(&marker_path).map_err(|error| {
        AppError::AccessDenied(format!(
            "managed instance marker missing at {}: {error}",
            marker_path.display()
        ))
    })?;
    let marker: InstanceMarker = serde_json::from_str(&text)
        .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
    if marker.instance_id != instance_id {
        return Err(AppError::AccessDenied(format!(
            "instance marker belongs to '{}' instead of '{instance_id}'",
            marker.instance_id
        )));
    }
    Ok(())
}

pub fn is_managed_instance_path(path: &Path) -> bool {
    if is_within_existing(&get_instances_directory(), path) {
        return true;
    }
    let _guard = REGISTRY_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    load_registry_unlocked().instances.values().any(|settings| {
        settings
            .custom_path
            .as_deref()
            .map(Path::new)
            .is_some_and(|root| is_within_existing(root, path))
    })
}

pub fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() {
        return 0;
    }
    if metadata.is_file() {
        return metadata.len();
    }
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| directory_size(&entry.path()))
                .sum()
        })
        .unwrap_or(0)
}

pub fn copy_directory(source: &Path, destination: &Path) -> Result<(), AppError> {
    if !source.exists() {
        fs::create_dir_all(destination)
            .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
        return Ok(());
    }
    fs::create_dir_all(destination)
        .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    for entry in
        fs::read_dir(source).map_err(|error| AppError::FileReadFailed(error.to_string()))?
    {
        let entry = entry.map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        if file_type.is_symlink() {
            continue;
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)
                .map_err(|error| AppError::FileCopyFailed(error.to_string()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{verify_instance_marker, write_instance_marker};
    use std::fs;

    #[test]
    fn instance_marker_rejects_a_different_instance() {
        let directory =
            std::env::temp_dir().join(format!("glitchy-instance-marker-{}", uuid::Uuid::new_v4()));
        write_instance_marker(&directory, "1.21.5").expect("marker should be written");

        assert!(verify_instance_marker(&directory, "1.21.5").is_ok());
        assert!(verify_instance_marker(&directory, "different-instance").is_err());

        fs::remove_dir_all(directory).expect("temporary test directory should be removed");
    }
}
