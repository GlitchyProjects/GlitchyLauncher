//! Legacy INI-shaped config structure used ONLY for one-way migration
//! from the old `launcher-settings.ini` file to the new JSON config.
//!
//! This mirrors the original `Config` struct as it existed when the
//! launcher used `serde_ini`. We keep it here so the migration path in
//! `services/config.rs` can deserialize the old format without polluting
//! the new JSON-native `Config` with INI quirks (the `Bool` enum,
//! nested struct flattening, etc.).
//!
//! DO NOT add new fields here. New config fields go in `models/config.rs`.

use crate::models::mirror::Mirror;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LegacyNativeLibraries {
    pub use_custom_glfw: Bool,
    pub glfw_path: String,
    pub use_custom_openal: Bool,
    pub openal_path: String,
}

impl Default for LegacyNativeLibraries {
    fn default() -> Self {
        Self {
            use_custom_glfw: Bool::FALSE,
            glfw_path: String::new(),
            use_custom_openal: Bool::FALSE,
            openal_path: String::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LegacyLaunchOptions {
    pub selected_profile: Uuid,
    pub ram_usage_min: u64,
    pub ram_usage_max: u64,
    pub use_dedicated_gpu: Bool,
    #[serde(default)]
    pub java_override: Option<String>,
}

impl Default for LegacyLaunchOptions {
    fn default() -> Self {
        Self {
            selected_profile: crate::services::utils::uuid_from_username("Player"),
            ram_usage_min: 1024,
            ram_usage_max: 2048,
            use_dedicated_gpu: Bool::TRUE,
            java_override: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LegacyLauncherSettings {
    pub language: String,
    pub exit_on_launch: Bool,
}

mod mirror_serialization_legacy {
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
pub struct LegacyDownloadSettings {
    #[serde(with = "mirror_serialization_legacy")]
    pub mirror: Mirror,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LegacyConfig {
    #[serde(default)]
    pub launch_options: LegacyLaunchOptions,
    #[serde(default)]
    pub launcher_settings: LegacyLauncherSettings,
    #[serde(default)]
    pub download_settings: LegacyDownloadSettings,
    #[serde(default)]
    pub native_libraries: LegacyNativeLibraries,
}

/// Legacy Bool enum — INI can't represent real booleans reliably, so
/// the old config used `TRUE`/`FALSE` variant strings.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Bool {
    TRUE,
    #[allow(dead_code)]
    #[serde(other)]
    FALSE,
}

impl Default for Bool {
    fn default() -> Self {
        Bool::FALSE
    }
}

impl Bool {
    pub fn boolean(&self) -> bool {
        matches!(self, Bool::TRUE)
    }
}
