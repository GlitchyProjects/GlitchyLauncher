use std::path::{Path, PathBuf};

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::downloader;
use crate::models::downloader::MinecraftManifestVersion;
use crate::models::error::AppError;
use crate::models::platform::get_current_os;
use crate::services::directory_manager::{get_libraries_directory, get_versions_directory};
use crate::services::utils::{extend_once, parse_library_name_to_path};

impl PartialEq for VersionType {
    fn eq(&self, other: &Self) -> bool {
        other == self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldAlpha,
    OldBeta,
}

impl MinecraftVersion {
    pub fn is_installed(&self) -> bool {
        Path::new(&self.get_json()).exists()
    }

    /// Construct a MinecraftVersion. `version_path` is the directory
    /// containing the version JSON and JAR, i.e. `<versions>/<id>/`.
    pub fn new(id: String, version_folder: String) -> Self {
        let versions_dir = get_versions_directory();
        let version_path = versions_dir
            .join(version_folder)
            .to_str()
            .unwrap_or_default()
            .to_string();
        Self { id, version_path }
    }

    /// Returns the absolute path to the version JSON file:
    /// `<versions>/<id>/<id>.json`
    ///
    /// CRITICAL: `version_path` already includes the `<id>` directory
    /// (set by `new()` / `from_id()`). The JSON file lives directly
    /// inside it — NOT in a nested subdirectory. The previous
    /// implementation produced `<versions>/<id>/<id>/<id>.json`
    /// (double-nested), which caused "file not found" (os error 3).
    pub fn get_json(&self) -> String {
        format!("{}/{}.json", self.version_path, self.id)
    }

    pub fn from_id(id: String) -> Self {
        MinecraftVersion::new(id.clone(), id)
    }

    /// Construct a `MinecraftVersion` from a version directory by
    /// finding the first valid `<id>.json` inside it.
    ///
    /// Returns `AppError::DirNotFound` when the directory cannot be
    /// read, and `AppError::JsonParseFailed` when no valid manifest is
    /// found inside.
    pub fn from_folder(directory: PathBuf) -> Result<MinecraftVersion, AppError> {
        let read_dir = std::fs::read_dir(&directory)
            .map_err(|e| AppError::DirNotFound(format!("{}: {e}", directory.display())))?;

        for entry in read_dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Ok(json) = serde_json::from_str::<MinecraftManifestVersion>(&text) {
                let version_path = directory
                    .to_str()
                    .ok_or_else(|| {
                        AppError::PathValidationFailed(
                            "version directory is not UTF-8".to_string(),
                        )
                    })?
                    .to_string();
                return Ok(Self {
                    id: json.id,
                    version_path,
                });
            }
        }
        Err(AppError::JsonParseFailed(format!(
            "no valid manifest in {}",
            directory.display()
        )))
    }

    pub fn is_forge(&self) -> bool {
        self.id.to_lowercase().contains("forge")
    }

    /// Load and parse the version JSON. Returns `Value::Null` on any
    /// error so callers can decide whether that's fatal. The launch
    /// flow uses [`Result`] return values for errors that should
    /// surface to the user.
    pub fn load_json(&self) -> Value {
        if !self.is_installed() {
            return Value::Null;
        }
        let Ok(content) = std::fs::read_to_string(PathBuf::from(self.get_json())) else {
            warn!("failed to read version json: {}", self.get_json());
            return Value::Null;
        };
        serde_json::from_str(&content).unwrap_or_else(|e| {
            warn!("failed to parse version json: {}: {e}", self.get_json());
            Value::Null
        })
    }

    /// Resolve the parent version this one inherits from. For Forge
    /// versions without an explicit `inheritsFrom`, infer the parent
    /// from the `<mc>-forge-<ver>` id pattern.
    pub fn get_inherited(&self) -> MinecraftVersion {
        let json = self.load_json();
        if json.get("inheritsFrom").is_none() {
            let id = json["id"].as_str().map(|s| s.to_string()).unwrap_or_default();
            if id.to_lowercase().contains("forge") {
                let parts: Vec<&str> = id.split('-').collect();
                if parts.len() >= 2 && parts[0] != "forge" {
                    return MinecraftVersion::from_id(parts[0].to_string());
                }
            }
            self.clone()
        } else {
            let inherited = json["inheritsFrom"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if inherited.is_empty() {
                self.clone()
            } else {
                MinecraftVersion::from_id(inherited)
            }
        }
    }

    pub fn is_fabric(&self) -> bool {
        self.id.to_lowercase().contains("fabric")
    }

    fn get_library_paths(&self) -> Vec<String> {
        let value = &self.load_json()["libraries"];
        let Some(arr) = value.as_array() else {
            return Vec::new();
        };

        let libraries_path = get_libraries_directory();
        let mut libraries = Vec::new();
        for library in arr {
            if library.get("downloads").is_none() || library["downloads"].is_null() {
                let Some(library_name) = library["name"].as_str() else {
                    continue;
                };
                let Ok(library_path_str) = parse_library_name_to_path(library_name) else {
                    continue;
                };
                let library_path_str =
                    library_path_str.replace('/', std::path::MAIN_SEPARATOR_STR);
                let library_path = PathBuf::from(&library_path_str);
                if library_path.exists() && !libraries.contains(&library_path_str) {
                    libraries.push(library_path_str);
                }
                continue;
            } else if library["downloads"].get("artifact").is_none() {
                // Classifier-based (natives) library.
                let Some(classifiers) = library["downloads"].get("classifiers") else {
                    continue;
                };
                let os = get_current_os();
                let Some(natives) = classifiers.get(format!("natives-{os}")) else {
                    continue;
                };
                let p = if natives.get("path").is_none() {
                    let Some(url) = natives["url"].as_str() else {
                        continue;
                    };
                    let https_less = url.replace("https://", "").replace("http://", "");
                    let url_args: Vec<&str> = https_less.split('/').collect();
                    if url_args.is_empty() {
                        continue;
                    }
                    https_less.replace(url_args[0], "")
                } else {
                    natives["path"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string()
                };
                let path = libraries_path
                    .join(&p)
                    .to_str()
                    .unwrap_or_default()
                    .to_string();
                libraries.push(path.replace('/', std::path::MAIN_SEPARATOR_STR));
                continue;
            }
            let Ok(library_info) = downloader::library_from_value_legacy(library) else {
                continue;
            };
            let path = libraries_path
                .join(library_info.path.replace('\\', std::path::MAIN_SEPARATOR_STR))
                .to_str()
                .unwrap_or_default()
                .replace('\\', std::path::MAIN_SEPARATOR_STR);
            if !libraries.contains(&path) {
                libraries.push(path);
            }
        }

        libraries
    }

    pub fn get_libraries(&self) -> Vec<String> {
        let mut libraries = self.get_library_paths();
        let libraries_2 = self.get_inherited().get_library_paths();
        libraries = libraries
            .into_iter()
            .filter(|x| {
                let path = PathBuf::from(x);
                let parent = match path.parent().and_then(|p| p.parent()) {
                    Some(p) => p,
                    None => return true,
                };
                let artifact = match parent.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_lowercase(),
                    None => return true,
                };
                let inherited_set: Vec<String> = libraries_2
                    .iter()
                    .filter_map(|p| {
                        PathBuf::from(p)
                            .parent()
                            .and_then(|p| p.parent())
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .map(|n| n.to_lowercase())
                    })
                    .collect();
                !inherited_set.contains(&artifact)
            })
            .collect::<Vec<String>>();
        libraries = extend_once(libraries, libraries_2);
        libraries
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionCategory {
    pub versions: Vec<downloader::VersionLoader>,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersion {
    pub id: String,
    pub version_path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VersionBase {
    VANILLA,
    FORGE,
    NEOFORGE,
    FABRIC,
    LITELOADER,
    OPTIFINE,
}

// Suppress unused-import warning while keeping `MinecraftManifestVersion`
// import path stable for downstream refactors.
#[allow(dead_code)]
fn _ensure_import_used(_v: MinecraftManifestVersion) {}
