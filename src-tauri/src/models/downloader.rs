use crate::models::error::AppError;
use crate::models::versions::VersionBase;
use crate::models::versions::VersionBase::{FABRIC, FORGE, OPTIFINE};
use crate::models::versions::VersionType;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub latest: LatestVersionDetail,
    pub versions: Vec<VersionInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(rename = "totalSize")]
    pub total_size: u64,
    pub url: String,
}

// Model for reading individual asset entries inside the assets index file
#[derive(Deserialize, Debug)]
pub struct AssetObjects {
    pub objects: HashMap<String, AssetEntry>,
}

#[derive(Deserialize, Debug)]
pub struct AssetEntry {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct LibraryInfo {
    pub name: String,
    pub size: u64,
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
}

pub struct LibraryRules {
    pub allowed_oses: Vec<String>,
    pub disallowed_oses: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoggingClient {
    pub argument: String,
    pub file: LoggingFile,
    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Logging {
    pub client: LoggingClient,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DownloadDetail {
    pub url: String,
    pub size: u64,
    pub sha1: String,
}

fn empty_object_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let opt_value: Option<Value> = Option::deserialize(deserializer)?;

    match opt_value {
        Some(Value::Object(map)) if map.is_empty() => Ok(None),
        Some(value) => T::deserialize(value)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftManifestVersion {
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndex>,
    pub downloads: Option<HashMap<String, DownloadDetail>>,
    #[serde(default, deserialize_with = "empty_object_as_none")]
    pub logging: Option<Logging>,
    pub java_version: Option<JavaVersion>,
    pub inherits_from: Option<String>,
    pub id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct RuleOS {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub action: String,
    pub os: Option<RuleOS>,
}

#[derive(Debug, Deserialize)]
pub struct Library {
    pub name: String,
    pub downloads: Option<LibraryDownloads>,
    pub rules: Option<Vec<Rule>>,
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
    pub classifiers: Option<HashMap<String, LibraryArtifact>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LibraryArtifact {
    pub path: Option<String>,
    pub url: String,
    pub size: u64,
    /// SHA-1 hash from the Mojang manifest. Present on all modern
    /// library artifacts. Missing on some legacy/Forge artifacts,
    /// in which case repair falls back to size-only verification.
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LatestVersionDetail {
    pub release: String,
    pub snapshot: String,
}

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: VersionType,
    pub url: String,
    pub time: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionLoader {
    pub id: String,
    pub base: VersionBase,
    pub date: String,
}

impl VersionLoader {
    pub fn get_installed_id(&self) -> String {
        match self.base {
            VersionBase::VANILLA => self.id.clone(),
            FORGE => {
                // Forge version IDs look like "1.18.2-40.2.0" or
                // "1.18.2-40.2.0-beta". We split on the FIRST '-' to
                // get the vanilla version and the forge version suffix.
                let mut parts = self.id.splitn(2, '-');
                let vanilla_id = parts.next().unwrap_or(&self.id);
                let forge_ver = parts.next().unwrap_or("");
                if forge_ver.is_empty() {
                    // No '-' found — treat the whole ID as-is.
                    self.id.clone()
                } else {
                    format!("{vanilla_id}-forge-{forge_ver}")
                }
            }
            VersionBase::NEOFORGE => self.id.clone(),
            FABRIC => {
                // Fabric version IDs from the frontend look like
                // "1.18.2-0.19.3" (mcVersion-loaderVersion). We need
                // to produce "fabric-loader-0.19.3-1.18.2".
                let mut parts = self.id.splitn(2, '-');
                let mc_ver = parts.next().unwrap_or(&self.id);
                let loader_ver = parts.next().unwrap_or("");
                if loader_ver.is_empty() {
                    // No '-' found — can't construct a valid Fabric ID.
                    // Fall back to the raw ID to avoid a panic.
                    self.id.clone()
                } else {
                    format!("fabric-loader-{loader_ver}-{mc_ver}")
                }
            }
            VersionBase::LITELOADER => self.id.clone(),
            OPTIFINE => self.id.clone(),
        }
    }
    pub fn get_fabric_loader_id(&self) -> String {
        // "1.18.2-0.19.3" → "0.19.3" (the loader version, 2nd part).
        self.id.splitn(2, '-').nth(1).unwrap_or("").to_string()
    }
    pub fn get_fabric_version_id(&self) -> String {
        // "1.18.2-0.19.3" → "1.18.2" (the MC version, 1st part).
        self.id.splitn(2, '-').next().unwrap_or("").to_string()
    }

    pub fn get_forge_version_id(&self) -> String {
        self.id
            .split_once('-')
            .map_or_else(String::new, |(minecraft, _)| minecraft.to_string())
    }

    pub fn get_optifine_version_id(&self) -> String {
        self.id
            .split_once("-OptiFine_")
            .map_or_else(String::new, |(minecraft, _)| minecraft.to_string())
    }

    pub fn get_optifine_release(&self) -> Option<(String, String)> {
        if let Some(metadata) = self.date.strip_prefix("OPTIFINE|") {
            let mut fields = metadata.splitn(3, '|');
            let kind = fields.next()?;
            let patch = fields.next()?;
            if !kind.is_empty() && !patch.is_empty() {
                return Some((kind.to_string(), patch.to_string()));
            }
        }

        let (_, release) = self.id.split_once("-OptiFine_")?;
        let mut fields = release.splitn(3, '_');
        let edition = fields.next()?;
        let tier = fields.next()?;
        let patch = fields.next()?;
        if edition.is_empty() || tier.is_empty() || patch.is_empty() {
            return None;
        }
        Some((format!("{edition}_{tier}"), patch.to_string()))
    }

    pub fn get_optifine_filename(&self) -> Option<String> {
        let metadata = self.date.strip_prefix("OPTIFINE|")?;
        let filename = metadata.splitn(3, '|').nth(2)?;
        if filename.is_empty()
            || !filename
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
        {
            return None;
        }
        Some(filename.to_string())
    }
}

#[derive(Debug, Deserialize)]
pub struct OptifineVersion {
    pub mcversion: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub patch: String,
    #[serde(default)]
    pub filename: String,
}

impl OptifineVersion {
    pub fn profile_id(&self) -> String {
        format!("{}-OptiFine_{}_{}", self.mcversion, self.kind, self.patch)
    }
}

// Models designed specifically for legacy Forge installer profile JSON extraction
#[derive(Deserialize, Debug)]
pub struct ForgeInstallProfile {
    pub install: Option<ForgeInstallData>,
    #[serde(rename = "versionInfo")]
    pub version_info: Option<ForgeVersionJsonInfo>,
    pub libraries: Option<Vec<ForgeLibrary>>,
}
fn default_mirror_list() -> String {
    "https://files.minecraftforge.net/mirror-brand.list".to_string()
}
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeInstallData {
    pub profile_name: String,
    pub target: String,
    pub path: String,
    pub version: String,
    pub file_path: String,
    pub welcome: Option<String>,
    pub minecraft: String,

    #[serde(default = "default_mirror_list")]
    pub mirror_list: String,
    pub logo: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeVersionJsonInfo {
    pub id: String,
    pub time: Option<String>,
    pub release_time: Option<String>,
    pub r#type: Option<String>,
    pub main_class: Option<String>,
    pub minecraft_arguments: Option<String>,
    pub minimum_launcher_version: Option<u32>,
    pub assets: Option<String>,
    pub inherits_from: Option<String>,
    pub jar: Option<String>,
    pub libraries: Vec<ForgeLibrary>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ForgeLibrary {
    pub name: String,
    pub url: Option<String>,
    pub downloads: Option<ForgeLibraryDownloads>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ForgeLibraryDownloads {
    pub artifact: Option<ForgeArtifact>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ForgeArtifact {
    pub path: Option<String>,
    pub url: String,
}

// Helper: convert a `serde_json::Value` describing a Minecraft library
// into a typed `LibraryInfo`. Returns `AppError::JsonParseFailed` on
// missing fields so callers can skip the bad entry instead of panicking.
pub fn library_from_value_legacy(value: &Value) -> Result<LibraryInfo, AppError> {
    let library_name = value
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::JsonParseFailed("library missing 'name'".to_string()))?;

    let library_downloads = value.get("downloads").ok_or_else(|| {
        AppError::JsonParseFailed(format!("library '{library_name}' missing 'downloads'"))
    })?;

    let library_artifact = library_downloads.get("artifact").ok_or_else(|| {
        AppError::JsonParseFailed(format!("library '{library_name}' missing 'artifact'"))
    })?;

    let library_path = if library_artifact.get("path").is_none() {
        let args: Vec<&str> = library_name.split(':').collect();
        if args.len() != 3 {
            return Err(AppError::JsonParseFailed(format!(
                "library '{library_name}' has invalid maven coordinate"
            )));
        }
        let group_id = args[0].replace('.', "/");
        let artifact = args[1];
        let version = args[2];
        let artifact_version = format!("{artifact}-{version}.jar");
        format!("{group_id}/{artifact}/{version}/{artifact_version}")
    } else {
        library_artifact["path"]
            .as_str()
            .ok_or_else(|| {
                AppError::JsonParseFailed(format!("library '{library_name}' has non-string path"))
            })?
            .to_string()
    };

    let library_url = library_artifact
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::JsonParseFailed(format!("library '{library_name}' missing url"))
        })?;

    let library_size = library_artifact
        .get("size")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let library_sha1 = library_artifact
        .get("sha1")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(LibraryInfo {
        name: library_name.to_string(),
        size: library_size,
        path: library_path,
        url: library_url.to_string(),
        sha1: library_sha1,
    })
}
