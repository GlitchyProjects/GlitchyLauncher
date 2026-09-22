use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use log::{info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::error::AppError;
use crate::services::directory_manager::get_mirrors_dir;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Mirror {
    pub name: String,
    pub description: String,
    pub maps: HashMap<String, String>,
}

impl Default for Mirror {
    fn default() -> Self {
        mojang_mirror()
    }
}

impl Mirror {
    /// Rewrite `url` to use this mirror's domain mappings.
    ///
    /// URLs whose host isn't in the mirror's map are returned
    /// unchanged — this is intentional, so non-Mojang hosts (e.g.
    /// `maven.minecraftforge.net`) still work even when a mirror is
    /// active.
    pub fn parse_url(&self, url: &str) -> String {
        let mut url = url.to_string();
        if url.to_lowercase().starts_with("http://") {
            url.insert("http".len(), 's');
        }

        let https_less_url = if url.to_lowercase().starts_with("https://") {
            url["https://".len()..].trim().to_string()
        } else {
            url.clone()
        };

        let domain = https_less_url
            .split('/')
            .next()
            .unwrap_or("")
            .to_lowercase();

        let https_domain = format!("https://{domain}/");

        if let Some(replacement) = self.maps.get(&https_domain) {
            url.replace(&https_domain, replacement)
        } else {
            url
        }
    }

    /// Returns true when every mapped host responds to a HEAD request
    /// within 3 seconds. Used at startup to decide whether to fetch the
    /// version manifest immediately or rely on the on-disk cache.
    pub async fn is_connected(&self) -> bool {
        let Ok(client) = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(2))
            .build()
        else {
            return false;
        };

        let futures: Vec<_> = self
            .maps
            .values()
            .map(|url| {
                let client = client.clone();
                let url = url.clone();
                async move { client.head(&url).send().await.is_ok() }
            })
            .collect();

        let results = futures_util::future::join_all(futures).await;
        results.into_iter().all(|ok| ok)
    }

    /// Persist this mirror definition to `<mirrors_dir>/<name>.json`.
    pub fn write(&self) -> Result<(), AppError> {
        let content = serde_json::to_string(self)
            .map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
        let path = get_mirrors_dir().join(format!("{}.json", self.name.to_lowercase()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
        }
        fs::write(&path, content).map_err(|e| AppError::FileWriteFailed(e.to_string()))
    }
}

/// Construct a Mirror with the standard Mojang domain mappings.
pub fn mirror(
    name: String,
    description: String,
    launcher_meta: String,
    piston_meta: String,
    piston_data: String,
    resources: String,
    libraries: String,
) -> Mirror {
    let maps = HashMap::from([
        (
            "https://launchermeta.mojang.com/".to_string(),
            launcher_meta,
        ),
        ("https://piston-meta.mojang.com/".to_string(), piston_meta),
        ("https://piston-data.mojang.com/".to_string(), piston_data),
        (
            "https://resources.download.minecraft.net/".to_string(),
            resources,
        ),
        ("https://libraries.minecraft.net/".to_string(), libraries),
    ]);
    Mirror {
        name,
        description,
        maps,
    }
}

pub fn ninecraft_mirror() -> Mirror {
    mirror(
        "9Craft".to_string(),
        "Official 9Craft Mirror".to_string(),
        "https://launchermeta.9craft.ir/".to_string(),
        "https://piston-meta.9craft.ir/".to_string(),
        "https://piston-data.9craft.ir/".to_string(),
        "https://resources-download.9craft.ir/".to_string(),
        "https://libraries-minecraft.9craft.ir/".to_string(),
    )
}

pub fn mojang_mirror() -> Mirror {
    mirror(
        "Official".to_string(),
        "Official mirror for Mojang assets".to_string(),
        "https://launchermeta.mojang.com/".to_string(),
        "https://piston-meta.mojang.com/".to_string(),
        "https://piston-data.mojang.com/".to_string(),
        "https://resources.download.minecraft.net/".to_string(),
        "https://libraries.minecraft.net/".to_string(),
    )
}

/// Enumerate every mirror in `<mirrors_dir>/`, always including the
/// built-in Mojang mirror as a fallback. Errors per-file are logged
/// and skipped so a single corrupt mirror file doesn't hide the rest.
pub fn list_mirrors() -> Result<Vec<Mirror>, AppError> {
    let mirrors_dir = get_mirrors_dir();
    let mut vec = Vec::new();

    let Ok(read_dir) = fs::read_dir(&mirrors_dir) else {
        warn!(
            "mirrors dir {} unreadable, returning Mojang mirror only",
            mirrors_dir.display()
        );
        mojang_mirror().write()?;
        return Ok(vec![mojang_mirror()]);
    };

    for entry in read_dir.flatten() {
        let path = mirrors_dir.join(entry.file_name());
        let Ok(content) = fs::read_to_string(&path) else {
            warn!("skipping unreadable mirror file: {}", path.display());
            continue;
        };
        match serde_json::from_str::<Mirror>(&content) {
            Ok(m) => vec.push(m),
            Err(e) => warn!("skipping invalid mirror file {}: {e}", path.display()),
        }
    }

    if !vec.iter().any(|m| m.name == mojang_mirror().name) {
        vec.push(mojang_mirror());
    }
    Ok(vec)
}

/// Look up a mirror by name, falling back to the Mojang mirror when
/// not found. Never fails — the worst case is "use Mojang directly".
pub fn mirror_from(name: &str) -> Mirror {
    let Ok(mirrors) = list_mirrors() else {
        warn!("failed to list mirrors; falling back to Mojang");
        return mojang_mirror();
    };
    mirrors
        .into_iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| mojang_mirror())
}

/// Pick the mirror actually used for a download run.
///
/// The configured (preferred) mirror is probed first; when its hosts
/// are unreachable — common for Mojang CDNs in regions with filtering —
/// the built-in 9Craft mirror is tried before giving up and returning
/// the preferred mirror unchanged (so any error shown to the user
/// refers to the mirror they actually configured).
pub async fn resolve_effective(preferred: &Mirror) -> Mirror {
    if preferred.is_connected().await {
        return preferred.clone();
    }

    let fallback = ninecraft_mirror();
    if preferred.name == fallback.name {
        return preferred.clone();
    }
    warn!(
        "configured mirror '{}' is unreachable — trying built-in 9Craft fallback",
        preferred.name
    );
    if fallback.is_connected().await {
        info!("using fallback mirror '{}' for this download", fallback.name);
        fallback
    } else {
        warn!("fallback mirror is unreachable too; continuing with '{}'", preferred.name);
        preferred.clone()
    }
}
