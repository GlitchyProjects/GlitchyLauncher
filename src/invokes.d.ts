import type { app } from "@tauri-apps/api";

type AppHandle = typeof app;

/**
 * Error envelope returned by every Tauri command.
 *
 * - `code` — stable token the frontend switches on (e.g. "ERROR_JAVA_NOT_FOUND")
 * - `message` — short human-readable summary from the Rust `Display` impl
 * - `details` — optional technical detail (file path, URL, parse error)
 * - `recoverable` — whether the user can retry (true for network errors)
 * - `recovery` — optional action token (e.g. "ACTION_INSTALL_JAVA")
 *
 * The frontend (`src/messages/errors.json` + `useBackend` hook) maps
 * `code` to a localized message and `recovery` to a button.
 */
export interface InvokeError<T = unknown> {
  code: string;
  details?: T;
  message: string;
  recoverable: boolean;
  recovery?: string | null;
}

/**
 * Per-command descriptor: arguments + return type. `customError` is
 * reserved for future use (currently the same `InvokeError` shape for
 * every command, but exposed so callers can narrow if needed).
 */
export interface InvokeDescriptor {
  args: unknown;
  returns: unknown;
}

/**
 * Strongly-typed map of every Tauri command exposed by the Rust backend.
 *
 * Every entry MUST match a `#[command]` function in `src-tauri/src/`.
 * When adding/removing a command, update this file AND the Rust side
 * in the same change to keep the contract synchronized.
 */
// biome-ignore lint/style/useConsistentTypeDefinitions: this command map is intentionally grouped by backend domain.
export type Invokes = {
  // Versions / downloads
  get_vanilla_versions: { args: undefined; returns: VersionCategory[] };
  get_forge_versions: { args: undefined; returns: VersionCategory[] };
  get_fabric_versions: { args: undefined; returns: VersionCategory[] };
  get_optifine_versions: { args: undefined; returns: VersionCategory[] };
  get_versions: { args: undefined; returns: string[] };
  get_installed_versions: { args: undefined; returns: string[] };
  get_non_installed_versions: { args: undefined; returns: string[] };
  download_version: {
    args: { versionLoader: VersionLoader; name: string };
    returns: string; // download session id
  };
  get_active_downloads: { args: undefined; returns: DownloadSessionInfo[] };
  pause_download: { args: { sessionId: string }; returns: void };
  resume_download: { args: { sessionId: string }; returns: void };
  cancel_download: { args: { sessionId: string }; returns: void };
  clear_finished_downloads: { args: undefined; returns: void };
  repair_version: {
    args: { versionId: string; applyRepair: boolean };
    returns: RepairReport;
  };

  // Profiles (offline only in this phase)
  get_profiles: { args: undefined; returns: Profile[] };
  create_offline_profile: { args: { username: string }; returns: Profile };
  rename_profile: {
    args: { uuid: string; newUsername: string };
    returns: Profile;
  };
  remove_profile: { args: { profile: Profile }; returns: void };
  get_selected_profile: { args: undefined; returns: Profile | null };
  set_selected_profile: { args: { profile: Profile }; returns: void };

  // Settings
  get_maximum_ram_usage: { args: undefined; returns: number };
  set_maximum_ram_usage: { args: { ramUsage: number }; returns: void };
  get_minimum_ram_usage: { args: undefined; returns: number };
  set_minimum_ram_usage: { args: { ramUsage: number }; returns: void };
  get_language: { args: undefined; returns: string };
  set_language: { args: { lang: string }; returns: void };
  should_exit_on_launch: { args: undefined; returns: boolean };
  set_exit_on_launch: { args: { toggle: boolean }; returns: void };
  get_total_ram: { args: undefined; returns: number };
  save: { args: undefined; returns: void };
  set_config: { args: { config: Config }; returns: void };

  // Java
  list_detected_javas: { args: undefined; returns: DetectedJava[] };
  get_selected_java: { args: undefined; returns: string | null };
  set_selected_java: { args: { path: string | null }; returns: void };

  // Backpack (mods / resource packs / shader packs — all per-instance)
  get_backpack_items: {
    args: { versionId: string; category: BackpackCategory };
    returns: ModInfo[];
  };
  get_instance_info: { args: { instanceId: string }; returns: InstanceInfo };
  toggle_backpack_item: {
    args: { item: ModInfo; toggle: boolean; category: BackpackCategory };
    returns: void;
  };
  delete_backpack_item: { args: { item: ModInfo }; returns: void };
  import_backpack_item: {
    args: { versionId: string; category: BackpackCategory };
    returns: void;
  };
  open_backpack_folder: {
    args: { versionId: string; category: BackpackCategory };
    returns: void;
  };

  // Modrinth
  modrinth_search: {
    args: {
      query: string;
      gameVersion: string;
      loader: string;
      projectType: string;
      offset: number;
      limit: number;
    };
    returns: ModrinthSearchResults;
  };
  modrinth_get_project_versions: {
    args: { projectId: string; gameVersion: string; loader: string };
    returns: ModrinthVersion[];
  };
  modrinth_download_item: {
    args: { versionId: string; instanceId: string; category: BackpackCategory };
    returns: string;
  };

  // Modpacks
  search_modpacks: {
    args: {
      query: string;
      offset: number;
      limit: number;
    };
    returns: ModpackSearchResults;
  };
  get_modpack_versions: {
    args: { projectId: string };
    returns: ModpackVersion[];
  };
  install_modpack: {
    args: {
      versionId: string;
      name: string;
    };
    returns: string;
  };
  import_modpack: { args: undefined; returns: string | null };

  // Per-version instance lifecycle, storage, worlds and backups
  list_instances: { args: undefined; returns: InstanceSummary[] };
  update_instance_settings: {
    args: {
      instanceId: string;
      displayName: string;
      ramMinMb: number | null;
      ramMaxMb: number | null;
    };
    returns: void;
  };
  set_instance_path: {
    args: { instanceId: string; newPath: string; moveFiles: boolean };
    returns: void;
  };
  reset_instance_path: { args: { instanceId: string }; returns: void };
  clone_instance: {
    args: { instanceId: string; newInstanceId: string };
    returns: void;
  };
  delete_instance: { args: { instanceId: string }; returns: void };
  reinstall_instance: {
    args: { instanceId: string };
    returns: RepairReport;
  };
  list_instance_worlds: {
    args: { instanceId: string };
    returns: WorldSummary[];
  };
  delete_instance_world: {
    args: { instanceId: string; worldName: string };
    returns: void;
  };
  create_instance_backup: {
    args: { instanceId: string };
    returns: BackupSummary;
  };
  list_instance_backups: {
    args: { instanceId: string };
    returns: BackupSummary[];
  };
  restore_instance_backup: {
    args: { instanceId: string; backupName: string };
    returns: void;
  };
  delete_instance_backup: {
    args: { instanceId: string; backupName: string };
    returns: void;
  };
  open_instance_folder: {
    args: {
      instanceId: string;
      folder:
        | "root"
        | "mods"
        | "resourcepacks"
        | "shaderpacks"
        | "saves"
        | "backups";
    };
    returns: void;
  };

  // Mirrors
  get_available_mirrors: { args: undefined; returns: Mirror[] };
  get_mirror: { args: undefined; returns: Mirror };
  set_mirror: { args: { mirror: Mirror }; returns: void };
  import_mirror: { args: { json: string }; returns: Mirror[] };

  // Logger
  get_log_history: { args: undefined; returns: LogLine[] };
  clear_log_history: { args: undefined; returns: void };
  clear_log_history_channel: { args: { channel: string }; returns: void };
  debug: { args: { text: string }; returns: void };

  // Play
  play: { args: { selectedVersion: string }; returns: LaunchResult };

  // Glitchy — achievements, badges, profile, journey, stats, fullscreen
  glitchy_get_achievements: {
    args: undefined;
    returns: AchievementWithState[];
  };
  glitchy_get_badges: { args: undefined; returns: BadgeWithState[] };
  glitchy_set_displayed_badges: { args: { badgeIds: string[] }; returns: void };
  glitchy_get_profile: { args: undefined; returns: GlitchyProfile };
  glitchy_save_customization: {
    args: { customization: ProfileCustomization };
    returns: void;
  };
  glitchy_upload_avatar: { args: { sourcePath: string }; returns: string };
  glitchy_avatar_path: { args: { filename: string }; returns: string };
  glitchy_avatar_data: { args: { filename: string }; returns: string }; // data: URL
  glitchy_get_allowed_accents: { args: undefined; returns: string[] };
  glitchy_get_journey: { args: undefined; returns: JourneyStore };
  glitchy_get_statistics: { args: undefined; returns: Statistics };
  glitchy_get_recent_activity: {
    args: { limit?: number };
    returns: ActivityEntry[];
  };
  glitchy_is_maximized: { args: undefined; returns: boolean };
  glitchy_toggle_maximized: { args: undefined; returns: boolean };

  // Glitchy AI Assistant
  get_system_diagnostics: { args: undefined; returns: SystemDiagnostics };
  ai_chat: {
    args: { messages: ChatMessage[]; systemContext?: string };
    returns: string;
  };
  ai_diagnose_log: {
    args: { manualLog?: string; systemContext?: string };
    returns: AiDiagnosisResult;
  };
  ai_apply_autofix: { args: { action: string }; returns: string };
  get_ai_usage_status: { args: undefined; returns: AiUsageStatus };
  check_launcher_update: { args: undefined; returns: LauncherUpdateInfo };
  apply_launcher_update: { args: { downloadUrl: string }; returns: void };

  // Glitchy Account
  glitchy_account_register: {
    args: { username: string; email: string; password: string };
    returns: GlitchyAuthResponse;
  };
  glitchy_account_login: {
    args: { login: string; password: string };
    returns: GlitchyAuthResponse;
  };
  glitchy_account_logout: { args: undefined; returns: void };
  glitchy_account_get_current: { args: undefined; returns: GlitchyUser | null };
  glitchy_account_get_token: { args: undefined; returns: string | null };
  glitchy_account_upload_skin: {
    args: { skinData: string; model: string; capeData?: string | null };
    returns: void;
  };
  glitchy_account_sync_save: {
    args: { settingsJson?: string | null; instancesJson?: string | null };
    returns: void;
  };
  glitchy_account_sync_load: {
    args: undefined;
    returns: [string | null, string | null];
  };
};

export interface GlitchyUser {
  id: string;
  username: string;
  email: string;
  role: string;
  badge: string;
  skinData?: string | null;
  skinModel: string;
  capeData?: string | null;
  createdAt: number;
}

export interface GlitchyAuthResponse {
  success: boolean;
  token?: string;
  user?: GlitchyUser;
  error?: string;
}

export interface LauncherUpdateInfo {
  currentVersion: string;
  downloadUrl?: string;
  hasUpdate: boolean;
  latestVersion: string;
  releaseNotes: string;
  releaseUrl: string;
}

export interface SystemDiagnostics {
  allocatedRamMb: number;
  cpu: string;
  freeRamMb: number;
  gameVersion: string;
  gpu: string;
  installedMods: string[];
  os: string;
  selectedJava: string;
  totalRamMb: number;
}

export interface ChatMessage {
  content: string;
  role: string;
}

export interface AiDiagnosisResult {
  aiExplanation: string;
  autoFixAvailable: boolean;
  autoFixDescription: string | null;
  detectedIssueType: string;
  logSnippet: string;
  source: string;
}

export interface AiUsageStatus {
  isRateLimited: boolean;
  maxRequests: number;
  remainingRequests: number;
  resetInSeconds: number;
}

export interface Mirror {
  description: string;
  maps?: Record<string, string>;
  name: string;
  url?: string;
}

export interface MinecraftVersion {
  base: "FABRIC" | "FORGE" | "NEO_FORGE" | "LITE_LOADER" | "VANILLA";
  date: string;
  id: string;
  inheritedVersion?: string;
  isInstalled: boolean;
}

export enum VersionBase {
  VANILLA = 0,
  FORGE = 1,
  NEOFORGE = 2,
  FABRIC = 3,
  LITELOADER = 4,
  OPTIFINE = 5,
}

export interface Profile {
  createdAt: number;
  online: boolean;
  username: string;
  uuid: string;
}

export interface VersionLoader {
  base: VersionBase;
  date: string;
  id: string;
}

export interface VersionCategory {
  name: string;
  versions: VersionLoader[];
}

export interface Config {
  downloadSettings: DownloadSettings;
  launcherSettings: LauncherSettings;
  launchOptions: LaunchOptions;
  nativeLibraries: NativeLibraries;
}

export interface LaunchOptions {
  javaOverride: string | null;
  ramUsageMax: number;
  ramUsageMin: number;
  selectedProfile: string;
  useDedicatedGpu: boolean;
}

export interface LauncherSettings {
  exitOnLaunch: boolean;
  language: string;
}

export interface DownloadSettings {
  mirror: string;
}

export interface NativeLibraries {
  glfwPath: string;
  openalPath: string;
  useCustomGlfw: boolean;
  useCustomOpenal: boolean;
}

export interface ModInfo {
  description: string;
  enabled: boolean;
  modId: string;
  name: string;
  path: string;
  version: string;
}

export interface ModpackSearchResults {
  hits: ModpackSearchHit[];
  totalHits: number;
}

export interface ModpackSearchHit {
  author: string;
  description: string;
  downloads: number;
  iconUrl: string | null;
  projectId: string;
  title: string;
}

export interface ModpackVersion {
  datePublished: string;
  downloadUrl: string | null;
  fileName: string;
  gameVersions: string[];
  id: string;
  name: string;
  size: number;
}

export interface InstanceSummary {
  backupCount: number;
  displayName: string;
  gameVersion: string;
  id: string;
  loader: string;
  modCount: number;
  modpackName: string | null;
  path: string;
  ramMaxMb: number | null;
  ramMinMb: number | null;
  sizeBytes: number;
  worldCount: number;
}

export interface WorldSummary {
  modifiedAt: number;
  name: string;
  path: string;
  sizeBytes: number;
}

export interface BackupSummary {
  createdAt: number;
  name: string;
  path: string;
  sizeBytes: number;
}

export interface DetectedJava {
  major: number;
  path: string;
  source: "launcher" | "system";
  version: string;
}

export interface LogLine {
  channel: string;
  level: string;
  message: string;
  timestamp: string;
}

export interface LaunchResult {
  pid: number;
  startedAt: number;
  username: string;
  versionId: string;
}

// Repair report types
export type ArtifactStatus =
  | "ok"
  | "missing"
  | { size_mismatch: { expected: number; actual: number } }
  | { hash_mismatch: { expected: string; actual: string } };

export interface RepairArtifact {
  category: string;
  path: string;
  status: ArtifactStatus;
}

export interface RepairReport {
  artifacts: RepairArtifact[];
  brokenCount: number;
  okCount: number;
  repaired: boolean;
  versionId: string;
}

// Download progress event payload
export interface DownloadProgress {
  batchId: string;
  completedBytes: number;
  currentLabel: string;
  filesDone: number;
  filesFailed: number;
  filesTotal: number;
  retryAttempt: number;
  state:
    | "queued"
    | "downloading"
    | "verifying"
    | "retrying"
    | "completed"
    | "failed"
    | "cancelled"
    | "completed_with_errors";
  totalBytes: number;
}

// Background download session (event `download-session` + the
// get_active_downloads command). Mirrors Rust `SessionInfo` with
// #[serde(rename_all = "camelCase")].
export type DownloadSessionState =
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type DownloadSessionPhase =
  | "prepare"
  | "loader"
  | "java"
  | "libraries"
  | "client"
  | "assets"
  | "done";

export interface DownloadSessionInfo {
  bytesPerSecond: number;
  createdAt: number;
  currentFile: string;
  downloadedBytes: number;
  error: string | null;
  id: string;
  label: string;
  percent: number;
  phase: DownloadSessionPhase | string;
  state: DownloadSessionState;
  totalBytes: number;
  versionId: string;
}

// Launch preparation progress event payload (event `launch-progress`)
export interface LaunchProgress {
  percent: number;
  phase: string;
}

// Game exit event payload
export type GameExitKind =
  | { kind: "normal_exit"; code: number }
  | { kind: "crash"; code: number }
  | { kind: "terminated" }
  | { kind: "launch_failure"; reason: string };

export interface GameExitEvent {
  durationMs: number;
  kind: GameExitKind["kind"];
  versionId: string;
}

// Modrinth

export type BackpackCategory = "mods" | "resourcePacks" | "shaderPacks";

export interface InstanceInfo {
  gameVersion: string;
  loader: string;
}

export interface ModrinthSearchResults {
  hits: ModrinthSearchHit[];
  totalHits: number;
}

export interface ModrinthSearchHit {
  author: string | null;
  categories: string[] | null;
  description: string | null;
  downloads: number | null;
  iconUrl: string | null;
  projectId: string;
  projectType: string | null;
  slug: string | null;
  title: string | null;
}

export interface ModrinthVersion {
  datePublished: string | null;
  downloads: number | null;
  gameVersions: string[] | null;
  id: string;
  loaders: string[] | null;
  name: string | null;
  primaryFile: ModrinthFile;
  versionNumber: string | null;
  versionType: string | null;
}

export interface ModrinthFile {
  filename: string;
  primary: boolean;
  size: number | null;
  url: string;
}

// ============================================================
// Glitchy — achievements, badges, profile, journey, stats, fullscreen
// ============================================================

export type AchievementRarity = "common" | "rare" | "epic" | "legendary";

export interface AchievementDef {
  condition: string;
  description: string;
  icon: string;
  id: string;
  rarity: AchievementRarity;
  target: number | null;
  title: string;
}
export interface AchievementState {
  progress: number;
  unlocked: boolean;
  unlockedAt: number | null;
}
export interface AchievementWithState
  extends AchievementDef,
    AchievementState {}

export interface BadgeDef {
  autoAwarded: boolean;
  condition: string | null;
  description: string;
  icon: string;
  id: string;
  title: string;
}
export interface BadgeWithState extends BadgeDef {
  displayed: boolean;
  earned: boolean;
}

export type Avatar =
  | { kind: "preset"; id: string }
  | { kind: "custom"; filename: string };

export type ProfileFrame =
  | "none"
  | "solid_blue"
  | "gradient_electric"
  | "gradient_icy"
  | "neon_pulse";
export type ProfileBackground =
  | "aurora"
  | "deep_sea"
  | "grid"
  | "particles"
  | "solid";
export type GlowStyle = "none" | "subtle" | "normal" | "strong";

/** Accent color is a plain hex string (e.g. "#2484EC"). The Rust side
 *  uses a newtype `AccentColor(pub String)` with `#[serde(transparent)]`
 *  so it serializes as a bare JSON string — NOT as an object or array. */
export type AccentColor = string;
export interface ProfileCustomization {
  accent: AccentColor;
  avatar: Avatar;
  background: ProfileBackground;
  frame: ProfileFrame;
  glow: GlowStyle;
  tagline: string;
}
export interface Statistics {
  achievementsUnlocked: number;
  badgesEarned: number;
  favoriteVersion: string | null;
  firstLaunchAt: number | null;
  lastPlayedAt: number | null;
  mostUsedInstance: string | null;
  totalPlaytimeSeconds: number;
  totalSessions: number;
}
export interface GlitchyProfile {
  achievements: { states: Record<string, AchievementState> };
  badges: { earned: string[]; displayed: string[] };
  customization: ProfileCustomization;
  statistics: Statistics;
}
export interface JourneyEvent {
  description: string;
  icon: string;
  id: string;
  kind: string;
  timestamp: number;
  title: string;
}
export interface JourneyStore {
  events: JourneyEvent[];
}
export interface ActivityEntry {
  icon: string;
  id: string;
  kind: string;
  timestamp: number;
  title: string;
}
