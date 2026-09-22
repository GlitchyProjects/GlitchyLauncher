use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Mutex;

use log::info;
use toml::Value;
use zip::ZipArchive;

use crate::models::error::AppError;
use crate::models::mods::{BackpackCategory, FabricModInfo, McModInfo, ModInfo};
use crate::services::path_safety::sanitize_archive_entry;

/// Enable/disable a backpack item by renaming its file.
///
/// Mods:   `foo.jar` <-> `foo.disabled`
/// Packs:  `foo.zip` <-> `foo.zip.disabled`
///
/// `toggle = true` means "enable".
pub fn set_item_enabled(
    path: &Path,
    toggle: bool,
    category: BackpackCategory,
) -> Result<(), AppError> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| AppError::PathValidationFailed(format!("no file name: {}", path.display())))?;

    let (enabled_name, disabled_name) = match category {
        BackpackCategory::Mods => {
            // Accept `foo.jar`, `foo.disabled` and the redundant
            // `foo.jar.disabled` — normalize to the stem first.
            let base = file_name.strip_suffix(".disabled").unwrap_or(file_name);
            match base.strip_suffix(".jar") {
                Some(stem) => (format!("{stem}.jar"), format!("{stem}.disabled")),
                None if file_name.ends_with(".disabled") => {
                    (format!("{base}.jar"), format!("{base}.disabled"))
                }
                None => {
                    return Err(AppError::PathValidationFailed(format!(
                        "not a mod file: {file_name}"
                    )))
                }
            }
        }
        BackpackCategory::ResourcePacks | BackpackCategory::ShaderPacks => {
            let stem = file_name
                .strip_suffix(".zip.disabled")
                .or_else(|| file_name.strip_suffix(".zip"));
            match stem {
                Some(s) => (format!("{s}.zip"), format!("{s}.zip.disabled")),
                None => {
                    return Err(AppError::PathValidationFailed(format!(
                        "not a pack file: {file_name}"
                    )))
                }
            }
        }
    };

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let new_path = parent.join(if toggle { enabled_name } else { disabled_name });
    std::fs::rename(path, &new_path)
        .map_err(|e| AppError::FileRenameFailed(format!("{}: {e}", path.display())))
}

/// Parse a mod's metadata from its JAR. Supports Forge legacy
/// (`mcmod.info`), Forge modern (`META-INF/mods.toml`), NeoForge
/// (`META-INF/neoforge.mods.toml`) and Fabric (`fabric.mod.json`).
///
/// Returns `AppError::ModLoadingFailed` when no recognized metadata
/// file is present. The caller (`commands::mods::get_mods`) skips
/// such mods rather than aborting the listing.
pub fn load_mod(zip: Mutex<ZipArchive<File>>, path: String) -> Result<ModInfo, AppError> {
    let enabled = path.to_lowercase().ends_with("jar");
    let mut zip_guard = zip.lock().map_err(|e| {
        AppError::ModLoadingFailed(format!("zip mutex poisoned: {e}"))
    })?;

    info!("Loading mod: {}", path);

    // Forge legacy
    if let Ok(mut entry) = zip_guard.by_name("mcmod.info") {
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_err() {
            return Err(AppError::ModLoadingFailed(
                "mcmod.info unreadable".to_string(),
            ));
        }
        let Ok(mcmods): Result<Vec<McModInfo>, _> = serde_json::from_str(&content) else {
            return Err(AppError::ModLoadingFailed("mcmod.info invalid JSON".to_string()));
        };
        let Some(mcmod_info) = mcmods.first() else {
            return Err(AppError::ModLoadingFailed("mcmod.info empty array".to_string()));
        };
        return Ok(ModInfo {
            path,
            mod_id: mcmod_info.mod_id.clone(),
            name: mcmod_info.name.clone(),
            version: mcmod_info.version.clone(),
            description: mcmod_info.description.clone(),
            enabled,
        });
    }

    // Forge modern
    if let Ok(mut entry) = zip_guard.by_name("META-INF/mods.toml") {
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_ok() {
            if let Ok(toml) = toml::from_str::<Value>(&content) {
                return Ok(load_from_toml(&toml, path, enabled));
            }
        }
    }

    // NeoForge
    if let Ok(mut entry) = zip_guard.by_name("META-INF/neoforge.mods.toml") {
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_ok() {
            if let Ok(toml) = toml::from_str::<Value>(&content) {
                return Ok(load_from_toml(&toml, path, enabled));
            }
        }
    }

    // Fabric
    if let Ok(mut entry) = zip_guard.by_name("fabric.mod.json") {
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_ok() {
            if let Ok(info) = serde_json::from_str::<FabricModInfo>(&content) {
                return Ok(ModInfo {
                    path,
                    mod_id: info.mod_id.clone(),
                    name: info.name.clone(),
                    version: info.version.clone(),
                    description: info.description.clone(),
                    enabled,
                });
            }
        }
    }

    Err(AppError::ModLoadingFailed(format!(
        "no recognized mod metadata file in {path}"
    )))
}

fn load_from_toml(toml: &Value, path: String, enabled: bool) -> ModInfo {
    let mut mod_id = String::new();
    let mut display_name = String::new();
    let mut version = String::new();
    let mut desc = String::new();
    if let Some(arr) = toml["mods"].as_array() {
        for entry in arr {
            if let Some(id) = entry.get("modId").and_then(|x| x.as_str()) {
                mod_id = id.to_string();
            }
            if let Some(description) = entry.get("description").and_then(|x| x.as_str()) {
                desc = description.to_string();
            }
            if let Some(ver) = entry.get("version").and_then(|x| x.as_str()) {
                version = ver.to_string();
            }
            if let Some(name) = entry.get("displayName").and_then(|x| x.as_str()) {
                display_name = name.to_string();
            }
        }
    }
    let mut info = ModInfo::new(path, mod_id, display_name, version, desc);
    info.enabled = enabled;
    info
}

#[derive(serde::Deserialize)]
struct PackMcMeta {
    #[serde(default)]
    pack: PackMeta,
}

#[derive(serde::Deserialize, Default)]
struct PackMeta {
    // The description may also be a JSON text component; treat a
    // non-string value as absent.
    #[serde(default, deserialize_with = "de_string_or_none")]
    description: Option<String>,
}

fn de_string_or_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;
    let v: Option<serde_json::Value> = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(v.and_then(|x| x.as_str().map(String::from)))
}

/// Build a [`ModInfo`] for a resource/shader pack zip. Unlike mods,
/// packs have no mandatory metadata — the display name is the file name
/// and the description is best-effort from `pack.mcmeta` when present.
pub fn load_pack(
    zip: &mut ZipArchive<File>,
    path: &str,
    enabled: bool,
) -> ModInfo {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = file_name
        .strip_suffix(".zip.disabled")
        .or_else(|| file_name.strip_suffix(".zip"))
        .unwrap_or(file_name);

    let mut description = String::new();
    if let Ok(mut entry) = zip.by_name("pack.mcmeta") {
        let mut content = String::new();
        if entry.read_to_string(&mut content).is_ok() {
            if let Ok(meta) = serde_json::from_str::<PackMcMeta>(&content) {
                if let Some(d) = meta.pack.description {
                    description = d;
                }
            }
        }
    }

    ModInfo {
        path: path.to_string(),
        mod_id: String::new(),
        name: stem.to_string(),
        version: String::new(),
        description,
        enabled,
    }
}

/// Safely extract a zip archive to `dest`, refusing entries that would
/// escape the destination directory (Zip Slip protection).
///
/// This replaces the previous direct `zip_extract::extract` calls,
/// which had no path traversal defense.
pub fn safe_extract_zip(zip_path: &Path, dest: &Path) -> Result<(), AppError> {
    let file = File::open(zip_path)
        .map_err(|e| AppError::FileReadFailed(format!("{}: {e}", zip_path.display())))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| AppError::ZipParseFailed(format!("{}: {e}", zip_path.display())))?;

    std::fs::create_dir_all(dest)
        .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", dest.display())))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::ZipParseFailed(format!("entry {i}: {e}")))?;
        let name = entry.name().to_string();
        let out = sanitize_archive_entry(dest, &name)?;

        if entry.is_dir() {
            std::fs::create_dir_all(&out)
                .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", out.display())))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                AppError::DirCreateFailed(format!("{}: {e}", parent.display()))
            })?;
        }
        let mut outfile = File::create(&out)
            .map_err(|e| AppError::FileCreateFailed(format!("{}: {e}", out.display())))?;
        std::io::copy(&mut entry, &mut outfile)
            .map_err(|e| AppError::FileWriteFailed(format!("{}: {e}", out.display())))?;

        // On Unix, restore the file mode from the zip entry if present.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}
