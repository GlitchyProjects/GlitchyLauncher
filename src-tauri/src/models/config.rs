use crate::models::error::AppError;
use crate::models::mirror::Mirror;
use crate::services::directory_manager::get_config_directory;
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;
use crate::services::utils;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct NativeLibraries {
    pub use_custom_glfw: bool,
    pub glfw_path: String,
    pub use_custom_openal: bool,
    pub openal_path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub selected_profile: Uuid,
    pub ram_usage_min: u64,
    pub ram_usage_max: u64,
    pub use_dedicated_gpu: bool,
    /// When set, the launcher uses this Java path instead of the
    /// manifest-recommended runtime. `None` means "automatic".
    #[serde(default)]
    pub java_override: Option<String>,
}
impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            selected_profile: utils::uuid_from_username("Player"),
            ram_usage_min: 1024,
            ram_usage_max: 2048,
            use_dedicated_gpu: true,
            java_override: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LauncherSettings {
    pub language: String,
    pub exit_on_launch: bool,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            exit_on_launch: false,
        }
    }
}

/// Mirror is serialized as its name string inside the config file. This
/// avoids embedding the full mirror maps (which can be large) into the
/// config and lets us look up the live mirror definition at runtime.
mod mirror_serialization {
    use super::*;
    use serde::{Deserializer, Serializer};
    use crate::models::mirror::mirror_from;

    pub fn serialize<S>(mirror: &Mirror, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&mirror.name)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Mirror, D::Error>
    where
        D: Deserializer<'de>,
    {
        let name = String::deserialize(deserializer)?;
        Ok(mirror_from(&name))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadSettings {
    #[serde(with = "mirror_serialization")]
    pub mirror: Mirror,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub launch_options: LaunchOptions,
    #[serde(default)]
    pub launcher_settings: LauncherSettings,
    #[serde(default)]
    pub download_settings: DownloadSettings,
    #[serde(default)]
    pub native_libraries: NativeLibraries,
}

impl Config {
    /// Persist the config to disk as JSON. Atomic write via tmp+rename.
    pub fn write_to_file(&self) -> Result<(), AppError> {
        let path = get_config_directory();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::JsonParseFailed(format!("config serialize: {e}")))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, &json).map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
        fs::rename(&tmp, &path).map_err(|e| AppError::FileRenameFailed(e.to_string()))?;
        Ok(())
    }
}
