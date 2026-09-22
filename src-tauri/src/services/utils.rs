use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use log::info;
use serde_json::{Map, Value};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha256};
use tauri::{AppHandle, Emitter};
use uuid::{Builder, Uuid};

use crate::models::downloader::{LibraryRules, Rule, RuleOS};
use crate::models::error::AppError;
use crate::models::java::Java;
use crate::models::platform::get_current_os;
use crate::services::directory_manager::get_libraries_directory;

/// Calculate the SHA-1 hex digest of a file on disk.
///
/// Used by the verify-and-repair flow when an upstream manifest only
/// provides a SHA-1 (which is the case for Minecraft assets and
/// libraries).
pub fn calculate_file_sha1<P: AsRef<Path>>(path: P) -> Result<String, AppError> {
    let file = File::open(path.as_ref())
        .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|e| AppError::BufferReadFailed(e.to_string()))?;
        if n == 0 {
            break;
        }
        Sha1Digest::update(&mut hasher, &buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Calculate the SHA-256 hex digest of a file on disk.
pub fn calculate_file_sha256<P: AsRef<Path>>(path: P) -> Result<String, AppError> {
    let file = File::open(path.as_ref())
        .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buffer)
            .map_err(|e| AppError::BufferReadFailed(e.to_string()))?;
        if n == 0 {
            break;
        }
        Sha2Digest::update(&mut hasher, &buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Returns true when a file exists at `path_str` AND its size matches
/// `expected_size` (when `expected_size > 0`). A zero `expected_size`
/// disables the size check (used when only a hash check is meaningful).
///
/// Note: this is a fast pre-check only. Final validation must go through
/// hash verification in [`crate::services::download_manager`].
pub fn verify_file_existence(path_str: &str, expected_size: u64) -> bool {
    let path = Path::new(path_str);
    if !path.exists() {
        return false;
    }
    if expected_size == 0 {
        return true;
    }
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    metadata.len() == expected_size
}

/// Fetch a JSON document from `url` and parse it as `Value`.
///
/// Errors are propagated instead of panicked. Callers that want a
/// best-effort fetch (e.g. cache warm-up) can `.ok()` the result.
pub async fn load_json_url(url: &str) -> Result<Value, AppError> {
    crate::services::http::get_json(url).await
}

/// Join a Vec<String> with a separator. Panics on empty input by design
/// (callers must guarantee at least one element); the alternative of
/// silently returning "" has historically masked bugs.
pub fn vec_to_string(vec: Vec<String>, separator: String) -> String {
    assert!(
        !vec.is_empty(),
        "vec_to_string called with empty input — caller bug"
    );
    let mut builder = String::new();
    for (i, s) in vec.iter().enumerate() {
        if i > 0 {
            builder.push_str(&separator);
        }
        builder.push_str(s);
    }
    builder
}

/// Convert a Maven coordinate (`group:artifact:version`) into a relative
/// repository path under the libraries directory.
pub fn parse_library_name_to_path(mavenized_path: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = mavenized_path.split(':').collect();
    if parts.len() != 3 {
        return Err(AppError::PathValidationFailed(format!(
            "invalid maven coordinate: {mavenized_path}"
        )));
    }
    let group = parts[0].replace('.', "/");
    let artifact_id = parts[1];
    let version = parts[2];
    let lib_dir = get_libraries_directory();
    let base = lib_dir
        .to_str()
        .ok_or_else(|| AppError::PathValidationFailed("libraries dir is not UTF-8".to_string()))?;
    Ok(format!(
        "{base}/{group}/{artifact_id}/{version}/{artifact_id}-{version}.jar"
    ))
}

/// Concatenate two Vecs, skipping entries already present in `vec1`.
pub fn extend_once<T: PartialEq>(mut vec1: Vec<T>, vec2: Vec<T>) -> Vec<T> {
    for item in vec2 {
        if !vec1.contains(&item) {
            vec1.push(item);
        }
    }
    vec1
}

pub fn convert_to_full_url(base_url: &str, library_name: &str) -> Result<String, AppError> {
    let args: Vec<&str> = library_name.split(':').collect();
    if args.len() != 3 {
        return Err(AppError::PathValidationFailed(format!(
            "invalid maven coordinate: {library_name}"
        )));
    }
    let group_id = args[0].replace('.', "/");
    let artifact_id = args[1];
    let version = args[2];
    let artifact_version = format!("{artifact_id}-{version}");
    Ok(format!(
        "{base_url}{group_id}/{artifact_id}/{version}/{artifact_version}.jar"
    ))
}

pub fn convert_to_full_path(base_path: &str, library_name: &str) -> Result<String, AppError> {
    let args: Vec<&str> = library_name.split(':').collect();
    if args.len() != 3 {
        return Err(AppError::PathValidationFailed(format!(
            "invalid maven coordinate: {library_name}"
        )));
    }
    let group_id = args[0].replace('.', "/");
    let artifact_id = args[1];
    let version = args[2];
    let artifact_version = format!("{artifact_id}-{version}");
    Ok(format!(
        "{base_path}/{group_id}/{artifact_id}/{version}/{artifact_version}.jar"
    ))
}

pub fn get_core_version(version_id: &str) -> Result<String, AppError> {
    let args: Vec<&str> = version_id.split('.').collect();
    if args.len() < 2 {
        return Err(AppError::UnknownError(format!(
            "invalid version id: {version_id}"
        )));
    }
    Ok(format!("{}.{}", args[0], args[1]))
}

pub fn can_apply_rule(rule_obj: &Map<String, Value>) -> bool {
    let rules = match rule_obj.get("rules").and_then(|r| r.as_array()) {
        Some(r) => r,
        None => return true,
    };

    let mut allowed = false;
    for rule in rules {
        if let Some(rule_map) = rule.as_object() {
            let action = rule_map
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("allow");
            let applies = check_os_rule(rule_map);
            if applies {
                match action {
                    "allow" => allowed = true,
                    "disallow" => return false,
                    _ => {}
                }
            }
        }
    }
    allowed
}

pub fn check_os_rule(rule_map: &Map<String, Value>) -> bool {
    let os_condition = match rule_map.get("os") {
        Some(os) => os.as_object(),
        None => return true,
    };
    let Some(os) = os_condition else {
        return true;
    };
    if let Some(name) = os.get("name").and_then(|n| n.as_str()) {
        let current_os = get_current_os();
        return current_os == name;
    }
    true
}

pub fn update_download_bar(progress: i64, app_handle: &AppHandle) {
    let _ = app_handle.emit("progressBar", progress);
}
pub fn update_download_status(text: &str, app_handle: &AppHandle) {
    let _ = app_handle.emit("progress", text);
}
pub fn update_download(progress: i64, text: &str, app_handle: &AppHandle) {
    let _ = app_handle.emit("progress", text);
    let _ = app_handle.emit("progressBar", progress);
}

/// Fix missing execute bit on the Java binary on Unix so we don't rely
/// on the user manually `chmod`-ing it. Returns an error instead of
/// panicking when the binary is missing — the caller (launcher) decides
/// whether that's fatal or recoverable.
pub fn linux_java_permission_fix(java: &Java) -> Result<(), AppError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin = java.get_bin_file();
        let metadata = std::fs::metadata(&bin).map_err(|e| {
            AppError::JavaExecutableInvalid(format!(
                "cannot stat {}: {e}",
                bin.display()
            ))
        })?;
        let mut permissions = metadata.permissions();
        let current_mode = permissions.mode();
        if current_mode & 0o111 == 0 {
            info!("Adding execute permission to Java binary: {}", bin.display());
            permissions.set_mode(current_mode | 0o111);
            std::fs::set_permissions(&bin, permissions).map_err(|e| {
                AppError::AccessDenied(format!(
                    "cannot set execute bit on {}: {e}",
                    bin.display()
                ))
            })?;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = java;
    }
    Ok(())
}

/// Deterministic offline UUID as per Minecraft's `OfflinePlayer:<name>`
/// convention (Java's `OfflinePlayer.getUniqueId()`). This produces a
/// version-3 (MD5-based) UUID identical to what a vanilla server would
/// assign to the same username.
pub fn uuid_from_username(username: &str) -> Uuid {
    let input = format!("OfflinePlayer:{username}");
    let hash = md5::compute(input.as_bytes());
    Builder::from_md5_bytes(*hash).into_uuid()
}

pub fn fetch_library_path(name: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = name.split('/').collect();
    if parts.len() != 3 {
        return Err(AppError::PathValidationFailed(format!(
            "invalid library path: {name}"
        )));
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    Ok(format!("{group}/{artifact}/{version}/{artifact}-{version}.jar"))
}

pub fn fetch_unofficial_library_repos(path: &str) -> Vec<String> {
    vec![
        format!("https://maven.minecraftforge.net/{path}"),
        format!("https://repo.spongepowered.org/maven/{path}"),
    ]
}

fn push_rule(rules: &[Rule], pushing_vec: &mut Vec<String>, rule_os: &Option<RuleOS>) {
    if rule_os.is_none() {
        pushing_vec.push("osx".to_string());
        pushing_vec.push("windows".to_string());
        pushing_vec.push("linux".to_string());
    } else if let Some(name) = rule_os.as_ref().and_then(|os| os.name.as_ref()) {
        pushing_vec.push(name.to_string());
    }
}

pub fn fetch_rules(value: Option<&Vec<Rule>>) -> LibraryRules {
    if value.is_none() {
        return LibraryRules {
            allowed_oses: vec![
                "osx".to_string(),
                "windows".to_string(),
                "linux".to_string(),
            ],
            disallowed_oses: vec![],
        };
    }
    let rules = value.unwrap();
    let mut allowed = vec![];
    let mut disallowed = vec![];
    for rule in rules {
        let rule_action = &rule.action;
        let rule_os = &rule.os;
        if rule_action == "allow" {
            push_rule(rules, &mut allowed, rule_os);
        } else if rule_action == "disallow" {
            push_rule(rules, &mut disallowed, rule_os);
        }
    }
    LibraryRules {
        allowed_oses: allowed,
        disallowed_oses: disallowed,
    }
}

/// Returns true if `version` is `1.x` with `x <= 12`. Used to decide
/// whether to use legacy `minecraftArguments` (space-separated) instead
/// of the modern `arguments.game` array.
pub fn is_legacy(version: &str) -> bool {
    if !version.starts_with("1.") {
        return false;
    }
    let mut parts = version.split('.');
    let _ = parts.next(); // "1"
    let Some(minor) = parts.next() else {
        return false;
    };
    let Ok(minor) = minor.parse::<u32>() else {
        return false;
    };
    minor <= 12
}

pub(crate) fn is_wayland() -> bool {
    let wayland_display = env::var("WAYLAND_DISPLAY").is_ok();
    let session_type = env::var("XDG_SESSION_TYPE")
        .map(|v| v.to_lowercase() == "wayland")
        .unwrap_or(false);
    wayland_display || session_type
}

/// Set environment variables that hint the OS to use the discrete GPU.
///
/// On Linux this targets PRIME/NVIDIA Optimus setups. On systems
/// without a discrete GPU the variables are harmless — the driver
/// simply ignores them. We always set them rather than trying to detect
/// hardware, because detection is unreliable across drivers and the
/// downside of setting them on integrated-only systems is zero.
pub fn apply_dedicated_gpu_env(cmd: &mut std::process::Command) {
    #[cfg(target_os = "linux")]
    {
        cmd.env("DRI_PRIME", "1");
        cmd.env("__NV_PRIME_RENDER_OFFLOAD", "1");
        cmd.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia");
        cmd.env("__VK_LAYER_NV_optimus", "NVIDIA_only");
    }
    #[cfg(windows)]
    {
        // On Windows, forcing Linux/X11 variables (__GLX_VENDOR_LIBRARY_NAME, __VK_LAYER_NV_optimus)
        // causes driver TDR (Timeout Detection and Recovery) and black screen/system restart
        // on dual-GPU laptops (Intel/AMD + Nvidia). We use safe Windows compatibility hints only.
        cmd.env("SHIM_MCCOMPAT", "0x800000001");
    }
}

/// Make a `PathBuf` from a path string, normalizing separators to the
/// platform's native form. Used when materializing library paths read
/// from manifests (which always use `/`).
pub fn normalize_separators(path: &str) -> PathBuf {
    let pb = PathBuf::from(path.replace('\\', std::path::MAIN_SEPARATOR_STR));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vanilla Minecraft server derives the offline UUID from
    /// `OfflinePlayer:<username>` via MD5 (version 3 UUID). We test
    /// against the known value for "Player" so a regression here would
    /// immediately break saves.
    #[test]
    fn offline_uuid_matches_minecraft_convention() {
        let uuid = uuid_from_username("Player");
        assert_eq!(uuid.get_version(), Some(uuid::Version::Md5));
        // Known offline UUID for "Player" — matches what vanilla
        // Minecraft servers assign. Pinned so any change to the
        // derivation breaks the test.
        assert_eq!(
            uuid.to_string(),
            "a01e3843-e521-3998-958a-f459800e4d11"
        );
    }

    #[test]
    fn offline_uuid_is_deterministic() {
        let a = uuid_from_username("Glitchy");
        let b = uuid_from_username("Glitchy");
        assert_eq!(a, b);
    }

    #[test]
    fn offline_uuid_differs_for_different_usernames() {
        let a = uuid_from_username("Alice");
        let b = uuid_from_username("Bob");
        assert_ne!(a, b);
    }

    #[test]
    fn is_legacy_recognizes_old_versions() {
        assert!(is_legacy("1.7.10"));
        assert!(is_legacy("1.12.2"));
        assert!(!is_legacy("1.13"));
        assert!(!is_legacy("1.21"));
        assert!(!is_legacy("2.0"));
        assert!(!is_legacy("snap"));
    }

    #[test]
    fn vec_to_string_joins_with_separator() {
        let v = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(vec_to_string(v, ":".to_string()), "a:b:c");
    }

    #[test]
    #[should_panic(expected = "vec_to_string called with empty input")]
    fn vec_to_string_panics_on_empty() {
        let _: String = vec_to_string(vec![], ",".to_string());
    }

    #[test]
    fn parse_library_name_to_path_builds_maven_layout() {
        let p = parse_library_name_to_path("com.mojang:authlib:1.5.25").unwrap();
        assert!(p.contains("com/mojang/authlib/1.5.25/authlib-1.5.25.jar"));
    }

    #[test]
    fn parse_library_name_to_path_rejects_invalid_coordinates() {
        assert!(parse_library_name_to_path("invalid").is_err());
        assert!(parse_library_name_to_path("a:b").is_err());
        assert!(parse_library_name_to_path("a:b:c:d").is_err());
    }

    #[test]
    fn extend_once_deduplicates() {
        let a = vec![1, 2, 3];
        let b = vec![3, 4, 5];
        assert_eq!(extend_once(a, b), vec![1, 2, 3, 4, 5]);
    }
}
