use std::path::Path;

use log::{info, warn};

use crate::models::downloader::Manifest;
use crate::models::error::AppError;
use crate::models::mirror::Mirror;
use crate::models::versions::MinecraftVersion;
use crate::services::directory_manager::{get_versions_directory, version_manifest_directory};
use crate::GLOBAL_CACHE;

/// Load the version manifest, downloading it first if necessary.
///
/// Falls back to the on-disk cache when the network is unreachable or
/// the server returns an invalid response — the launcher stays usable
/// offline and a broken download can never corrupt the cached manifest.
pub async fn load_version_manifest(mirror: &Mirror) -> Result<Manifest, AppError> {
    if let Err(e) = download_version_manifest(mirror).await {
        warn!("manifest download failed, falling back to cached copy: {e:?}");
    }
    load_version_manifest_local()
}

/// Load the manifest from the on-disk cache.
pub fn load_version_manifest_local() -> Result<Manifest, AppError> {
    let path = version_manifest_directory();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| AppError::FileReadFailed(format!("manifest: {e}")))?;
    serde_json::from_str(&text).map_err(|e| AppError::ManifestParseFailed(e.to_string()))
}

/// Scan the versions directory and rebuild the in-memory version list.
pub async fn reload_installed_versions() {
    let versions_dir = get_versions_directory();
    let Ok(read_dir) = std::fs::read_dir(&versions_dir) else {
        warn!(
            "versions directory unreadable at {}, starting with empty version list",
            versions_dir.display()
        );
        let mut global = GLOBAL_CACHE.lock().await;
        global.versions = Vec::new();
        return;
    };

    let mut versions = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match MinecraftVersion::from_folder(path) {
            Ok(v) => versions.push(v),
            Err(e) => warn!("skipping version directory: {e:?}"),
        }
    }

    let mut global = GLOBAL_CACHE.lock().await;
    global.versions = versions;
    info!("loaded {} installed versions", global.versions.len());
}

pub async fn initialize_versions() -> Result<(), AppError> {
    let manifest = load_version_manifest_local()?;
    let mut global = GLOBAL_CACHE.lock().await;
    for v in &manifest.versions {
        global
            .versions
            .push(MinecraftVersion::from_id(v.id.clone()));
    }
    Ok(())
}

/// Download the Mojang version manifest and cache it on disk.
///
/// This function is defensive: it validates the HTTP status code AND
/// the response body (must be valid JSON, must deserialize as a
/// `Manifest`, must contain at least one version entry) BEFORE writing
/// to the cache. A 403/404/500/HTML/empty/truncated response is rejected
/// and the existing cached manifest is preserved untouched.
pub async fn download_version_manifest(mirror: &Mirror) -> Result<(), AppError> {
    let url = mirror
        .parse_url(&"https://launchermeta.mojang.com/mc/game/version_manifest.json".to_string());

    // 1. HTTP request with explicit error mapping.
    let resp = crate::services::http::get(&url)
        .await
        .map_err(|e| AppError::NetworkRequestFailed(format!("manifest request: {e}")))?;

    // 2. Validate status code.
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::NetworkRequestFailed(format!(
            "manifest HTTP {} — the mirror may be down or rate-limiting",
            status
        )));
    }

    // 3. Validate content-type (reject HTML error pages).
    if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct_str = ct.to_str().unwrap_or("");
        if !ct_str.contains("json") && !ct_str.contains("text") {
            return Err(AppError::ManifestParseFailed(format!(
                "manifest content-type is '{ct_str}', expected JSON"
            )));
        }
    }

    // 4. Read body.
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::NetworkRequestFailed(format!("manifest body: {e}")))?;

    // 5. Reject empty / truncated responses.
    if bytes.is_empty() {
        return Err(AppError::ManifestParseFailed(
            "manifest response is empty".to_string(),
        ));
    }

    // 6. Deserialize as JSON → Manifest. This catches malformed JSON
    //    AND structurally-wrong responses (missing `versions` array, etc.).
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::ManifestParseFailed(format!("manifest JSON invalid: {e}")))?;

    // 7. Structural sanity check: must have at least one version.
    if manifest.versions.is_empty() {
        return Err(AppError::ManifestParseFailed(
            "manifest contains zero versions — likely a server error".to_string(),
        ));
    }

    // 8. Only NOW write to disk. Atomic tmp+rename.
    let path = version_manifest_directory();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
    replace_cached_manifest(&tmp, &path)?;
    info!("manifest cached: {} versions", manifest.versions.len());
    Ok(())
}

/// Replace the validated cache on Windows as well as Unix. A plain
/// `rename(tmp, destination)` cannot overwrite an existing file on
/// Windows, which previously made every manifest refresh after the first
/// successful download fail. Keep a backup until the new file is in place
/// so an interrupted replacement does not destroy the last good cache.
fn replace_cached_manifest(tmp: &Path, destination: &Path) -> Result<(), AppError> {
    if !destination.exists() {
        return std::fs::rename(tmp, destination)
            .map_err(|error| AppError::FileRenameFailed(error.to_string()));
    }

    let backup = destination.with_extension("json.backup");
    if backup.exists() {
        std::fs::remove_file(&backup)
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    std::fs::rename(destination, &backup)
        .map_err(|error| AppError::FileRenameFailed(error.to_string()))?;

    match std::fs::rename(tmp, destination) {
        Ok(()) => {
            let _ = std::fs::remove_file(backup);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::rename(&backup, destination);
            Err(AppError::FileRenameFailed(error.to_string()))
        }
    }
}

pub fn is_version_installed(id: &str) -> bool {
    let path = version_manifest_directory_for(id);
    Path::new(&path).exists()
}

fn version_manifest_directory_for(id: &str) -> std::path::PathBuf {
    get_versions_directory().join(id).join(format!("{id}.json"))
}
