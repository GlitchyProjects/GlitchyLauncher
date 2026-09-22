//! Tauri commands wrapping the Modrinth API client.
//!
//! The wire structs in `services::modrinth_helper` deserialize
//! Modrinth's snake_case JSON; the DTOs here serialize as camelCase so
//! the frontend keeps a single naming convention across all commands.

use serde::Serialize;
use tauri::{command, AppHandle};

use crate::models::error::AppError;
use crate::models::mods::BackpackCategory;
use crate::services::directory_manager::{ensure_instance_dirs, get_category_folder};
use crate::services::modrinth_helper as api;
use crate::services::path_safety::sanitize_user_path_segment;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthSearchResultsDto {
    pub hits: Vec<ModrinthSearchHitDto>,
    pub total_hits: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthSearchHitDto {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthVersionDto {
    pub id: String,
    pub name: Option<String>,
    pub version_number: Option<String>,
    pub version_type: Option<String>,
    pub date_published: Option<String>,
    pub downloads: Option<u64>,
    pub loaders: Option<Vec<String>>,
    pub game_versions: Option<Vec<String>>,
    pub primary_file: ModrinthFileDto,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthFileDto {
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: Option<u64>,
}

impl From<api::ModrinthSearchHit> for ModrinthSearchHitDto {
    fn from(h: api::ModrinthSearchHit) -> Self {
        Self {
            project_id: h.project_id,
            slug: h.slug,
            author: h.author,
            title: h.title,
            description: h.description,
            categories: h.categories,
            project_type: h.project_type,
            downloads: h.downloads,
            icon_url: h.icon_url,
        }
    }
}

impl From<api::ModrinthProjectVersion> for ModrinthVersionDto {
    fn from(v: api::ModrinthProjectVersion) -> Self {
        // Modrinth always sends at least one file for a released version;
        // if the array is unexpectedly empty, fall back to placeholder
        // values so the DTO stays serializable (the frontend hides the
        // download button when the URL is empty).
        let primary = api::primary_file(&v);
        let primary_file = match primary {
            Some(f) => ModrinthFileDto {
                url: f.url.clone(),
                filename: f.filename.clone(),
                primary: f.primary.unwrap_or(false),
                size: f.size,
            },
            None => ModrinthFileDto {
                url: String::new(),
                filename: v
                    .version_number
                    .clone()
                    .unwrap_or_else(|| "unknown.jar".to_string()),
                primary: false,
                size: None,
            },
        };
        Self {
            id: v.id,
            name: v.name,
            version_number: v.version_number,
            version_type: v.version_type,
            date_published: v.date_published,
            downloads: v.downloads,
            loaders: v.loaders,
            game_versions: v.game_versions,
            primary_file,
        }
    }
}

/// Search Modrinth for projects of one type ("mod", "resourcepack",
/// "shader") compatible with the given game version and loader. Empty
/// strings disable the corresponding facet filter.
#[command]
pub async fn modrinth_search(
    query: String,
    game_version: String,
    loader: String,
    project_type: String,
    offset: u64,
    limit: u64,
) -> Result<ModrinthSearchResultsDto, AppError> {
    let results =
        api::search(&query, &game_version, &loader, &project_type, offset, limit).await?;
    Ok(ModrinthSearchResultsDto {
        hits: results.hits.into_iter().map(Into::into).collect(),
        total_hits: results.total_hits,
    })
}

/// List a project's released versions, newest first, filtered by the
/// instance's game version and loader.
#[command]
pub async fn modrinth_get_project_versions(
    project_id: String,
    game_version: String,
    loader: String,
) -> Result<Vec<ModrinthVersionDto>, AppError> {
    let versions = api::get_project_versions(&project_id, &game_version, &loader).await?;
    Ok(versions.into_iter().map(Into::into).collect())
}

/// Download one Modrinth version's primary file into the instance's own
/// folder for the given category (mods / resourcepacks / shaderpacks).
/// Returns the saved file name.
#[command]
pub async fn modrinth_download_item(
    app: AppHandle,
    version_id: String,
    instance_id: String,
    category: BackpackCategory,
) -> Result<String, AppError> {
    let version = api::get_version(&version_id).await?;
    let file = api::primary_file(&version)
        .ok_or_else(|| AppError::DownloadFailed(format!("version {version_id} has no files")))?;

    ensure_instance_dirs(&instance_id).await?;
    let dest_dir = get_category_folder(&instance_id, category.folder_name())?;
    // The filename comes from Modrinth but still goes through the same
    // segment validation as user input before joining the path.
    let dest = sanitize_user_path_segment(&dest_dir, &file.filename)?;

    api::download_file(&file.url, &dest).await?;
    crate::commands::mods::recheck_achievements(&app)?;
    Ok(file.filename.clone())
}
