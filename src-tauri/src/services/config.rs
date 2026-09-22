use std::fs;
use std::path::PathBuf;

use log::{info, warn};

use crate::models::config::Config;
use crate::services::directory_manager::get_config_directory;

/// Load config from disk. The launcher migrated from INI to JSON because
/// `serde_ini` cannot round-trip the nested `Config` structure (nested
/// structs + custom `Mirror` serialization + `Bool` enum) reliably —
/// writing the config after creating an offline profile produced
/// "Invalid INI" errors and the profile was never persisted.
///
/// This loader is rollback-safe:
///   1. If a JSON config exists → load it directly.
///   2. If only an INI config exists → parse it, write a JSON copy,
///      rename the INI to `.ini.bak` (never delete).
///   3. If neither exists → write a default JSON config.
///   4. If the JSON config is corrupt → back it up, use defaults.
///
/// Offline accounts (profiles.json) are stored separately and are NOT
/// touched by this migration — they survive intact.
pub fn load() -> Config {
    let json_path = get_config_directory();
    let ini_path = ini_path();

    // 1. JSON config exists → load it.
    if json_path.exists() {
        return load_json_or_default(&json_path);
    }

    // 2. INI config exists → migrate.
    if ini_path.exists() {
        warn!("INI config found at {}, migrating to JSON", ini_path.display());
        return migrate_ini_to_json(&ini_path, &json_path);
    }

    // 3. Neither exists → write default JSON.
    info!("no config found, writing default JSON config");
    let default = Config::default();
    let _ = default.write_to_file();
    default
}

fn load_json_or_default(path: &std::path::Path) -> Config {
    let Ok(text) = fs::read_to_string(path) else {
        warn!("config unreadable at {}, using defaults", path.display());
        return Config::default();
    };
    match serde_json::from_str::<Config>(&text) {
        Ok(c) => c,
        Err(e) => {
            warn!("config parse failed at {} ({e}), using defaults — backing up", path.display());
            let backup = path.with_extension("json.bak");
            let _ = fs::rename(path, &backup);
            let default = Config::default();
            let _ = default.write_to_file();
            default
        }
    }
}

fn migrate_ini_to_json(ini_path: &std::path::Path, json_path: &std::path::Path) -> Config {
    let text = match fs::read_to_string(ini_path) {
        Ok(t) => t,
        Err(e) => {
            warn!("failed to read INI config for migration: {e}; using defaults");
            let default = Config::default();
            let _ = default.write_to_file();
            return default;
        }
    };

    // serde_ini uses a different intermediate representation than serde_json,
    // so we deserialize into the legacy INI-shaped Config and then re-serialize
    // as JSON. We use serde_ini directly here.
    let parsed: Result<crate::models::config_legacy::LegacyConfig, _> =
        serde_ini::from_str(&text);
    let config = match parsed {
        Ok(legacy) => {
            // Convert legacy → new Config. Most fields map 1:1.
            let mut c = Config::default();
            c.launch_options.selected_profile = legacy.launch_options.selected_profile;
            c.launch_options.ram_usage_min = legacy.launch_options.ram_usage_min;
            c.launch_options.ram_usage_max = legacy.launch_options.ram_usage_max;
            c.launch_options.use_dedicated_gpu = legacy.launch_options.use_dedicated_gpu.boolean();
            c.launch_options.java_override = legacy.launch_options.java_override;
            c.launcher_settings.language = legacy.launcher_settings.language;
            c.launcher_settings.exit_on_launch = legacy.launcher_settings.exit_on_launch.boolean();
            c.download_settings.mirror = legacy.download_settings.mirror;
            c.native_libraries.use_custom_glfw = legacy.native_libraries.use_custom_glfw.boolean();
            c.native_libraries.glfw_path = legacy.native_libraries.glfw_path;
            c.native_libraries.use_custom_openal = legacy.native_libraries.use_custom_openal.boolean();
            c.native_libraries.openal_path = legacy.native_libraries.openal_path;
            info!("INI config migrated successfully");
            c
        }
        Err(e) => {
            warn!("INI config parse failed during migration: {e}; using defaults");
            Config::default()
        }
    };

    // Write the JSON config.
    if let Err(e) = config.write_to_file() {
        warn!("failed to write migrated JSON config: {e:?}");
    }

    // Rename the INI file to .bak — never delete it.
    let backup = ini_path.with_extension("ini.bak");
    let _ = fs::rename(ini_path, &backup);
    info!("INI config backed up to {}", backup.display());

    config
}

/// Path of the legacy INI config file.
fn ini_path() -> PathBuf {
    get_falcon_launcher_directory_for_ini().join("launcher-settings.ini")
}

fn get_falcon_launcher_directory_for_ini() -> PathBuf {
    crate::services::directory_manager::get_falcon_launcher_directory()
}

/// Re-apply `cfg` fields from a freshly loaded `Config` (used when
/// reloading after an external edit).
pub fn load_config(cfg: &mut Config) {
    let fresh = load();
    cfg.launch_options = fresh.launch_options;
    cfg.launcher_settings = fresh.launcher_settings;
    cfg.download_settings = fresh.download_settings;
    cfg.native_libraries = fresh.native_libraries;
}
