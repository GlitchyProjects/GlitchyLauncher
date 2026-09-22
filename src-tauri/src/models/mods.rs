use serde::{Deserialize, Serialize};
fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct McModInfo {
    #[serde(rename = "modid")]
    pub mod_id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "logoFile")]
    pub logo_file: Option<String>,
    pub url: String,
    pub mcversion: String,
    pub version: String,
    pub screenshots: Vec<String>,
    pub dependencies: Vec<String>,
    #[serde(rename = "authorList")]
    pub author_list: Vec<String>,
    #[serde(rename = "updateUrl")]
    pub update_url: Option<String>,
    pub credits: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]

pub struct FabricModInfo {
    #[serde(rename = "id")]
    pub mod_id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "icon")]
    pub logo_file: Option<String>,
    pub contact: Option<FabricModInfoContact>,
    pub version: String,
    #[serde(rename = "authors")]
    pub author_list: Vec<String>,
    #[serde(rename = "updateUrl")]
    pub update_url: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FabricModInfoContact {
    pub homepage: Option<String>,
    pub issues: Option<String>,
    pub sources: Option<String>,
    pub twitter: Option<String>,
    pub discord: Option<String>,
}


/// One of the three Backpack sections. Every command that manages
/// instance content takes this so mods, resource packs and shader packs
/// each land in their own per-instance folder — nothing mixes between
/// installed versions.
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BackpackCategory {
    Mods,
    ResourcePacks,
    ShaderPacks,
}

impl BackpackCategory {
    /// Subfolder name inside the instance directory.
    pub fn folder_name(&self) -> &'static str {
        match self {
            BackpackCategory::Mods => "mods",
            BackpackCategory::ResourcePacks => "resourcepacks",
            BackpackCategory::ShaderPacks => "shaderpacks",
        }
    }

    /// Modrinth `project_type:` facet value.
    pub fn project_type(&self) -> &'static str {
        match self {
            BackpackCategory::Mods => "mod",
            BackpackCategory::ResourcePacks => "resourcepack",
            BackpackCategory::ShaderPacks => "shader",
        }
    }

    /// File extension of an enabled item (lowercase, no dot).
    pub fn enabled_ext(&self) -> &'static str {
        match self {
            BackpackCategory::Mods => "jar",
            BackpackCategory::ResourcePacks | BackpackCategory::ShaderPacks => "zip",
        }
    }

    /// File dialog filter extensions for the import picker.
    pub fn import_extensions(&self) -> &'static [&'static str] {
        match self {
            BackpackCategory::Mods => &["jar", "disabled"],
            BackpackCategory::ResourcePacks | BackpackCategory::ShaderPacks => &["zip", "disabled"],
        }
    }

    /// Human-readable label used in error details.
    pub fn label(&self) -> &'static str {
        match self {
            BackpackCategory::Mods => "mod",
            BackpackCategory::ResourcePacks => "resource pack",
            BackpackCategory::ShaderPacks => "shader pack",
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub path: String,
    pub mod_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
impl ModInfo {
    pub fn new(
        path: String,
        mod_id: String,
        display_name: String,
        version: String,
        description: String,
    ) -> Self {
        Self {
            path,
            mod_id,
            name: display_name,
            version,
            description,
            enabled: true,
        }
    }
}