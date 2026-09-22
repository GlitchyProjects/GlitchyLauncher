//! Modrinth modpack browsing and installation.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::RwLock;
use zip::ZipArchive;

use crate::commands::downloader::run_install;
use crate::models::config::Config;
use crate::models::downloader::VersionLoader;
use crate::models::error::AppError;
use crate::models::logger::LogLine;
use crate::models::versions::VersionBase;
use crate::services::directory_manager::{
    ensure_instance_dirs, get_instance_directory, get_temp_directory, get_version_manifest,
    get_versions_directory,
};
use crate::services::download_manager::{self, DownloadJob, VerifySpec};
use crate::services::download_session::{self, SessionHandle};
use crate::services::game_downloader::download_file_verified_for_session;
use crate::services::modrinth_helper;
use crate::services::path_safety::sanitize_archive_entry;
use crate::services::version_manager::reload_installed_versions;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSearchResults {
    pub hits: Vec<ModpackSearchHit>,
    pub total_hits: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackSearchHit {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackVersion {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub download_url: Option<String>,
    pub size: u64,
    pub date_published: String,
    pub game_versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct MrpackIndex {
    name: String,
    #[serde(default)]
    version_id: String,
    dependencies: HashMap<String, String>,
    files: Vec<MrpackFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackFile {
    path: String,
    hashes: HashMap<String, String>,
    downloads: Vec<String>,
    file_size: u64,
    #[serde(default)]
    env: Option<MrpackEnvironment>,
}

#[derive(Debug, Deserialize)]
struct MrpackEnvironment {
    client: Option<String>,
}

#[command]
pub async fn search_modpacks(
    query: String,
    offset: u64,
    limit: u64,
) -> Result<ModpackSearchResults, AppError> {
    let results = modrinth_helper::search(&query, "", "", "modpack", offset, limit).await?;
    Ok(ModpackSearchResults {
        total_hits: results.total_hits,
        hits: results
            .hits
            .into_iter()
            .map(|hit| ModpackSearchHit {
                project_id: hit.project_id,
                title: hit.title.unwrap_or_else(|| "Untitled modpack".to_string()),
                description: hit.description.unwrap_or_default(),
                author: hit.author.unwrap_or_default(),
                icon_url: hit.icon_url,
                downloads: hit.downloads.unwrap_or(0),
            })
            .collect(),
    })
}

#[command]
pub async fn get_modpack_versions(
    project_id: String,
) -> Result<Vec<ModpackVersion>, AppError> {
    let versions = modrinth_helper::get_project_versions(&project_id, "", "").await?;
    Ok(versions
        .into_iter()
        .filter_map(|version| {
            let file = modrinth_helper::primary_file(&version)?;
            Some(ModpackVersion {
                id: version.id.clone(),
                name: version
                    .name
                    .clone()
                    .unwrap_or_else(|| version.version_number.clone().unwrap_or_default()),
                file_name: file.filename.clone(),
                download_url: Some(file.url.clone()),
                size: file.size.unwrap_or(0),
                date_published: version.date_published.unwrap_or_default(),
                game_versions: version.game_versions.unwrap_or_default(),
            })
        })
        .collect())
}

#[command]
pub async fn install_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
    name: String,
) -> Result<String, AppError> {
    let config = state.config.clone();
    let logger = state.log_tx.clone();
    let session = download_session::create_session(&app, &name, &version_id);
    let session_id = session.id().to_string();
    tauri::async_runtime::spawn(async move {
        let result = download_remote_pack(
            &app,
            &config,
            &logger,
            &version_id,
            &name,
            &session,
        )
        .await;
        finish_session(result, &session).await;
    });
    Ok(session_id)
}

#[command]
pub async fn import_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let Some(file_path) = app
        .dialog()
        .file()
        .add_filter("Modrinth modpack", &["mrpack"])
        .blocking_pick_file()
        .and_then(|path| path.into_path().ok())
    else {
        return Ok(None);
    };
    let name = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported modpack")
        .to_string();
    let config = state.config.clone();
    let logger = state.log_tx.clone();
    let session = download_session::create_session(&app, &name, "modpack-import");
    let session_id = session.id().to_string();
    tauri::async_runtime::spawn(async move {
        let result = install_archive(&app, &config, &logger, &file_path, &session).await;
        finish_session(result, &session).await;
    });
    Ok(Some(session_id))
}

async fn finish_session(result: Result<String, AppError>, session: &SessionHandle) {
    match result {
        Ok(version_id) => {
            session.set_version_id(&version_id);
            session.complete();
            let _ = reload_installed_versions().await;
        }
        Err(AppError::Cancelled) => session.mark_cancelled(),
        Err(error) => {
            warn!("modpack installation failed: {error:?}");
            session.fail(&error.to_string());
        }
    }
}

async fn download_remote_pack(
    app: &AppHandle,
    config: &Arc<RwLock<Config>>,
    logger: &tokio::sync::mpsc::UnboundedSender<LogLine>,
    version_id: &str,
    name: &str,
    session: &SessionHandle,
) -> Result<String, AppError> {
    let version = modrinth_helper::get_version(version_id).await?;
    let file = modrinth_helper::primary_file(&version).ok_or_else(|| {
        AppError::DownloadFailed("This Modrinth version has no pack file".to_string())
    })?;
    let (url, file_name, size) = (
        file.url.clone(),
        file.filename.clone(),
        file.size.unwrap_or(0),
    );
    tokio::fs::create_dir_all(get_temp_directory())
        .await
        .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let archive = get_temp_directory().join(format!("{}-{file_name}", session.id()));
    session.set_phase("modpack");
    download_file_verified_for_session(&archive, url, size, None, Some(session)).await?;
    let result = install_archive(app, config, logger, &archive, session).await;
    let _ = tokio::fs::remove_file(&archive).await;
    info!("processed remote modpack {name}");
    result
}

async fn install_archive(
    app: &AppHandle,
    config: &Arc<RwLock<Config>>,
    logger: &tokio::sync::mpsc::UnboundedSender<LogLine>,
    archive_path: &Path,
    session: &SessionHandle,
) -> Result<String, AppError> {
    let mut archive = ZipArchive::new(
        File::open(archive_path).map_err(|error| AppError::FileReadFailed(error.to_string()))?,
    )
    .map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;
    if let Some(json) = read_zip_text(&mut archive, "modrinth.index.json")? {
        let index: MrpackIndex = serde_json::from_str(&json)
            .map_err(|error| AppError::JsonParseFailed(format!("modrinth.index.json: {error}")))?;
        return install_mrpack(app, config, logger, archive, index, session).await;
    }
    Err(AppError::ManifestParseFailed(
        "The archive is not a Modrinth .mrpack file".to_string(),
    ))
}

fn read_zip_text(archive: &mut ZipArchive<File>, name: &str) -> Result<Option<String>, AppError> {
    let Ok(mut file) = archive.by_name(name) else {
        return Ok(None);
    };
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
    Ok(Some(text))
}

async fn install_mrpack(
    app: &AppHandle,
    config: &Arc<RwLock<Config>>,
    logger: &tokio::sync::mpsc::UnboundedSender<LogLine>,
    mut archive: ZipArchive<File>,
    index: MrpackIndex,
    session: &SessionHandle,
) -> Result<String, AppError> {
    let loader = loader_from_dependencies(&index.dependencies)?;
    let base_id = install_base(app, config, logger, loader, &index.name, session).await?;
    let version_id = create_modpack_profile(&base_id, &index.name, session.id())?;
    session.set_version_id(&version_id);
    ensure_instance_dirs(&version_id).await?;
    let instance = get_instance_directory(&version_id)?;
    session.set_pass_range(0.85, 1.0);
    session.set_phase("modpack");

    let mut jobs = Vec::with_capacity(index.files.len());
    for item in index.files {
        if item.env.as_ref().and_then(|env| env.client.as_deref()) == Some("unsupported") {
            continue;
        }
        let Some(url) = item.downloads.first() else { continue };
        validate_download_url(url)?;
        let destination = sanitize_archive_entry(&instance, &item.path)?;
        jobs.push(DownloadJob {
            url: url.clone(),
            destination: destination.to_string_lossy().to_string(),
            expected_size: Some(item.file_size),
            verify: item.hashes.get("sha1").cloned().map(VerifySpec::sha1),
            category: "modpack".to_string(),
        });
    }
    download_jobs(jobs, app, session).await?;
    extract_prefix(&mut archive, "overrides/", &instance)?;
    extract_prefix(&mut archive, "client-overrides/", &instance)?;
    write_metadata(&instance, "modrinth", &index.name, &index.version_id)?;
    Ok(version_id)
}

fn loader_from_dependencies(dependencies: &HashMap<String, String>) -> Result<VersionLoader, AppError> {
    let minecraft = dependencies.get("minecraft").ok_or_else(|| {
        AppError::ManifestParseFailed("Modpack does not declare a Minecraft version".to_string())
    })?;
    if let Some(fabric) = dependencies.get("fabric-loader") {
        return Ok(VersionLoader { id: format!("{minecraft}-{fabric}"), base: VersionBase::FABRIC, date: String::new() });
    }
    if let Some(forge) = dependencies.get("forge") {
        return Ok(VersionLoader { id: format!("{minecraft}-{forge}"), base: VersionBase::FORGE, date: String::new() });
    }
    if dependencies.contains_key("neoforge") || dependencies.contains_key("quilt-loader") {
        return Err(AppError::VersionNotFound("This pack requires NeoForge or Quilt, which is not installed by this launcher yet".to_string()));
    }
    Ok(VersionLoader { id: minecraft.clone(), base: VersionBase::VANILLA, date: String::new() })
}

async fn install_base(
    app: &AppHandle,
    config: &Arc<RwLock<Config>>,
    logger: &tokio::sync::mpsc::UnboundedSender<LogLine>,
    loader: VersionLoader,
    name: &str,
    session: &SessionHandle,
) -> Result<String, AppError> {
    let mut version_id = loader.get_installed_id();
    session.set_version_id(&version_id);
    session.set_pass_range(0.0, 0.85);
    run_install(app.clone(), config.clone(), logger.clone(), loader, name.to_string(), session, &mut version_id).await?;
    Ok(version_id)
}

fn create_modpack_profile(base_id: &str, name: &str, session_id: &str) -> Result<String, AppError> {
    let source_path = get_version_manifest(base_id);
    let text = std::fs::read_to_string(&source_path)
        .map_err(|error| AppError::FileReadFailed(format!("{}: {error}", source_path.display())))?;
    let mut profile: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| AppError::JsonParseFailed(format!("{}: {error}", source_path.display())))?;
    let mut slug = name
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character.to_ascii_lowercase() } else { '-' })
        .collect::<String>();
    while slug.contains("--") { slug = slug.replace("--", "-"); }
    slug = slug.trim_matches('-').chars().take(36).collect();
    if slug.is_empty() { slug = "pack".to_string(); }
    let suffix: String = session_id.chars().filter(|character| character.is_ascii_hexdigit()).take(8).collect();
    let profile_id = format!("modpack-{slug}-{suffix}");
    profile["id"] = serde_json::Value::String(profile_id.clone());
    if profile.get("inheritsFrom").and_then(serde_json::Value::as_str).is_none() {
        profile["inheritsFrom"] = serde_json::Value::String(base_id.to_string());
    }
    let directory = get_versions_directory().join(&profile_id);
    std::fs::create_dir_all(&directory)
        .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let destination = directory.join(format!("{profile_id}.json"));
    let serialized = serde_json::to_vec_pretty(&profile)
        .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
    std::fs::write(&destination, serialized)
        .map_err(|error| AppError::FileWriteFailed(format!("{}: {error}", destination.display())))?;
    Ok(profile_id)
}

async fn download_jobs(
    jobs: Vec<DownloadJob>,
    app: &AppHandle,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let progress_session = session.clone();
    let progress = Arc::new(move |done: u32, total: u32, current: String| {
        if total > 0 { progress_session.progress(done as f64 / total as f64, &current); }
    });
    let transfer_session = session.clone();
    let transfer = Arc::new(move |downloaded: u64, total: u64, current: String| {
        transfer_session.transfer(downloaded, total, &current);
    });
    download_manager::global()
        .download_batch_with_callbacks(jobs, Some(app), &session.token(), Some(progress), Some(transfer))
        .await
}

fn validate_download_url(url: &str) -> Result<(), AppError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|error| AppError::PathValidationFailed(format!("invalid download URL: {error}")))?;
    if parsed.scheme() != "https" {
        return Err(AppError::PathValidationFailed("Modpack files must use HTTPS".to_string()));
    }
    Ok(())
}

fn extract_prefix(archive: &mut ZipArchive<File>, prefix: &str, root: &Path) -> Result<(), AppError> {
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;
        let name = entry.name().replace('\\', "/");
        let Some(relative) = name.strip_prefix(prefix) else { continue };
        if relative.is_empty() { continue; }
        let destination = sanitize_archive_entry(root, relative)?;
        if entry.is_dir() {
            std::fs::create_dir_all(&destination).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
        }
        let mut output = File::create(&destination).map_err(|error| AppError::FileCreateFailed(error.to_string()))?;
        std::io::copy(&mut entry, &mut output).map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
    }
    Ok(())
}

fn write_metadata(root: &Path, source: &str, name: &str, version: &str) -> Result<(), AppError> {
    let metadata = serde_json::json!({ "source": source, "name": name, "version": version });
    let mut file = File::create(root.join(".glitchy-modpack.json"))
        .map_err(|error| AppError::FileCreateFailed(error.to_string()))?;
    file.write_all(metadata.to_string().as_bytes())
        .map_err(|error| AppError::FileWriteFailed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_fabric_from_mrpack_dependencies() {
        let dependencies = HashMap::from([
            ("minecraft".to_string(), "1.21.1".to_string()),
            ("fabric-loader".to_string(), "0.16.9".to_string()),
        ]);
        let loader = loader_from_dependencies(&dependencies).unwrap();
        assert_eq!(loader.id, "1.21.1-0.16.9");
        assert_eq!(loader.base, VersionBase::FABRIC);
    }

    #[test]
    fn rejects_insecure_modpack_download_urls() {
        assert!(validate_download_url("http://example.com/mod.jar").is_err());
        assert!(validate_download_url("https://cdn.modrinth.com/mod.jar").is_ok());
    }
}
