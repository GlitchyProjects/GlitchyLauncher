use std::env::{home_dir, var_os};
use std::path::PathBuf;

use log::warn;
use tokio::fs::create_dir_all;

use crate::models::error::AppError;
use crate::models::mirror::mojang_mirror;
use crate::services::path_safety::sanitize_user_path_segment;

/// Return the OS-specific `.minecraft` directory.
///
/// Falls back to `~/.minecraft` when the standard env var is missing
/// (which can happen in some sandboxed environments). Never panics.
pub fn get_minecraft_directory() -> PathBuf {
    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            return home.join("Library/Application Support/minecraft");
        }
    } else if cfg!(target_os = "windows") {
        if let Some(appdata) = var_os("APPDATA") {
            return PathBuf::from(appdata).join(".minecraft");
        }
    } else if let Some(home) = home_dir() {
        return home.join(".minecraft");
    }
    // Last-resort fallback: current dir. The launcher will still try
    // to operate, but file paths may be relative.
    warn!("could not locate .minecraft directory; falling back to current dir");
    PathBuf::from(".minecraft")
}

pub fn get_libraries_directory() -> PathBuf {
    get_minecraft_directory().join("libraries")
}

pub fn get_versions_directory() -> PathBuf {
    get_minecraft_directory().join("versions")
}

pub fn get_version_directory(version: &str) -> PathBuf {
    get_versions_directory().join(version)
}

pub fn get_natives_folder(version: &str) -> PathBuf {
    get_version_directory(version).join("natives")
}

pub fn get_assets_directory() -> PathBuf {
    get_minecraft_directory().join("assets")
}

pub fn get_falcon_launcher_directory() -> PathBuf {
    get_minecraft_directory().join("falconlauncher")
}

pub fn get_launcher_java_directory() -> PathBuf {
    get_falcon_launcher_directory().join("java")
}

/// Root of the per-version instance folders. Every installed version
/// gets its own directory below this one so mods, saves and config
/// never mix between versions.
pub fn get_instances_directory() -> PathBuf {
    get_minecraft_directory().join("instances")
}

/// The isolated game directory of one installed version:
/// `<minecraft>/instances/<version_id>/`. Used as the process working
/// directory and `--gameDir` when launching, so the game writes its
/// `mods/`, `saves/` etc. inside the instance folder.
pub fn get_instance_directory(version_id: &str) -> Result<PathBuf, AppError> {
    let default = sanitize_user_path_segment(&get_instances_directory(), version_id)?;
    Ok(crate::services::instance_manager::configured_instance_path(version_id)
        .unwrap_or(default))
}

/// A category subfolder of one instance (mods / resourcepacks /
/// shaderpacks). The game runs with the instance dir as `--gameDir`,
/// so the game picks these folders up automatically and content never
/// leaks between versions.
pub fn get_category_folder(
    version_id: &str,
    folder_name: &str,
) -> Result<PathBuf, AppError> {
    // `folder_name` comes from our own enum, but validate anyway so the
    // helper stays safe for any future caller.
    if folder_name.is_empty()
        || !folder_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(AppError::PathValidationFailed(format!(
            "invalid category folder name: {folder_name:?}"
        )));
    }
    Ok(get_instance_directory(version_id)?.join(folder_name))
}

/// Create the instance directory and its content subfolders (mods,
/// resourcepacks, shaderpacks) if missing. Called when a version finishes
/// installing, when the backpack page is opened, and before launch so the
/// game always finds its folders.
pub async fn ensure_instance_dirs(version_id: &str) -> Result<(), AppError> {
    for folder in ["mods", "resourcepacks", "shaderpacks", "saves", "backups"] {
        let dir = get_category_folder(version_id, folder)?;
        create_dir_all(&dir)
            .await
            .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", dir.display())))?;
    }
    crate::services::instance_manager::write_instance_marker(
        &get_instance_directory(version_id)?,
        version_id,
    )?;
    if let Ok(inst_dir) = get_instance_directory(version_id) {
        crate::services::game_launcher::ensure_instance_options(&inst_dir);
    }
    Ok(())
}

pub fn get_profiles_file() -> PathBuf {
    get_falcon_launcher_directory().join("profiles.json")
}

pub fn get_temp_directory() -> PathBuf {
    get_falcon_launcher_directory().join("temp")
}

/// Create all directories the launcher needs at startup. Best-effort:
/// individual failures are logged but don't abort startup so a
/// permission issue on one dir doesn't prevent the launcher from
/// opening (the user can still see an error in Settings).
pub async fn create_necessary_dirs() {
    let dirs = [
        get_versions_directory(),
        get_instances_directory(),
        get_falcon_launcher_directory(),
        get_assets_directory(),
        get_launcher_java_directory(),
        get_mirrors_dir(),
    ];
    for d in dirs {
        if let Err(e) = create_dir_all(&d).await {
            warn!("failed to create {}: {}", d.display(), e);
        }
    }
    let _ = mojang_mirror().write();
}

pub fn version_manifest_directory() -> PathBuf {
    get_versions_directory().join("version_manifest_v2.json")
}

/// Path of the JSON config file. The launcher migrated from INI to JSON
/// because `serde_ini` cannot round-trip the nested Config structure.
/// See `services/config.rs::load()` for the migration path.
pub fn get_config_directory() -> PathBuf {
    get_falcon_launcher_directory().join("launcher-settings.json")
}

/// Validate that `path` looks like a JRE root (has `bin/java` or
/// `bin/javaw.exe`). Used by Java auto-detection.
fn validate_java(path: &PathBuf) -> bool {
    let java_file = if cfg!(target_os = "windows") {
        "javaw.exe"
    } else {
        "java"
    };
    path.join("bin").join(java_file).exists()
}

/// Auto-detect Java installations in well-known system locations.
///
/// Returns an empty Vec when none are found — never panics. The
/// `commands::java::list_detected_javas` command uses this to populate
/// the manual Java picker in Settings.
pub fn auto_detect_javas() -> Result<Vec<crate::models::java::Java>, AppError> {
    let mut paths = Vec::new();
    let dirs: Vec<PathBuf> = if cfg!(target_os = "windows") {
        vec![
            PathBuf::from(r"C:\Program Files\Java"),
            PathBuf::from(r"C:\Program Files (x86)\Java"),
        ]
    } else if cfg!(target_os = "linux") {
        vec![
            PathBuf::from("/usr/lib/jvm"),
            PathBuf::from("/usr/java"),
            PathBuf::from("/usr/local/java"),
        ]
    } else {
        vec![PathBuf::from("/Library/Java/JavaVirtualMachines")]
    };

    for path in dirs {
        let Ok(read_dir) = std::fs::read_dir(&path) else {
            continue;
        };
        for entry in read_dir.flatten() {
            let p = entry.path();
            if validate_java(&p) {
                paths.push(crate::models::java::Java::new(p));
            }
        }
    }
    Ok(paths)
}

pub fn get_java_dir() -> PathBuf {
    get_minecraft_directory().join("runtime")
}

pub fn get_mirrors_dir() -> PathBuf {
    get_falcon_launcher_directory().join("mirrors")
}

pub fn get_version_manifest(id: &str) -> PathBuf {
    get_version_directory(id).join(format!("{id}.json"))
}
