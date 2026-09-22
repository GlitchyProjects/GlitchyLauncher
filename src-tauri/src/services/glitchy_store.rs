//! Persistent Glitchy user data: achievements, badges, profile
//! customization, journey timeline, statistics, recent activity.
//!
//! All data is stored as JSON under `<mc>/falconlauncher/glitchy/` so it
//! survives launcher restarts. Atomic writes (tmp + rename) prevent
//! corruption on crash.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Datelike};
use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::models::error::AppError;
use crate::services::directory_manager::{get_falcon_launcher_directory, get_instances_directory, get_versions_directory};
use crate::services::path_safety::sanitize_user_path_segment;

const DATA_DIR_NAME: &str = "glitchy";

pub fn data_dir() -> PathBuf { get_falcon_launcher_directory().join(DATA_DIR_NAME) }
pub fn avatars_dir() -> PathBuf { data_dir().join("avatars") }
pub fn backgrounds_dir() -> PathBuf { data_dir().join("backgrounds") }

pub fn ensure_dirs() {
    let _ = fs::create_dir_all(data_dir());
    let _ = fs::create_dir_all(avatars_dir());
    let _ = fs::create_dir_all(backgrounds_dir());
}

pub fn persist_json<T: Serialize>(path: &Path, value: &T) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| AppError::JsonParseFailed(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
    fs::rename(&tmp, path).map_err(|e| AppError::FileRenameFailed(e.to_string()))?;
    Ok(())
}

pub fn load_json_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    if !path.exists() { return T::default(); }
    let Ok(text) = fs::read_to_string(path) else {
        warn!("unreadable data file at {}, using default", path.display());
        return T::default();
    };
    match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warn!("corrupt data file at {} ({e}), using default — backing up", path.display());
            let _ = fs::rename(path, path.with_extension("json.bak"));
            T::default()
        }
    }
}

pub fn safe_asset_path(base: &Path, name: &str) -> Result<PathBuf, AppError> {
    sanitize_user_path_segment(base, name)
}

// ──────────────────────────────────────────────────────────────────
// DATA MODELS
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AchievementRarity { Common, Rare, Epic, Legendary }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub rarity: AchievementRarity,
    pub condition: String,
    pub target: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AchievementState {
    pub unlocked: bool,
    pub progress: u64,
    pub unlocked_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AchievementsStore {
    pub states: HashMap<String, AchievementState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeDef {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub auto_awarded: bool,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BadgesStore {
    pub earned: Vec<String>,
    pub displayed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Avatar {
    Preset { id: String },
    Custom { filename: String },
}

impl Default for Avatar {
    fn default() -> Self { Avatar::Preset { id: "glitchy".to_string() } }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileFrame { None, SolidBlue, GradientElectric, GradientIcy, NeonPulse }
impl Default for ProfileFrame { fn default() -> Self { ProfileFrame::GradientElectric } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileBackground { Aurora, DeepSea, Grid, Particles, Solid }
impl Default for ProfileBackground { fn default() -> Self { ProfileBackground::Aurora } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GlowStyle { None, Subtle, Normal, Strong }
impl Default for GlowStyle { fn default() -> Self { GlowStyle::Normal } }

/// Accent color is a plain hex string (e.g. "#2484EC"). We use a
/// newtype wrapper so we can enforce the allowed-palette validation in
/// one place. It serializes as a plain JSON string — NOT as an object
/// or array — so the TypeScript contract is `type AccentColor = string`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccentColor(pub String);
impl Default for AccentColor { fn default() -> Self { AccentColor("#2484EC".to_string()) } }

pub const ALLOWED_ACCENTS: &[&str] = &[
    "#2484EC", "#247CD4", "#4C8CCC", "#94BCE4", "#B4D4F4", "#ACC4DC",
];

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileCustomization {
    pub avatar: Avatar,
    pub frame: ProfileFrame,
    pub background: ProfileBackground,
    pub accent: AccentColor,
    pub glow: GlowStyle,
    #[serde(default)]
    pub tagline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyEvent {
    pub id: String,
    pub timestamp: i64,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JourneyStore {
    pub events: Vec<JourneyEvent>,
}

impl JourneyStore {
    pub const MAX_EVENTS: usize = 500;
    pub fn push(&mut self, event: JourneyEvent) {
        const DUPLICATE_WINDOW_SECONDS: u64 = 5;
        let is_recent_duplicate = self.events.iter().take(8).any(|existing| {
            existing.kind == event.kind
                && existing.title == event.title
                && existing.description == event.description
                && existing.timestamp.abs_diff(event.timestamp) <= DUPLICATE_WINDOW_SECONDS
        });
        if is_recent_duplicate {
            return;
        }
        self.events.insert(0, event);
        if self.events.len() > Self::MAX_EVENTS { self.events.truncate(Self::MAX_EVENTS); }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub total_playtime_seconds: u64,
    pub total_sessions: u64,
    pub favorite_version: Option<String>,
    pub most_used_instance: Option<String>,
    pub last_played_at: Option<i64>,
    pub first_launch_at: Option<i64>,
    pub achievements_unlocked: u32,
    pub badges_earned: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: String,
    pub timestamp: i64,
    pub kind: String,
    pub title: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlitchyProfile {
    pub customization: ProfileCustomization,
    pub achievements: AchievementsStore,
    pub badges: BadgesStore,
    pub statistics: Statistics,
}

pub fn profile_path() -> PathBuf { data_dir().join("profile.json") }
pub fn journey_path() -> PathBuf { data_dir().join("journey.json") }

pub fn load_profile() -> GlitchyProfile {
    ensure_dirs();
    load_json_or_default(&profile_path())
}

pub fn save_profile(profile: &GlitchyProfile) -> Result<(), AppError> {
    persist_json(&profile_path(), profile)
}

pub fn load_journey() -> JourneyStore {
    ensure_dirs();
    load_json_or_default(&journey_path())
}

pub fn save_journey(journey: &JourneyStore) -> Result<(), AppError> {
    persist_json(&journey_path(), journey)
}

// ──────────────────────────────────────────────────────────────────
// CATALOGS
// ──────────────────────────────────────────────────────────────────

pub fn achievement_catalog() -> Vec<AchievementDef> {
    vec![
        AchievementDef { id: "first_launch".into(), title: "First Launch".into(), description: "Launch Minecraft through Glitchy for the first time.".into(), icon: "Rocket01Icon".into(), rarity: AchievementRarity::Common, condition: "sessions>=1".into(), target: Some(1) },
        AchievementDef { id: "speedrunner".into(), title: "Speedrunner".into(), description: "Launch Minecraft 10 times.".into(), icon: "FlashIcon".into(), rarity: AchievementRarity::Rare, condition: "sessions>=10".into(), target: Some(10) },
        AchievementDef { id: "collector".into(), title: "Collector".into(), description: "Create 5 launcher instances.".into(), icon: "Package01Icon".into(), rarity: AchievementRarity::Rare, condition: "instances>=5".into(), target: Some(5) },
        AchievementDef { id: "night_player".into(), title: "Night Player".into(), description: "Play Minecraft for 1 hour between 22:00 and 04:00.".into(), icon: "Moon02Icon".into(), rarity: AchievementRarity::Epic, condition: "night_seconds>=3600".into(), target: Some(3600) },
        AchievementDef { id: "veteran".into(), title: "Veteran".into(), description: "Reach 100 hours of total playtime.".into(), icon: "Trophy01Icon".into(), rarity: AchievementRarity::Legendary, condition: "playtime_seconds>=360000".into(), target: Some(360000) },
        AchievementDef { id: "ten_hours".into(), title: "Getting Started".into(), description: "Accumulate 10 hours of playtime.".into(), icon: "Clock01Icon".into(), rarity: AchievementRarity::Common, condition: "playtime_seconds>=36000".into(), target: Some(36000) },
        AchievementDef { id: "profile_curator".into(), title: "Profile Curator".into(), description: "Customize your profile for the first time.".into(), icon: "UserEditIcon".into(), rarity: AchievementRarity::Common, condition: "customized>=1".into(), target: Some(1) },
        AchievementDef { id: "badge_collector".into(), title: "Badge Collector".into(), description: "Display 3 badges on your profile.".into(), icon: "Medal01Icon".into(), rarity: AchievementRarity::Rare, condition: "displayed_badges>=3".into(), target: Some(3) },
        // ── 1.0.0 catalog expansion ─────────────────────────────────
        AchievementDef { id: "frequent_flyer".into(), title: "Frequent Flyer".into(), description: "Launch Minecraft 25 times.".into(), icon: "Time04Icon".into(), rarity: AchievementRarity::Rare, condition: "sessions>=25".into(), target: Some(25) },
        AchievementDef { id: "century_club".into(), title: "Century Club".into(), description: "Launch Minecraft 100 times. A true regular.".into(), icon: "ChartUpIcon".into(), rarity: AchievementRarity::Epic, condition: "sessions>=100".into(), target: Some(100) },
        AchievementDef { id: "half_century".into(), title: "Half Century".into(), description: "Accumulate 50 hours of playtime.".into(), icon: "Time01Icon".into(), rarity: AchievementRarity::Rare, condition: "playtime_seconds>=180000".into(), target: Some(180000) },
        AchievementDef { id: "devoted".into(), title: "Devoted".into(), description: "Reach 250 hours of total playtime. The blocks never forget.".into(), icon: "FireIcon".into(), rarity: AchievementRarity::Legendary, condition: "playtime_seconds>=900000".into(), target: Some(900000) },
        AchievementDef { id: "version_explorer".into(), title: "Version Explorer".into(), description: "Play 3 different Minecraft versions.".into(), icon: "Compass01Icon".into(), rarity: AchievementRarity::Common, condition: "distinct_versions>=3".into(), target: Some(3) },
        AchievementDef { id: "multiverse_traveler".into(), title: "Multiverse Traveler".into(), description: "Play 5 different Minecraft versions across the ages.".into(), icon: "Globe02Icon".into(), rarity: AchievementRarity::Rare, condition: "distinct_versions>=5".into(), target: Some(5) },
        AchievementDef { id: "mod_squad".into(), title: "Mod Squad".into(), description: "Install 10 mods in your mods folder.".into(), icon: "RubiksCubeIcon".into(), rarity: AchievementRarity::Rare, condition: "mods>=10".into(), target: Some(10) },
        AchievementDef { id: "mod_library".into(), title: "Mod Library".into(), description: "Install 25 mods. Your loadout is a library now.".into(), icon: "SafeBoxIcon".into(), rarity: AchievementRarity::Epic, condition: "mods>=25".into(), target: Some(25) },
        AchievementDef { id: "master_architect".into(), title: "Master Architect".into(), description: "Create 10 launcher instances.".into(), icon: "Castle01Icon".into(), rarity: AchievementRarity::Epic, condition: "instances>=10".into(), target: Some(10) },
        AchievementDef { id: "marathon_session".into(), title: "Marathon Session".into(), description: "Play a single session lasting 3 hours.".into(), icon: "Timer01Icon".into(), rarity: AchievementRarity::Rare, condition: "longest_session_seconds>=10800".into(), target: Some(10800) },
        AchievementDef { id: "ultra_marathon".into(), title: "Ultra Marathon".into(), description: "Play a single session lasting 6 hours. Sunglasses recommended.".into(), icon: "SunglassesIcon".into(), rarity: AchievementRarity::Legendary, condition: "longest_session_seconds>=21600".into(), target: Some(21600) },
        AchievementDef { id: "night_shift".into(), title: "Night Shift".into(), description: "Play for 10 hours between 22:00 and 04:00.".into(), icon: "AlarmClockIcon".into(), rarity: AchievementRarity::Epic, condition: "night_seconds>=36000".into(), target: Some(36000) },
        AchievementDef { id: "badge_showcase".into(), title: "Badge Showcase".into(), description: "Fill all 6 badge slots on your profile.".into(), icon: "FireworksIcon".into(), rarity: AchievementRarity::Epic, condition: "displayed_badges>=6".into(), target: Some(6) },
        AchievementDef { id: "fresh_look".into(), title: "Fresh Look".into(), description: "Upload your own custom profile picture.".into(), icon: "CameraAdd01Icon".into(), rarity: AchievementRarity::Common, condition: "custom_avatar>=1".into(), target: Some(1) },
        AchievementDef { id: "the_regular".into(), title: "The Regular".into(), description: "Play Minecraft on 7 different days.".into(), icon: "Calendar01Icon".into(), rarity: AchievementRarity::Rare, condition: "days_active>=7".into(), target: Some(7) },
    ]
}

pub fn badge_catalog() -> Vec<BadgeDef> {
    vec![
        BadgeDef { id: "early_user".into(), title: "Early User".into(), description: "Used Glitchy Launcher during its first release.".into(), icon: "Star01Icon".into(), auto_awarded: true, condition: Some("first_launch".into()) },
        BadgeDef { id: "minecraft_player".into(), title: "Minecraft Player".into(), description: "Launched Minecraft at least once.".into(), icon: "GameboyIcon".into(), auto_awarded: true, condition: Some("sessions>=1".into()) },
        BadgeDef { id: "100_hours".into(), title: "100 Hours".into(), description: "Reached 100 hours of total playtime.".into(), icon: "Clock01Icon".into(), auto_awarded: true, condition: Some("playtime_seconds>=360000".into()) },
        BadgeDef { id: "glitchy".into(), title: "Glitchy".into(), description: "The signature Glitchy badge. Equipped by default.".into(), icon: "Bolt01Icon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "founder".into(), title: "Founder".into(), description: "Supported Glitchy from day one.".into(), icon: "Crown01Icon".into(), auto_awarded: true, condition: Some("first_launch".into()) },
        BadgeDef { id: "night_owl".into(), title: "Night Owl".into(), description: "Played Minecraft after midnight.".into(), icon: "Moon02Icon".into(), auto_awarded: true, condition: Some("night_seconds>=1".into()) },
        BadgeDef { id: "speedrunner".into(), title: "Speedrunner".into(), description: "Launched Minecraft 10 times.".into(), icon: "FlashIcon".into(), auto_awarded: true, condition: Some("sessions>=10".into()) },
        BadgeDef { id: "modder".into(), title: "Modder".into(), description: "Installed at least one mod.".into(), icon: "Puzzle01Icon".into(), auto_awarded: true, condition: Some("mods>=1".into()) },
        // ── 1.0.0 catalog expansion ─────────────────────────────────
        BadgeDef { id: "century".into(), title: "Century Club".into(), description: "Launched Minecraft 100 times.".into(), icon: "Calendar03Icon".into(), auto_awarded: true, condition: Some("sessions>=100".into()) },
        BadgeDef { id: "mod_librarian".into(), title: "Mod Librarian".into(), description: "Installed 25 mods.".into(), icon: "DeliveryBox01Icon".into(), auto_awarded: true, condition: Some("mods>=25".into()) },
        BadgeDef { id: "traveler".into(), title: "Multiverse Traveler".into(), description: "Played 5 different Minecraft versions.".into(), icon: "EarthIcon".into(), auto_awarded: true, condition: Some("distinct_versions>=5".into()) },
        BadgeDef { id: "marathoner".into(), title: "Marathoner".into(), description: "Finished a single 3-hour session.".into(), icon: "Timer01Icon".into(), auto_awarded: true, condition: Some("longest_session_seconds>=10800".into()) },
        BadgeDef { id: "face_reveal".into(), title: "Face Reveal".into(), description: "Uploaded a custom profile picture.".into(), icon: "Camera01Icon".into(), auto_awarded: true, condition: Some("custom_avatar>=1".into()) },
        BadgeDef { id: "regular".into(), title: "The Regular".into(), description: "Played on 7 different days.".into(), icon: "CalendarCheckIn01Icon".into(), auto_awarded: true, condition: Some("days_active>=7".into()) },
        BadgeDef { id: "hours_250".into(), title: "250 Hours".into(), description: "Reached 250 hours of total playtime.".into(), icon: "HourglassIcon".into(), auto_awarded: true, condition: Some("playtime_seconds>=900000".into()) },
        BadgeDef { id: "architect".into(), title: "Architect".into(), description: "Created 10 launcher instances.".into(), icon: "Castle01Icon".into(), auto_awarded: true, condition: Some("instances>=10".into()) },
        BadgeDef { id: "completionist".into(), title: "Completionist".into(), description: "Unlocked 15 achievements.".into(), icon: "Trophy01Icon".into(), auto_awarded: true, condition: Some("achievements>=15".into()) },
        BadgeDef { id: "diamond".into(), title: "Diamond in the Rough".into(), description: "Shine bright, miner. A free cosmetic badge.".into(), icon: "Diamond01Icon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "enderborn".into(), title: "Enderborn".into(), description: "Touched by the End. A free cosmetic badge.".into(), icon: "MagicWand01Icon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "redstone_engineer".into(), title: "Redstone Engineer".into(), description: "Wires, torches and pistons. A free cosmetic badge.".into(), icon: "Flowchart01Icon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "creeper_whisperer".into(), title: "Creeper Whisperer".into(), description: "That hissing sound? Just a compliment. A free cosmetic badge.".into(), icon: "Fire02Icon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "dragon_rider".into(), title: "Dragon Rider".into(), description: "First one through the End gateway. A free cosmetic badge.".into(), icon: "EggIcon".into(), auto_awarded: false, condition: None },
        BadgeDef { id: "champion".into(), title: "Champion".into(), description: "For the fighters of blocky arenas. A free cosmetic badge.".into(), icon: "Sword01Icon".into(), auto_awarded: false, condition: None },
    ]
}

/// Live metrics that cannot be derived from the profile itself. These
/// used to be hardcoded to 0, which made the "Modder" badge, the
/// "Collector" achievement and night-play tracking impossible to earn.
#[derive(Debug, Clone, Copy, Default)]
pub struct EvalCtx {
    /// Mod files currently in the mods folder (jar / jar.disabled).
    pub mods: u64,
    /// Installed version folders (instances).
    pub instances: u64,
    /// Seconds played at night, summed from `night_play` journey events.
    pub night_seconds: u64,
    /// Distinct versions appearing in `play_session` journey events.
    pub distinct_versions: u64,
    /// Longest single session in seconds (from `play_session` durations).
    pub longest_session_seconds: u64,
    /// Distinct local calendar days with at least one `play_session`.
    pub days_active: u64,
    /// 1 when the user has uploaded a custom avatar file, else 0.
    pub custom_avatar: u64,
}

pub fn collect_eval_ctx(journey: &JourneyStore) -> EvalCtx {
    // Mods live in each instance's own folder (instances/<id>/mods), so
    // the total is summed across every instance directory.
    let mods = std::fs::read_dir(get_instances_directory())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| std::fs::read_dir(e.path().join("mods")).ok())
                .flat_map(|rd| rd.flatten())
                .filter(|e| e.path().is_file())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    name.ends_with(".jar") || name.ends_with(".jar.disabled")
                })
                .count() as u64
        })
        .unwrap_or(0);

    let instances = std::fs::read_dir(get_versions_directory())
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).count() as u64)
        .unwrap_or(0);

    // Preset avatars ship with the app; only user uploads land in the
    // avatars dir, so any file there counts as "has a custom picture".
    let custom_avatar = fs::read_dir(avatars_dir())
        .map(|rd| rd.flatten().any(|e| e.path().is_file()) as u64)
        .unwrap_or(0);

    let night_seconds = journey
        .events
        .iter()
        .filter(|e| e.kind == "night_play")
        .filter_map(|e| e.description.strip_prefix('+'))
        .filter_map(|s| s.strip_suffix('s'))
        .filter_map(|s| s.parse::<u64>().ok())
        .sum();

    let play_sessions = journey.events.iter().filter(|e| e.kind == "play_session");

    let distinct_versions = play_sessions
        .clone()
        .filter_map(|e| e.title.strip_prefix("Played "))
        .collect::<std::collections::HashSet<&str>>()
        .len() as u64;

    let longest_session_seconds = play_sessions
        .clone()
        .filter_map(|e| e.description.strip_prefix("Session duration: "))
        .filter_map(|s| s.strip_suffix('m'))
        .filter_map(|s| s.parse::<u64>().ok())
        .map(|m| m * 60)
        .max()
        .unwrap_or(0);

    let days_active = play_sessions
        .filter_map(|e| chrono::Local.timestamp_opt(e.timestamp, 1).single())
        .map(|dt| dt.date_naive())
        .collect::<std::collections::HashSet<chrono::NaiveDate>>()
        .len() as u64;

    EvalCtx { mods, instances, night_seconds, distinct_versions, longest_session_seconds, days_active, custom_avatar }
}

pub fn evaluate_achievements(profile: &mut GlitchyProfile, ctx: &EvalCtx) -> Vec<String> {
    let catalog = achievement_catalog();
    let mut newly_unlocked = Vec::new();
    for def in &catalog {
        let state = profile.achievements.states.entry(def.id.clone()).or_default();
        if state.unlocked { continue; }
        let (met, progress) = evaluate_condition(&def.condition, &profile.statistics, &profile.badges, ctx);
        state.progress = progress;
        if met {
            state.unlocked = true;
            state.unlocked_at = Some(chrono::Utc::now().timestamp());
            newly_unlocked.push(def.id.clone());
            info!("Achievement unlocked: {} ({})", def.title, def.id);
        }
    }
    if !newly_unlocked.is_empty() {
        profile.statistics.achievements_unlocked = profile.achievements.states.values().filter(|s| s.unlocked).count() as u32;
    }
    newly_unlocked
}

pub fn evaluate_badges(profile: &mut GlitchyProfile, ctx: &EvalCtx) -> Vec<String> {
    let catalog = badge_catalog();
    let mut newly_earned = Vec::new();
    for def in &catalog {
        if !def.auto_awarded { continue; }
        if profile.badges.earned.contains(&def.id) { continue; }
        let Some(condition) = &def.condition else { continue; };
        let (met, _) = evaluate_condition(condition, &profile.statistics, &profile.badges, ctx);
        if met {
            profile.badges.earned.push(def.id.clone());
            newly_earned.push(def.id.clone());
            info!("Badge earned: {} ({})", def.title, def.id);
        }
    }
    if !newly_earned.is_empty() {
        profile.statistics.badges_earned = profile.badges.earned.len() as u32;
    }
    newly_earned
}

fn evaluate_condition(condition: &str, stats: &Statistics, badges: &BadgesStore, ctx: &EvalCtx) -> (bool, u64) {
    let (metric, op, value) = parse_condition(condition);
    let current = match metric.as_str() {
        "sessions" => stats.total_sessions,
        // Live metrics from the filesystem / journey (EvalCtx).
        "instances" => ctx.instances,
        "playtime_seconds" => stats.total_playtime_seconds,
        "night_seconds" => ctx.night_seconds,
        "customized" => if stats.first_launch_at.is_some() { 1 } else { 0 },
        "displayed_badges" => badges.displayed.len() as u64,
        "mods" => ctx.mods,
        "first_launch" => if stats.first_launch_at.is_some() { 1 } else { 0 },
        // 1.0.0 metrics
        "distinct_versions" => ctx.distinct_versions,
        "longest_session_seconds" => ctx.longest_session_seconds,
        "days_active" => ctx.days_active,
        "custom_avatar" => ctx.custom_avatar,
        "achievements" => stats.achievements_unlocked as u64,
        _ => 0,
    };
    let met = if op.is_empty() { current > 0 } else {
        match op.as_str() {
            ">=" => current >= value,
            "==" => current == value,
            _ => false,
        }
    };
    (met, current)
}

fn parse_condition(condition: &str) -> (String, String, u64) {
    if let Some((lhs, rhs)) = condition.split_once(">=") {
        return (lhs.trim().to_string(), ">=".to_string(), rhs.trim().parse().unwrap_or(0));
    }
    if let Some((lhs, rhs)) = condition.split_once("==") {
        return (lhs.trim().to_string(), "==".to_string(), rhs.trim().parse().unwrap_or(0));
    }
    (condition.to_string(), String::new(), 0)
}

// ──────────────────────────────────────────────────────────────────
// JOURNEY EVENT FACTORIES
// ──────────────────────────────────────────────────────────────────

pub fn journey_first_launch() -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "first_launch".into(), title: "Started Glitchy".into(), description: "First launch of Glitchy Launcher.".into(), icon: "Rocket01Icon".into() }
}

pub fn journey_play_session(version: &str, duration_seconds: u64) -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "play_session".into(), title: format!("Played {}", version), description: format!("Session duration: {}m", duration_seconds / 60), icon: "GameboyIcon".into() }
}

pub fn journey_achievement_unlocked(title: &str) -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "achievement_unlocked".into(), title: "Earned Achievement".into(), description: title.to_string(), icon: "Trophy01Icon".into() }
}

pub fn journey_badge_earned(title: &str) -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "badge_earned".into(), title: "Earned Badge".into(), description: title.to_string(), icon: "Medal01Icon".into() }
}

pub fn journey_instance_created(name: &str) -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "instance_created".into(), title: "Created Instance".into(), description: name.to_string(), icon: "Package01Icon".into() }
}

pub fn journey_profile_customized() -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "profile_customized".into(), title: "Customized Profile".into(), description: "Updated profile appearance.".into(), icon: "UserEditIcon".into() }
}

pub fn journey_milestone(label: &str) -> JourneyEvent {
    JourneyEvent { id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().timestamp(), kind: "milestone".into(), title: label.to_string(), description: "Reached a new milestone.".into(), icon: "Star01Icon".into() }
}

pub fn recompute_statistics(profile: &mut GlitchyProfile, journey: &JourneyStore) {
    let mut stats = Statistics::default();
    stats.achievements_unlocked = profile.achievements.states.values().filter(|s| s.unlocked).count() as u32;
    stats.badges_earned = profile.badges.earned.len() as u32;

    if let Some(ev) = journey.events.iter().filter(|e| e.kind == "first_launch").min_by_key(|e| e.timestamp) {
        stats.first_launch_at = Some(ev.timestamp);
    }

    let mut sessions_by_version: HashMap<String, u64> = HashMap::new();
    let mut last_played: Option<i64> = None;
    for ev in &journey.events {
        if ev.kind == "play_session" {
            stats.total_sessions += 1;
            if let Some(mins_str) = ev.description.strip_prefix("Session duration: ") {
                if let Some(mins_str) = mins_str.strip_suffix('m') {
                    if let Ok(mins) = mins_str.parse::<u64>() {
                        stats.total_playtime_seconds += mins * 60;
                    }
                }
            }
            if let Some(ver) = ev.title.strip_prefix("Played ") {
                *sessions_by_version.entry(ver.to_string()).or_default() += 1;
            }
            if last_played.map_or(true, |t| ev.timestamp > t) {
                last_played = Some(ev.timestamp);
            }
        }
    }
    stats.last_played_at = last_played;
    stats.favorite_version = sessions_by_version.iter().max_by_key(|(_, v)| *v).map(|(k, _)| k.clone());
    profile.statistics = stats;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn condition_parser_handles_geq() {
        let (m, op, v) = parse_condition("sessions>=10");
        assert_eq!(m, "sessions"); assert_eq!(op, ">="); assert_eq!(v, 10);
    }
    #[test]
    fn condition_parser_handles_eq() {
        let (m, op, v) = parse_condition("first_launch==1");
        assert_eq!(m, "first_launch"); assert_eq!(op, "=="); assert_eq!(v, 1);
    }
    #[test]
    fn evaluate_unlocks_first_launch() {
        let mut profile = GlitchyProfile::default();
        profile.statistics.total_sessions = 1;
        profile.statistics.first_launch_at = Some(chrono::Utc::now().timestamp());
        let unlocked = evaluate_achievements(&mut profile, &EvalCtx::default());
        assert!(unlocked.contains(&"first_launch".to_string()));
    }
    #[test]
    fn evaluate_badges_for_first_launch() {
        let mut profile = GlitchyProfile::default();
        profile.statistics.total_sessions = 1;
        profile.statistics.first_launch_at = Some(chrono::Utc::now().timestamp());
        let earned = evaluate_badges(&mut profile, &EvalCtx::default());
        assert!(earned.contains(&"minecraft_player".to_string()));
        assert!(earned.contains(&"early_user".to_string()));
        assert!(earned.contains(&"founder".to_string()));
    }
    #[test]
    fn evaluate_condition_supports_1_0_0_metrics() {
        let stats = Statistics::default();
        let badges = BadgesStore::default();
        let ctx = EvalCtx {
            distinct_versions: 5,
            longest_session_seconds: 21600,
            days_active: 7,
            custom_avatar: 1,
            ..EvalCtx::default()
        };
        let (met, progress) = evaluate_condition("distinct_versions>=3", &stats, &badges, &ctx);
        assert!(met);
        assert_eq!(progress, 5);
        let (met, _) = evaluate_condition("longest_session_seconds>=21600", &stats, &badges, &ctx);
        assert!(met);
        let (met, _) = evaluate_condition("days_active>=7", &stats, &badges, &ctx);
        assert!(met);
        let (met, _) = evaluate_condition("custom_avatar>=1", &stats, &badges, &ctx);
        assert!(met);
        let (met, _) = evaluate_condition("achievements>=15", &stats, &badges, &ctx);
        assert!(!met);
    }
    #[test]
    fn catalog_contains_fifteen_new_entries_each() {
        assert_eq!(achievement_catalog().len(), 23);
        assert_eq!(badge_catalog().len(), 23);
    }
    #[test]
    fn journey_store_caps_at_max() {
        let mut store = JourneyStore::default();
        for index in 0..JourneyStore::MAX_EVENTS + 50 {
            store.push(journey_milestone(&format!("test-{index}")));
        }
        assert_eq!(store.events.len(), JourneyStore::MAX_EVENTS);
    }
    #[test]
    fn journey_store_ignores_immediate_duplicates() {
        let mut store = JourneyStore::default();
        let event = journey_profile_customized();
        store.push(event.clone());
        store.push(event);
        assert_eq!(store.events.len(), 1);
    }
    #[test]
    fn allowed_accents_are_exactly_the_palette() {
        assert_eq!(ALLOWED_ACCENTS.len(), 6);
        assert!(ALLOWED_ACCENTS.contains(&"#2484EC"));
    }
}
