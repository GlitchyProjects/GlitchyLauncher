use std::fs;
use std::fs::create_dir_all;
use std::path::Path;
use std::sync::Arc;

use log::{info, warn};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::models::error::AppError;
use crate::models::java::Java;
use crate::models::logger::LogLine;
use crate::models::mirror::Mirror;
use crate::models::platform::get_current_os_with_architecture;
use crate::services::directory_manager::get_java_dir;
use crate::services::download_manager::{self, DownloadJob, DownloadToken, VerifySpec};
use crate::services::download_session::SessionHandle;
use crate::services::game_downloader::download_file_verified_for_session;
use crate::services::utils::load_json_url;

/// Return a `Java` handle for the runtime identified by `component`
/// (e.g. `jre-legacy`, `java-runtime-gamma`). The runtime must already
/// be installed; this function does NOT trigger a download.
pub fn get_java(java: String) -> Result<Java, AppError> {
    let runtime_dir = get_java_dir().join(&java);
    if !runtime_dir.exists() {
        return Err(AppError::JavaNotFound(format!(
            "runtime '{}' is not installed (expected at {})",
            java,
            runtime_dir.display()
        )));
    }
    Ok(Java::new(runtime_dir))
}

/// Download a Java runtime from Mojang's runtime manifest.
///
/// `java` is the component name (e.g. `jre-legacy`); `version` is the
/// major version string used to pick the right runtime when multiple
/// are available for the same component.
///
/// Errors are propagated; the caller (typically the version download
/// flow) decides whether to abort or fall back to a system Java.
pub async fn download_java(
    java: &str,
    version: &str,
    logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    session: Option<&SessionHandle>,
) -> Result<(), AppError> {
    let _ = logger; // currently unused; reserved for future progress events.
    let runtime_dir = get_java_dir().join(java);

    let url = mirror.parse_url(
        &"https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json".to_string(),
    );
    let current_os = get_current_os_with_architecture();
    let json: Value = load_json_url(&url).await?;
    let runtime_arr = &json[&current_os][java];
    let runtime_v = runtime_arr
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|x| {
                x["version"]["name"]
                    .as_str()
                    .map(|n| n.to_lowercase().starts_with(version))
                    .unwrap_or(false)
            })
        })
        .or_else(|| runtime_arr.as_array().and_then(|arr| arr.first()))
        .ok_or_else(|| {
            AppError::JavaNotFound(format!(
                "no Java runtime entry for os={current_os}, component={java}"
            ))
        })?;

    let manifest_url = runtime_v["manifest"]["url"]
        .as_str()
        .ok_or_else(|| AppError::JavaNotFound("runtime manifest URL missing".to_string()))?;
    let manifest_url = mirror.parse_url(&manifest_url.to_string());
    let runtime_manifest: Value = load_json_url(&manifest_url).await?;

    let Some(files) = runtime_manifest["files"].as_object() else {
        return Err(AppError::JsonParseFailed(
            "runtime manifest has no 'files' object".to_string(),
        ));
    };

    if let Some(s) = session {
        s.set_phase("java");
    }

    // Directory entries first (cheap), then the actual files with
    // per-file progress reporting.
    for (k, v) in files
        .iter()
        .filter(|(_, v)| v["type"].as_str() != Some("file"))
    {
        let dir = runtime_dir.join(k);
        create_dir_all(&dir)
            .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", dir.display())))?;
    }

    let file_entries: Vec<(&String, &Value)> = files
        .iter()
        .filter(|(_, v)| v["type"].as_str() == Some("file"))
        .collect();
    let mut jobs: Vec<DownloadJob> = Vec::with_capacity(file_entries.len());
    for (k, v) in file_entries {
        let download_raw = &v["downloads"]["raw"];
        let url = download_raw["url"]
            .as_str()
            .ok_or_else(|| AppError::JsonParseFailed("file entry missing url".to_string()))?;
        let url = mirror.parse_url(&url.to_string());
        let size = download_raw["size"].as_u64().unwrap_or(0);
        let sha1 = download_raw["sha1"].as_str().map(|s| s.to_string());
        let dest = runtime_dir.join(k);
        if let Some(parent) = dest.parent() {
            create_dir_all(parent).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
        }
        jobs.push(DownloadJob {
            url,
            destination: dest.to_string_lossy().to_string(),
            expected_size: Some(size),
            verify: sha1.map(VerifySpec::sha1),
            category: "java".to_string(),
        });
    }

    let progress_cb = session.map(|s| {
        let s = s.clone();
        Arc::new(move |done: u32, total: u32, file: String| {
            s.progress(done as f64 / total.max(1) as f64, &file);
        }) as Arc<dyn Fn(u32, u32, String) + Send + Sync>
    });
    let token = DownloadToken::new();
    download_manager::global()
        .download_batch_with_progress(jobs, None, &token, progress_cb)
        .await?;

    // Write a `release` file so subsequent `Java::new()` reads work
    // even when Mojang's runtime didn't ship one.
    let release_path = runtime_dir.join("release");
    if !release_path.exists() {
        fs::write(&release_path, format!("JAVA_VERSION=\"{version}\"\n"))
            .map_err(|e| AppError::FileCreateFailed(format!("{}: {e}", release_path.display())))?;
    }

    // Set executable bit on the Java binary on Unix so the launcher
    // doesn't fail at spawn time.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin = runtime_dir.join("bin").join("java");
        if bin.exists() {
            if let Ok(meta) = fs::metadata(&bin) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o111);
                let _ = fs::set_permissions(&bin, perms);
            }
        }
    }

    info!(
        "Java runtime {} installed at {}",
        java,
        runtime_dir.display()
    );
    let _ = Path::new(""); // suppress unused Path import warning on non-unix
    Ok(())
}

#[allow(dead_code)]
fn _unused_warn(_e: &str) {
    warn!("");
}
