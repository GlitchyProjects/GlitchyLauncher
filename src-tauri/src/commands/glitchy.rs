//! Tauri commands for the Glitchy user-data store.

use std::fs;
use std::path::PathBuf;

use tauri::{command, AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::models::error::AppError;
use crate::services::glitchy_store::{
    achievement_catalog, badge_catalog, collect_eval_ctx, evaluate_achievements, evaluate_badges,
    journey_achievement_unlocked, journey_badge_earned, journey_first_launch,
    journey_milestone, journey_play_session, journey_profile_customized, load_journey,
    load_profile, recompute_statistics, save_journey, save_profile, ALLOWED_ACCENTS,
    AchievementDef, AchievementState, ActivityEntry, Avatar, BadgeDef, GlitchyProfile,
    JourneyStore, ProfileCustomization, Statistics,
};
use crate::services::path_safety::sanitize_user_path_segment;

pub(crate) fn refresh_and_emit(
    app: &AppHandle,
    profile: &mut GlitchyProfile,
    journey: &mut JourneyStore,
) -> Result<(), AppError> {
    recompute_statistics(profile, journey);
    let ctx = collect_eval_ctx(journey);
    let new_achievements = evaluate_achievements(profile, &ctx);
    let new_badges = evaluate_badges(profile, &ctx);
    for id in &new_achievements {
        if let Some(def) = achievement_catalog().into_iter().find(|d| &d.id == id) {
            journey.push(journey_achievement_unlocked(&def.title));
        }
    }
    for id in &new_badges {
        if let Some(def) = badge_catalog().into_iter().find(|d| &d.id == id) {
            journey.push(journey_badge_earned(&def.title));
        }
    }
    save_profile(profile)?;
    save_journey(journey)?;
    let _ = app.emit("glitchy-update", &());
    for id in &new_achievements {
        if let Some(def) = achievement_catalog().into_iter().find(|d| &d.id == id) {
            let _ = app.notification().builder()
                .title("Achievement Unlocked")
                .body(&format!("{} — {}", def.title, def.description))
                .show();
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementWithState {
    #[serde(flatten)] pub def: AchievementDef,
    // Flattened so the frontend can read `unlocked` / `progress` /
    // `unlockedAt` as top-level fields (matches the TS interface).
    // The previous nested `state: {...}` shape made every achievement
    // read as locked in the UI even after unlocking.
    #[serde(flatten)] pub state: AchievementState,
}

#[command]
pub async fn glitchy_get_achievements() -> Result<Vec<AchievementWithState>, AppError> {
    let mut profile = load_profile();
    let journey = load_journey();
    recompute_statistics(&mut profile, &journey);
    let catalog = achievement_catalog();
    Ok(catalog.into_iter().map(|def| {
        let state = profile.achievements.states.get(&def.id).cloned().unwrap_or_default();
        AchievementWithState { def, state }
    }).collect())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeWithState {
    #[serde(flatten)] pub def: BadgeDef,
    pub earned: bool,
    pub displayed: bool,
}

#[command]
pub async fn glitchy_get_badges() -> Result<Vec<BadgeWithState>, AppError> {
    let profile = load_profile();
    let catalog = badge_catalog();
    Ok(catalog.into_iter().map(|def| {
        let earned = profile.badges.earned.contains(&def.id);
        let displayed = profile.badges.displayed.contains(&def.id);
        BadgeWithState { def, earned, displayed }
    }).collect())
}

#[command]
pub async fn glitchy_set_displayed_badges(app: AppHandle, badge_ids: Vec<String>) -> Result<(), AppError> {
    let catalog = badge_catalog();
    let mut profile = load_profile();
    // A badge can only be displayed if it is actually earned. The
    // previous implementation accepted any catalog id, which let users
    // equip locked badges. Now we validate server-side: an id must be
    // in the catalog AND in the user's earned list. Badges that are
    // not auto-awarded (e.g. "glitchy") are treated as always-earned
    // because they're freely equippable cosmetics — but only those.
    let valid_ids: Vec<String> = badge_ids.into_iter()
        .filter(|id| {
            if let Some(def) = catalog.iter().find(|d| &d.id == id) {
                if !def.auto_awarded {
                    // Free cosmetic — always allowed.
                    true
                } else {
                    // Auto-awarded — must actually be in earned list.
                    profile.badges.earned.contains(id)
                }
            } else {
                false
            }
        })
        .take(6)
        .collect();
    let mut journey = load_journey();
    profile.badges.displayed = valid_ids;
    refresh_and_emit(&app, &mut profile, &mut journey)?;
    Ok(())
}

#[command]
pub async fn glitchy_get_profile() -> Result<GlitchyProfile, AppError> {
    let mut profile = load_profile();
    let journey = load_journey();
    recompute_statistics(&mut profile, &journey);
    Ok(profile)
}

#[command]
pub async fn glitchy_save_customization(app: AppHandle, customization: ProfileCustomization) -> Result<(), AppError> {
    if !ALLOWED_ACCENTS.contains(&customization.accent.0.as_str()) {
        return Err(AppError::PathValidationFailed(format!("accent color '{}' is not in the allowed Glitchy palette", customization.accent.0)));
    }
    let mut tagline = customization.tagline.trim().to_string();
    if tagline.chars().count() > 80 { tagline = tagline.chars().take(80).collect(); }
    if let Avatar::Custom { ref filename } = customization.avatar {
        let _ = sanitize_user_path_segment(&crate::services::glitchy_store::avatars_dir(), filename)?;
    }
    let mut profile = load_profile();
    let mut journey = load_journey();
    profile.customization = ProfileCustomization { tagline, ..customization };
    journey.push(journey_profile_customized());
    refresh_and_emit(&app, &mut profile, &mut journey)?;
    Ok(())
}

#[command]
pub async fn glitchy_upload_avatar(app: AppHandle, source_path: String) -> Result<String, AppError> {
    let src = PathBuf::from(&source_path);
    if !src.exists() { return Err(AppError::FileNotFound(source_path)); }
    crate::services::glitchy_store::ensure_dirs();
    let allowed = ["png", "jpg", "jpeg", "webp", "gif"];
    let ext = src
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !allowed.contains(&ext.as_str()) {
        return Err(AppError::PathValidationFailed(format!("avatar file type '{ext}' not allowed; use one of: {allowed:?}")));
    }
    let generated_name = format!("avatar-{}.{}", uuid::Uuid::new_v4(), ext);
    let dest = sanitize_user_path_segment(
        &crate::services::glitchy_store::avatars_dir(),
        &generated_name,
    )?;
    let safe_name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::PathValidationFailed("avatar filename is not UTF-8".to_string()))?
        .to_string();
    fs::copy(&src, &dest).map_err(|e| AppError::FileCopyFailed(e.to_string()))?;
    let mut profile = load_profile();
    let mut journey = load_journey();
    profile.customization.avatar = Avatar::Custom { filename: safe_name.clone() };
    refresh_and_emit(&app, &mut profile, &mut journey)?;
    Ok(safe_name)
}

#[command]
pub async fn glitchy_avatar_path(filename: String) -> Result<String, AppError> {
    let path = sanitize_user_path_segment(&crate::services::glitchy_store::avatars_dir(), &filename)?;
    if !path.exists() { return Err(AppError::FileNotFound(filename)); }
    Ok(path.to_string_lossy().to_string())
}

/// Return a custom avatar as a `data:` URL (base64). Used instead of
/// the asset protocol so the binary needs no extra Tauri features.
#[command]
pub fn glitchy_avatar_data(filename: String) -> Result<String, AppError> {
    let path = sanitize_user_path_segment(&crate::services::glitchy_store::avatars_dir(), &filename)?;
    if !path.exists() { return Err(AppError::FileNotFound(filename)); }
    let bytes = fs::read(&path).map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

#[command]
pub async fn glitchy_get_allowed_accents() -> Result<Vec<String>, AppError> {
    Ok(ALLOWED_ACCENTS.iter().map(|s| s.to_string()).collect())
}

#[command]
pub async fn glitchy_get_journey() -> Result<JourneyStore, AppError> {
    Ok(load_journey())
}

#[command]
pub async fn glitchy_get_statistics() -> Result<Statistics, AppError> {
    let mut profile = load_profile();
    let journey = load_journey();
    recompute_statistics(&mut profile, &journey);
    Ok(profile.statistics)
}

#[command]
pub async fn glitchy_get_recent_activity(limit: Option<u32>) -> Result<Vec<ActivityEntry>, AppError> {
    let journey = load_journey();
    let cap = limit.unwrap_or(20).min(100) as usize;
    Ok(journey.events.iter().take(cap).map(|e| ActivityEntry {
        id: e.id.clone(), timestamp: e.timestamp, kind: e.kind.clone(),
        title: e.title.clone(), icon: e.icon.clone(),
    }).collect())
}

pub fn on_play_session(app: &AppHandle, version: &str) {
    let mut profile = load_profile();
    let mut journey = load_journey();
    if journey.events.iter().all(|e| e.kind != "first_launch") {
        journey.push(journey_first_launch());
    }
    profile.statistics.total_sessions += 1;
    journey.push(journey_play_session(version, 0));
    let _ = refresh_and_emit(app, &mut profile, &mut journey);
}

pub fn on_play_session_end(app: &AppHandle, version: &str, duration_seconds: u64) {
    let mut profile = load_profile();
    let mut journey = load_journey();
    profile.statistics.total_playtime_seconds = profile.statistics.total_playtime_seconds.saturating_add(duration_seconds);
    if let Some(ev) = journey.events.iter_mut().find(|e| e.kind == "play_session" && e.title == format!("Played {version}")) {
        ev.description = format!("Session duration: {}m", duration_seconds / 60);
    }
    let local_hour = chrono::Local::now().format("%H").to_string().parse::<u32>().unwrap_or(12);
    if local_hour >= 22 || local_hour < 4 {
        journey.push(crate::services::glitchy_store::JourneyEvent {
            id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(),
            kind: "night_play".into(), title: "Night Play".into(),
            description: format!("+{}s", duration_seconds), icon: "Moon02Icon".into(),
        });
    }
    if profile.statistics.total_playtime_seconds >= 36000 && profile.statistics.total_playtime_seconds < 36000 + duration_seconds {
        journey.push(journey_milestone("10 hours of playtime"));
    }
    if profile.statistics.total_playtime_seconds >= 360000 && profile.statistics.total_playtime_seconds < 360000 + duration_seconds {
        journey.push(journey_milestone("100 hours of playtime"));
    }
    let _ = refresh_and_emit(app, &mut profile, &mut journey);
}

#[command]
pub async fn glitchy_is_maximized(app: AppHandle) -> Result<bool, AppError> {
    let window = app.get_webview_window("main").ok_or_else(|| AppError::UnknownError("main window not found".to_string()))?;
    window
        .is_maximized()
        .map_err(|error| AppError::UnknownError(format!("is_maximized failed: {error}")))
}

#[command]
pub async fn glitchy_toggle_maximized(app: AppHandle) -> Result<bool, AppError> {
    let window = app.get_webview_window("main").ok_or_else(|| AppError::UnknownError("main window not found".to_string()))?;
    let currently_maximized = window
        .is_maximized()
        .map_err(|error| AppError::UnknownError(format!("is_maximized failed: {error}")))?;
    if currently_maximized {
        window
            .unmaximize()
            .map_err(|error| AppError::UnknownError(format!("unmaximize failed: {error}")))?;
    } else {
        window
            .maximize()
            .map_err(|error| AppError::UnknownError(format!("maximize failed: {error}")))?;
    }
    Ok(!currently_maximized)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GlitchySkinConfig {
    pub skin_url_or_data: String,
    pub model: String, // "default" | "slim"
    pub cape_url: Option<String>,
}

#[command]
pub async fn glitchy_set_active_skin(
    skin_data: String,
    model: String,
    cape_url: Option<String>,
) -> Result<(), AppError> {
    let skins_dir = crate::services::directory_manager::get_falcon_launcher_directory().join("skins");
    fs::create_dir_all(&skins_dir)
        .map_err(|e| AppError::UnknownError(format!("Failed to create skins directory: {e}")))?;

    // If skin_data is a base64 Data URL (data:image/png;base64,...), decode and write directly
    let png_bytes = if let Some(stripped) = skin_data.strip_prefix("data:image/png;base64,") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(stripped)
            .map_err(|e| AppError::UnknownError(format!("Base64 decode failed: {e}")))?
    } else if skin_data.starts_with("http://") || skin_data.starts_with("https://") {
        let client = reqwest::Client::new();
        let resp = client.get(&skin_data).send().await
            .map_err(|e| AppError::UnknownError(format!("Failed to download skin: {e}")))?;
        resp.bytes().await
            .map_err(|e| AppError::UnknownError(format!("Failed to read skin bytes: {e}")))?
            .to_vec()
    } else {
        return Err(AppError::UnknownError("Invalid skin data format".to_string()));
    };

    let skin_file = skins_dir.join("active_skin.png");
    fs::write(&skin_file, &png_bytes)
        .map_err(|e| AppError::UnknownError(format!("Failed to write active_skin.png: {e}")))?;

    let config = GlitchySkinConfig {
        skin_url_or_data: skin_data,
        model,
        cape_url,
    };
    let json = serde_json::to_string_pretty(&config)
        .map_err(|e| AppError::UnknownError(format!("Failed to serialize skin config: {e}")))?;
    fs::write(skins_dir.join("active_skin.json"), json)
        .map_err(|e| AppError::UnknownError(format!("Failed to write active_skin.json: {e}")))?;

    Ok(())
}

#[command]
pub async fn glitchy_get_active_skin() -> Result<Option<GlitchySkinConfig>, AppError> {
    let skins_dir = crate::services::directory_manager::get_falcon_launcher_directory().join("skins");
    let json_file = skins_dir.join("active_skin.json");
    if !json_file.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&json_file)
        .map_err(|e| AppError::UnknownError(format!("Failed to read active_skin.json: {e}")))?;
    let parsed: GlitchySkinConfig = serde_json::from_str(&data)
        .map_err(|e| AppError::UnknownError(format!("Failed to parse active_skin.json: {e}")))?;
    Ok(Some(parsed))
}
