use std::fs;
use std::fs::{read_to_string, File};
use std::io::Write;

use log::{info, warn};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::error::AppError;
use crate::services::directory_manager::get_profiles_file;
use crate::services::utils::uuid_from_username;

/// An offline Minecraft profile.
///
/// - `username` is the in-game name shown above the player's head and
///   used for the deterministic offline UUID.
/// - `uuid` is derived from `OfflinePlayer:<username>` and is identical
///   to what vanilla Minecraft servers assign offline players. This
///   means saves and player data stay consistent across launches.
/// - `created_at` is a Unix timestamp (seconds) for sorting/display.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub username: String,
    #[serde(default)]
    pub online: bool,
    pub uuid: Uuid,
    #[serde(default)]
    pub created_at: i64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            username: "Player".to_string(),
            online: false,
            uuid: uuid_from_username("Player"),
            created_at: 0,
        }
    }
}

/// Sanitize a username: trim, collapse whitespace, enforce length and
/// allowed character set (matching Minecraft's offline rules: letters,
/// digits and `_`). Returns the cleaned username or an error.
fn sanitize_username(raw: &str) -> Result<String, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::ProfileInvalid("username is empty".to_string()));
    }
    // Count characters, not bytes: a 16-char Persian name is ~32 bytes
    // in UTF-8 and was wrongly rejected as "too long" by the byte check.
    if trimmed.chars().count() > 16 {
        return Err(AppError::ProfileInvalid(format!(
            "username too long (max 16 chars): {trimmed}"
        )));
    }
    // Unicode-aware: accepts letters/digits from any script (Latin,
    // Persian, Arabic, …) plus `_`. The old predicate tried to allow
    // non-ASCII letters via `is_ascii_alphabetic() && (c as u32) > 127`,
    // which is always false — ASCII chars are by definition < 128 — so
    // every non-English username (e.g. Persian) was rejected.
    let valid = trimmed.chars().all(|c| c.is_alphanumeric() || c == '_');
    if !valid {
        return Err(AppError::ProfileInvalid(format!(
            "username contains illegal characters: {trimmed}"
        )));
    }
    Ok(trimmed.to_string())
}

/// Create a new offline profile and persist it to disk.
///
/// Returns `ProfileInvalid` when the username fails validation or when
/// a profile with the same username already exists.
pub fn create_new_profile(username: String, _online: bool) -> Result<Profile, AppError> {
    let clean = sanitize_username(&username)?;
    let mut profiles = get_profiles();
    let uuid = uuid_from_username(&clean);
    if profiles.iter().any(|p| p.uuid == uuid) {
        return Err(AppError::ProfileInvalid(format!(
            "a profile for username '{clean}' already exists"
        )));
    }
    let profile = Profile {
        username: clean.clone(),
        online: false,
        uuid,
        created_at: chrono::Utc::now().timestamp(),
    };
    profiles.push(profile.clone());
    persist(&profiles)?;
    info!("Created offline profile: {} ({})", profile.username, profile.uuid);
    Ok(profile)
}

/// Rename an existing profile (identified by UUID). The UUID is
/// re-derived from the new username so it stays consistent with
/// Minecraft's offline convention. Returns the updated profile.
pub fn rename_profile(uuid: Uuid, new_username: String) -> Result<Profile, AppError> {
    let clean = sanitize_username(&new_username)?;
    let mut profiles = get_profiles();
    let target_uuid = uuid_from_username(&clean);
    // If the new name maps to an existing different profile, refuse.
    if target_uuid != uuid && profiles.iter().any(|p| p.uuid == target_uuid) {
        return Err(AppError::ProfileInvalid(format!(
            "a profile for username '{clean}' already exists"
        )));
    }
    let Some(profile) = profiles.iter_mut().find(|p| p.uuid == uuid) else {
        return Err(AppError::ProfileNotFound(uuid.to_string()));
    };
    profile.username = clean.clone();
    profile.uuid = target_uuid;
    let updated = profile.clone();
    persist(&profiles)?;
    info!("Renamed profile to: {} ({})", updated.username, updated.uuid);
    Ok(updated)
}

/// Persist the profile list to disk. Atomic: writes to a `.tmp` file
/// then renames over the final path so a crash mid-write never leaves a
/// truncated profiles.json.
fn persist(profiles: &[Profile]) -> Result<(), AppError> {
    let path = get_profiles_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(profiles)
        .map_err(|e| AppError::JsonParseFailed(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = File::create(&tmp).map_err(|e| AppError::FileCreateFailed(e.to_string()))?;
        f.write_all(json.as_bytes())
            .map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
        f.sync_all()
            .map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
    }
    fs::rename(&tmp, &path).map_err(|e| AppError::FileRenameFailed(e.to_string()))?;
    Ok(())
}

/// Load all profiles from disk. Recovers from missing/corrupt files by
/// returning an empty list rather than panicking — the launcher can then
/// prompt the user to create a profile.
pub fn get_profiles() -> Vec<Profile> {
    let path = get_profiles_file();
    if !path.exists() {
        return Vec::new();
    }
    let Ok(text) = read_to_string(&path) else {
        warn!("profiles.json unreadable at {}, treating as empty", path.display());
        return Vec::new();
    };
    let Ok(profiles) = serde_json::from_str::<Vec<Profile>>(&text) else {
        warn!("profiles.json corrupt at {}, treating as empty", path.display());
        return Vec::new();
    };
    // Filter out entries with empty usernames (data-integrity safety net).
    profiles.into_iter().filter(|p| !p.username.trim().is_empty()).collect()
}

/// Find a profile by UUID. `None` if not found.
pub fn get_profile(uuid: &Uuid) -> Option<Profile> {
    get_profiles().into_iter().find(|p| &p.uuid == uuid)
}

/// Delete a profile by UUID. Returns `ProfileNotFound` if it doesn't exist.
pub fn delete_profile(uuid: Uuid) -> Result<(), AppError> {
    let mut profiles = get_profiles();
    let before = profiles.len();
    profiles.retain(|p| p.uuid != uuid);
    if profiles.len() == before {
        return Err(AppError::ProfileNotFound(uuid.to_string()));
    }
    persist(&profiles)?;
    info!("Deleted profile: {}", uuid);
    Ok(())
}

/// Pick a sensible default profile when the selected one is missing.
/// Prefers the most recently created profile; returns `None` when the
/// user has no profiles at all (UI should show the create-profile dialog).
pub fn fallback_profile() -> Option<Profile> {
    let mut profiles = get_profiles();
    profiles.sort_by_key(|p| p.created_at);
    profiles.into_iter().last()
}

/// Ensure `selected` is valid; if not, return a fallback. This is the
/// single chokepoint the launcher uses to recover from a deleted or
/// corrupt selected-profile pointer.
pub fn ensure_selected(selected: Option<Uuid>) -> Option<Profile> {
    if let Some(uuid) = selected {
        if let Some(p) = get_profile(&uuid) {
            return Some(p);
        }
        warn!("selected profile {} not found, falling back", uuid);
    }
    fallback_profile()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_empty_username() {
        assert!(sanitize_username("").is_err());
        assert!(sanitize_username("   ").is_err());
    }

    #[test]
    fn sanitize_rejects_too_long_username() {
        let long = "a".repeat(17);
        assert!(sanitize_username(&long).is_err());
    }

    #[test]
    fn sanitize_accepts_normal_username() {
        assert_eq!(sanitize_username("Player").unwrap(), "Player");
        assert_eq!(sanitize_username("Player_01").unwrap(), "Player_01");
    }

    #[test]
    fn sanitize_trims_whitespace() {
        assert_eq!(sanitize_username("  Glitchy  ").unwrap(), "Glitchy");
    }
}
