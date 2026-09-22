//! Modrinth API client (https://api.modrinth.com/v2).
//!
//! Implements project search, project-version listing and mod file
//! download. All JSON requests go through the shared [`http`] client
//! (timeouts + retries); file downloads use a dedicated client because
//! mod jars can comfortably exceed the shared client's 60s cap.
//!
//! The wire structs below deserialize Modrinth's snake_case JSON.
//! `commands::modrinth` converts them into camelCase DTOs for the
//! frontend so the JS side keeps its naming conventions.

use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use futures_util::StreamExt;
use log::info;
use reqwest::Client;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::models::error::AppError;
use crate::services::http;

const MODRINTH_API: &str = "https://api.modrinth.com/v2";
const MAX_PAGE_LIMIT: u64 = 40;

static DOWNLOAD_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .user_agent(format!("GlitchyLauncher/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build modrinth download client")
});

/// Reject path-like segments before they end up in a URL path slot.
fn is_safe_url_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.contains('/')
        && !segment.contains('\\')
        && !segment.contains("..")
        && !segment.chars().any(|c| c.is_control())
}

/// Build a request URL with properly percent-encoded query params.
/// Modrinth's `facets`/`game_versions` params are JSON strings full of
/// `[`, `]` and `"` — they MUST be encoded, so never use `format!`
/// interpolation for the query string.
fn build_url(path: &str, params: &[(String, String)]) -> Result<String, AppError> {
    reqwest::Url::parse(&format!("{MODRINTH_API}{path}"))
        .map(|mut u| {
            u.query_pairs_mut()
                .extend_pairs(params.iter())
                .finish()
                .to_string()
        })
        .map_err(|e| AppError::UnknownError(format!("invalid modrinth url: {e}")))
}

/// Search Modrinth for projects of one type ("mod", "resourcepack",
/// "shader").
///
/// `game_version` and `loader` are optional facet filters (empty string
/// = no filter) so results are restricted to items compatible with the
/// selected instance.
pub async fn search(
    query: &str,
    game_version: &str,
    loader: &str,
    project_type: &str,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchResults, AppError> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{project_type}")]];
    if !game_version.is_empty() {
        facets.push(vec![format!("versions:{game_version}")]);
    }
    if !loader.is_empty() {
        facets.push(vec![format!("categories:{loader}")]);
    }
    let facets = serde_json::to_string(&facets)
        .map_err(|e| AppError::JsonParseFailed(format!("facets: {e}")))?;

    let url = build_url(
        "/search",
        &[
            ("query".to_string(), query.to_string()),
            ("facets".to_string(), facets),
            ("index".to_string(), "relevance".to_string()),
            ("offset".to_string(), offset.to_string()),
            ("limit".to_string(), limit.min(MAX_PAGE_LIMIT).to_string()),
        ],
    )?;
    info!("modrinth search: {url}");
    http::get_json(&url).await
}

/// List the released versions of a project, optionally filtered by MC
/// version and loader.
pub async fn get_project_versions(
    project_id: &str,
    game_version: &str,
    loader: &str,
) -> Result<Vec<ModrinthProjectVersion>, AppError> {
    if !is_safe_url_segment(project_id) {
        return Err(AppError::PathValidationFailed(format!(
            "invalid project id: {project_id:?}"
        )));
    }
    let mut params: Vec<(String, String)> = Vec::new();
    if !game_version.is_empty() {
        let gv = serde_json::to_string(&[game_version])
            .map_err(|e| AppError::JsonParseFailed(format!("game_versions: {e}")))?;
        params.push(("game_versions".to_string(), gv));
    }
    if !loader.is_empty() {
        let loaders = serde_json::to_string(&[loader])
            .map_err(|e| AppError::JsonParseFailed(format!("loaders: {e}")))?;
        params.push(("loaders".to_string(), loaders));
    }
    let url = build_url(&format!("/project/{project_id}/version"), &params)?;
    http::get_json(&url).await
}

/// Fetch a single version by id (used before downloading its files).
pub async fn get_version(version_id: &str) -> Result<ModrinthProjectVersion, AppError> {
    if !is_safe_url_segment(version_id) {
        return Err(AppError::PathValidationFailed(format!(
            "invalid version id: {version_id:?}"
        )));
    }
    let url = format!("{MODRINTH_API}/version/{version_id}");
    http::get_json(&url).await
}

/// Pick the file to download for a version: Modrinth's primary file,
/// falling back to the first listed one.
pub fn primary_file(version: &ModrinthProjectVersion) -> Option<&ModrinthVersionFile> {
    version
        .files
        .iter()
        .find(|f| f.primary.unwrap_or(false))
        .or_else(|| version.files.first())
}

/// Stream a Modrinth CDN file to `dest`, creating parent directories.
pub async fn download_file(url: &str, dest: &Path) -> Result<(), AppError> {
    let resp = DOWNLOAD_CLIENT
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::DownloadFailed(format!("{url}: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::DownloadFailed(format!("HTTP {status} for {url}")));
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", parent.display())))?;
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| AppError::FileCreateFailed(format!("{}: {e}", dest.display())))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| AppError::DownloadFailed(format!("{url}: {e}")))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| AppError::FileWriteFailed(format!("{}: {e}", dest.display())))?;
    }
    file.flush()
        .await
        .map_err(|e| AppError::FileWriteFailed(format!("{}: {e}", dest.display())))?;
    info!("downloaded mod file {} -> {}", url, dest.display());
    Ok(())
}

// ── Wire structs (Modrinth snake_case JSON) ──────────────────────────

#[derive(Deserialize, Debug)]
pub struct ModrinthSearchResults {
    pub hits: Vec<ModrinthSearchHit>,
    pub total_hits: u64,
}

#[derive(Deserialize, Debug)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    pub slug: Option<String>,
    pub author: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub categories: Option<Vec<String>>,
    pub project_type: Option<String>,
    pub downloads: Option<u64>,
    pub icon_url: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ModrinthProjectVersion {
    pub id: String,
    pub name: Option<String>,
    pub version_number: Option<String>,
    pub version_type: Option<String>,
    pub date_published: Option<String>,
    pub downloads: Option<u64>,
    pub loaders: Option<Vec<String>>,
    pub game_versions: Option<Vec<String>>,
    pub files: Vec<ModrinthVersionFile>,
}

#[derive(Deserialize, Debug)]
pub struct ModrinthVersionFile {
    pub url: String,
    pub filename: String,
    pub primary: Option<bool>,
    pub size: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_facet_params() {
        let url = build_url(
            "/search",
            &[("facets".to_string(), "[[\"project_type:mod\"]]".to_string())],
        )
        .unwrap();
        assert!(url.contains("facets=%5B%5B%22project_type%3Amod%22%5D%5D"), "{url}");
    }

    #[test]
    fn rejects_pathlike_project_ids() {
        assert!(!is_safe_url_segment("../admin"));
        assert!(!is_safe_url_segment("a/b"));
        assert!(!is_safe_url_segment(""));
        assert!(is_safe_url_segment("AABBCCDD"));
    }
}
