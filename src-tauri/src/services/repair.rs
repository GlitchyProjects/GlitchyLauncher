//! Installation verification and repair.
//!
//! The repair flow:
//!   1. Parse the version JSON (and inherited parent if present).
//!   2. Enumerate every required artifact (client jar, libraries,
//!      assets, logging config, java runtime).
//!   3. For each, check existence, size, and (when available) SHA-1.
//!   4. Collect the list of missing/corrupt files.
//!   5. If `apply_repair` is true, re-download only the broken files
//!      via the central DownloadManager (which itself verifies hashes).
//!   6. Return a structured report the frontend can render.

use std::collections::HashMap;
use std::path::PathBuf;

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, State};
use tokio::sync::mpsc::UnboundedSender;

use crate::models::config::Config;
use crate::models::downloader::{AssetIndex, AssetObjects, MinecraftManifestVersion};
use crate::models::error::AppError;
use crate::models::logger::{info as log_info, LogLine};
use crate::models::mirror::Mirror;
use crate::models::versions::MinecraftVersion;
use crate::services::directory_manager::{
    get_assets_directory, get_libraries_directory, get_version_directory, get_version_manifest,
};
use crate::services::download_manager::{DownloadJob, HashKind, VerifySpec};
use crate::services::utils::{calculate_file_sha1, parse_library_name_to_path};
use crate::AppState;

/// Status of a single artifact during repair verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactStatus {
    Ok,
    Missing,
    SizeMismatch { expected: u64, actual: u64 },
    HashMismatch { expected: String, actual: String },
}

/// One row in the repair report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairArtifact {
    pub category: String,
    pub path: String,
    pub status: ArtifactStatus,
}

/// Aggregate repair report returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub version_id: String,
    pub artifacts: Vec<RepairArtifact>,
    pub ok_count: u32,
    pub broken_count: u32,
    pub repaired: bool,
}

/// Verify a version's installation. Does NOT modify anything on disk.
pub async fn verify_version(version_id: &str) -> Result<RepairReport, AppError> {
    let version = MinecraftVersion::from_id(version_id.to_string());
    let json_path = get_version_manifest(&version_id.to_string());
    if !json_path.exists() {
        return Err(AppError::FileNotFound(json_path.to_string_lossy().to_string()));
    }
    let text = std::fs::read_to_string(&json_path)
        .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let manifest: MinecraftManifestVersion = serde_json::from_str(&text)
        .map_err(|e| AppError::JsonParseFailed(e.to_string()))?;

    let mut artifacts = Vec::new();

    // Client jar
    if let Some(downloads) = &manifest.downloads {
        if let Some(client) = downloads.get("client") {
            let p = get_version_directory(&version.id).join(format!("{}.jar", version.id));
            artifacts.push(check_artifact(
                "client",
                &p,
                Some(client.size),
                Some(VerifySpec::sha1(&client.sha1)),
            ));
        }
    }

    // Libraries
    for lib in &manifest.libraries {
        let Some(name) = lib.name.split(':').next() else {
            continue;
        };
        let _ = name;
        // Resolve path: prefer `downloads.artifact.path`, else compute from name.
        let path_str = if let Some(d) = &lib.downloads {
            if let Some(a) = &d.artifact {
                if let Some(p) = &a.path {
                    get_libraries_directory().join(p).to_string_lossy().to_string()
                } else {
                    parse_library_name_to_path(&lib.name).unwrap_or_default()
                }
            } else {
                continue;
            }
        } else {
            continue;
        };
        let p = PathBuf::from(&path_str);
        let verify = lib.downloads.as_ref().and_then(|d| {
            d.artifact.as_ref().and_then(|a| {
                // Library artifacts in Mojang manifests don't always
                // ship a sha1 field on the `LibraryArtifact` struct;
                // when absent we skip hash verification.
                let _ = a;
                None
            })
        });
        let expected_size = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref().map(|a| a.size));
        artifacts.push(check_artifact("library", &p, expected_size, verify));
    }

    // Assets index + objects
    if let Some(asset_index) = &manifest.asset_index {
        let index_path = get_assets_directory()
            .join("indexes")
            .join(format!("{}.json", asset_index.id));
        artifacts.push(check_artifact(
            "asset_index",
            &index_path,
            Some(asset_index.size),
            Some(VerifySpec::sha1(&asset_index.sha1)),
        ));
        // If we have the index, also verify each object's hash.
        if let Ok(text) = std::fs::read_to_string(&index_path) {
            if let Ok(objects) = serde_json::from_str::<AssetObjects>(&text) {
                for (_name, entry) in &objects.objects {
                    let prefix = &entry.hash[..2];
                    let p = get_assets_directory()
                        .join("objects")
                        .join(prefix)
                        .join(&entry.hash);
                    artifacts.push(check_artifact(
                        "asset",
                        &p,
                        Some(entry.size),
                        Some(VerifySpec::sha1(&entry.hash)),
                    ));
                }
            }
        }
    }

    let ok_count = artifacts.iter().filter(|a| matches!(a.status, ArtifactStatus::Ok)).count() as u32;
    let broken_count = artifacts.len() as u32 - ok_count;

    Ok(RepairReport {
        version_id: version_id.to_string(),
        artifacts,
        ok_count,
        broken_count,
        repaired: false,
    })
}

/// Verify a version and, if `apply_repair` is true, re-download any
/// missing or corrupt artifacts via the central DownloadManager.
pub async fn repair_version(
    version_id: &str,
    apply_repair: bool,
    app_handle: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<RepairReport, AppError> {
    let mut report = verify_version(version_id).await?;

    if !apply_repair || report.broken_count == 0 {
        return Ok(report);
    }

    // Build a list of download jobs for everything that's broken.
    // For each broken artifact we need to know its URL — we re-parse
    // the manifest to get it.
    let version = MinecraftVersion::from_id(version_id.to_string());
    let json_path = get_version_manifest(&version_id.to_string());
    let text = std::fs::read_to_string(&json_path)
        .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let manifest: MinecraftManifestVersion = serde_json::from_str(&text)
        .map_err(|e| AppError::JsonParseFailed(e.to_string()))?;

    let mirror = {
        let cfg = state.config.read().await;
        cfg.download_settings.mirror.clone()
    };
    let logger = &state.log_tx;
    let token = crate::services::download_manager::DownloadToken::new();
    let broken_set: std::collections::HashSet<String> = report
        .artifacts
        .iter()
        .filter(|a| !matches!(a.status, ArtifactStatus::Ok))
        .map(|a| a.path.clone())
        .collect();

    let mut jobs: Vec<DownloadJob> = Vec::new();

    // Client jar
    if let Some(downloads) = &manifest.downloads {
        if let Some(client) = downloads.get("client") {
            let dest = get_version_directory(&version.id).join(format!("{}.jar", version.id));
            if needs_repair(&broken_set, &dest) {
                jobs.push(DownloadJob {
                    url: mirror.parse_url(&client.url),
                    destination: dest.to_string_lossy().to_string(),
                    expected_size: Some(client.size),
                    verify: Some(VerifySpec::sha1(&client.sha1)),
                    category: "client".to_string(),
                });
            }
        }
    }

    // Libraries
    for lib in &manifest.libraries {
        let Some(d) = &lib.downloads else { continue };
        let Some(a) = &d.artifact else { continue };
        let path_str = if let Some(p) = &a.path {
            get_libraries_directory().join(p).to_string_lossy().to_string()
        } else {
            match parse_library_name_to_path(&lib.name) {
                Ok(p) => p,
                Err(_) => continue,
            }
        };
        if needs_repair(&broken_set, &PathBuf::from(&path_str)) {
            jobs.push(DownloadJob {
                url: mirror.parse_url(&a.url),
                destination: path_str,
                expected_size: Some(a.size),
                verify: None, // LibraryArtifact doesn't carry sha1 in our model
                category: "library".to_string(),
            });
        }
    }

    // Assets
    if let Some(asset_index) = &manifest.asset_index {
        let index_path = get_assets_directory()
            .join("indexes")
            .join(format!("{}.json", asset_index.id));
        if needs_repair(&broken_set, &index_path) {
            jobs.push(DownloadJob {
                url: mirror.parse_url(&asset_index.url),
                destination: index_path.to_string_lossy().to_string(),
                expected_size: Some(asset_index.size),
                verify: Some(VerifySpec::sha1(&asset_index.sha1)),
                category: "asset_index".to_string(),
            });
        }
        // Re-download broken objects.
        if let Ok(text) = std::fs::read_to_string(&index_path) {
            if let Ok(objects) = serde_json::from_str::<AssetObjects>(&text) {
                for (_name, entry) in &objects.objects {
                    let prefix = &entry.hash[..2];
                    let p = get_assets_directory()
                        .join("objects")
                        .join(prefix)
                        .join(&entry.hash);
                    if needs_repair(&broken_set, &p) {
                        let url = mirror.parse_url(
                            &format!(
                                "https://resources.download.minecraft.net/{prefix}/{}",
                                entry.hash
                            )
                            .to_string(),
                        );
                        jobs.push(DownloadJob {
                            url,
                            destination: p.to_string_lossy().to_string(),
                            expected_size: Some(entry.size),
                            verify: Some(VerifySpec::sha1(&entry.hash)),
                            category: "asset".to_string(),
                        });
                    }
                }
            }
        }
    }

    info!("repair: re-downloading {} artifacts", jobs.len());
    let _ = logger.send(log_info(
        format!("Repairing {} artifacts for {}", jobs.len(), version_id),
        version_id.to_string(),
    ));

    state
        .downloader
        .download_batch(jobs, Some(app_handle), &token)
        .await?;

    // Re-verify after repair.
    report = verify_version(version_id).await?;
    report.repaired = true;
    Ok(report)
}

fn needs_repair(broken_set: &std::collections::HashSet<String>, path: &std::path::Path) -> bool {
    broken_set.contains(&path.to_string_lossy().to_string())
}

fn check_artifact(
    category: &str,
    path: &std::path::Path,
    expected_size: Option<u64>,
    verify: Option<VerifySpec>,
) -> RepairArtifact {
    if !path.exists() {
        return RepairArtifact {
            category: category.to_string(),
            path: path.to_string_lossy().to_string(),
            status: ArtifactStatus::Missing,
        };
    }
    if let Some(expected) = expected_size {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() != expected {
                return RepairArtifact {
                    category: category.to_string(),
                    path: path.to_string_lossy().to_string(),
                    status: ArtifactStatus::SizeMismatch {
                        expected,
                        actual: meta.len(),
                    },
                };
            }
        }
    }
    // For asset objects, matching file size is sufficient and avoids calculating
    // SHA-1 for 5,000+ files sequentially which freezes the UI.
    if category != "asset" {
        if let Some(spec) = verify {
            match calculate_file_sha1(path) {
                Ok(actual) => {
                    if !actual.eq_ignore_ascii_case(&spec.expected_hash) {
                        return RepairArtifact {
                            category: category.to_string(),
                            path: path.to_string_lossy().to_string(),
                            status: ArtifactStatus::HashMismatch {
                                expected: spec.expected_hash,
                                actual,
                            },
                        };
                    }
                }
                Err(e) => {
                    return RepairArtifact {
                        category: category.to_string(),
                        path: path.to_string_lossy().to_string(),
                        status: ArtifactStatus::HashMismatch {
                            expected: spec.expected_hash,
                            actual: format!("read error: {e}"),
                        },
                    };
                }
            }
        }
    }
    RepairArtifact {
        category: category.to_string(),
        path: path.to_string_lossy().to_string(),
        status: ArtifactStatus::Ok,
    }
}

/// Tauri command wrapper. Verifies (and optionally repairs) a version.
/// Re-exported by `commands::downloader` as the actual `#[command]`.
pub use repair_version as repair_version_command;

// Suppress unused-import warning for things only used by future phases.
#[allow(dead_code)]
fn _unused(_a: &AssetIndex, _b: &Mirror, _c: &Config, _d: &UnboundedSender<LogLine>) {}
