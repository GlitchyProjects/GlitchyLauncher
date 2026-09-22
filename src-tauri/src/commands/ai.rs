use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::models::error::AppError;
use crate::services::directory_manager::{get_falcon_launcher_directory, get_minecraft_directory};
use crate::AppState;

const MAX_AI_REQUESTS_PER_WINDOW: u32 = 20;
const AI_WINDOW_DURATION_SECS: u64 = 5 * 3600; // 5 hours

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AiUsageStatus {
    pub remaining_requests: u32,
    pub max_requests: u32,
    pub reset_in_seconds: u64,
    pub is_rate_limited: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct AiRateLimitStore {
    window_start_epoch: u64,
    used_requests: u32,
}

fn get_rate_limit_storage_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. Launcher folder in minecraft directory
    paths.push(get_falcon_launcher_directory().join("ai_rate_limit.json"));

    // 2. Windows LocalAppData or Unix config directory
    if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
        paths.push(PathBuf::from(local_app).join("GlitchyLauncher").join("sys_vault.bin"));
    } else if let Ok(home) = std::env::var("HOME") {
        paths.push(PathBuf::from(home).join(".config").join("GlitchyLauncher").join("sys_vault.bin"));
    }

    // 3. System-wide ProgramData (Windows) or /var/tmp (Unix)
    if let Ok(prog_data) = std::env::var("ProgramData") {
        paths.push(PathBuf::from(prog_data).join("GlitchyLauncher").join("sys_vault.bin"));
    } else if let Ok(all_users) = std::env::var("ALLUSERSPROFILE") {
        paths.push(PathBuf::from(all_users).join("GlitchyLauncher").join("sys_vault.bin"));
    }

    // 4. User profile root
    if let Ok(profile) = std::env::var("USERPROFILE") {
        paths.push(PathBuf::from(profile).join(".glitchy_sys_vault.bin"));
    }

    paths
}

fn encode_rate_limit_store(store: &AiRateLimitStore) -> Vec<u8> {
    if let Ok(json) = serde_json::to_string(store) {
        let key: &[u8] = b"GlitchyAntiTamperVaultKey2026";
        let masked: Vec<u8> = json
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(masked).into_bytes()
    } else {
        Vec::new()
    }
}

fn decode_rate_limit_store(bytes: &[u8]) -> Option<AiRateLimitStore> {
    use base64::Engine;
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(bytes) {
        let key: &[u8] = b"GlitchyAntiTamperVaultKey2026";
        let unmasked: Vec<u8> = decoded
            .into_iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        if let Ok(store) = serde_json::from_slice::<AiRateLimitStore>(&unmasked) {
            return Some(store);
        }
    }

    // Fallback: try plain JSON
    if let Ok(store) = serde_json::from_slice::<AiRateLimitStore>(bytes) {
        return Some(store);
    }

    None
}

fn get_current_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn save_rate_limit_store(store: &AiRateLimitStore) {
    let encoded = encode_rate_limit_store(store);
    if encoded.is_empty() {
        return;
    }
    for path in get_rate_limit_storage_paths() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, &encoded);
    }
}

fn load_rate_limit_store() -> (AiRateLimitStore, u64) {
    let now = get_current_epoch_secs();
    let paths = get_rate_limit_storage_paths();

    let mut valid_stores = Vec::new();

    for path in paths {
        if let Ok(bytes) = fs::read(&path) {
            if let Some(store) = decode_rate_limit_store(&bytes) {
                valid_stores.push(store);
            }
        }
    }

    // Filter for stores whose 5-hour window has not expired yet
    let active_stores: Vec<AiRateLimitStore> = valid_stores
        .into_iter()
        .filter(|s| {
            now >= s.window_start_epoch && now < s.window_start_epoch.saturating_add(AI_WINDOW_DURATION_SECS)
        })
        .collect();

    let consolidated_store = if !active_stores.is_empty() {
        // Pick the most restrictive record (highest used_requests)
        let max_used = active_stores.iter().map(|s| s.used_requests).max().unwrap_or(0);
        let min_start = active_stores.iter().map(|s| s.window_start_epoch).min().unwrap_or(now);

        AiRateLimitStore {
            window_start_epoch: min_start,
            used_requests: max_used,
        }
    } else {
        AiRateLimitStore {
            window_start_epoch: now,
            used_requests: 0,
        }
    };

    // Synchronize to all locations
    save_rate_limit_store(&consolidated_store);

    (consolidated_store, now)
}

fn check_and_consume_rate_limit() -> Result<AiUsageStatus, AppError> {
    let (mut store, now) = load_rate_limit_store();
    let window_end = store.window_start_epoch.saturating_add(AI_WINDOW_DURATION_SECS);
    let reset_in_seconds = window_end.saturating_sub(now);

    if store.used_requests >= MAX_AI_REQUESTS_PER_WINDOW {
        let hours = reset_in_seconds / 3600;
        let minutes = (reset_in_seconds % 3600) / 60;
        return Err(AppError::UnknownError(format!(
            "سقف مجاز ۲۰ درخواست هوش مصنوعی در هر ۵ ساعت تکمیل شده است. زمان باقی‌مانده تا بازنشانی: {} ساعت و {} دقیقه دیگر.",
            hours, minutes
        )));
    }

    store.used_requests += 1;
    save_rate_limit_store(&store);

    let remaining_requests = MAX_AI_REQUESTS_PER_WINDOW.saturating_sub(store.used_requests);
    Ok(AiUsageStatus {
        remaining_requests,
        max_requests: MAX_AI_REQUESTS_PER_WINDOW,
        reset_in_seconds,
        is_rate_limited: remaining_requests == 0,
    })
}

#[command]
pub async fn get_ai_usage_status() -> Result<AiUsageStatus, AppError> {
    let (store, now) = load_rate_limit_store();
    let window_end = store.window_start_epoch.saturating_add(AI_WINDOW_DURATION_SECS);
    let reset_in_seconds = window_end.saturating_sub(now);
    let remaining_requests = MAX_AI_REQUESTS_PER_WINDOW.saturating_sub(store.used_requests);

    Ok(AiUsageStatus {
        remaining_requests,
        max_requests: MAX_AI_REQUESTS_PER_WINDOW,
        reset_in_seconds,
        is_rate_limited: remaining_requests == 0,
    })
}

// Cloudflare Workers AI Configuration
const CF_ACCOUNT_ID: &str = "";
const CF_API_TOKEN: &str = "";
const CF_MODEL: &str = "@cf/meta/llama-3.1-8b-instruct";

// Secondary Fallback Configuration
const FALLBACK_API_URL: &str = "https://api.z.ai/api/paas/v4/chat/completions";
const FALLBACK_API_KEY: &str = "";
const FALLBACK_MODEL: &str = "GLM-4.5-Flash";

const MINECRAFT_KNOWLEDGE_BASE: &str = r#"
=== GLITCHY MINECRAFT TECHNICAL KNOWLEDGE BASE & LAUNCHER MANUAL ===

1. STRICT BAN ON CURSEFORGE & EXTERNAL SOURCES:
- Glitchy Launcher does NOT use, have, or support CurseForge!
- Glitchy Launcher uses MODRINTH ONLY for all mods, modpacks, resource packs, and shaders.
- NEVER suggest or recommend CurseForge or CurseForge App to the user under any circumstances!
- NEVER tell the user to open an external web browser, download jar files from random websites, or manually place files in Windows File Explorer!
- Everything in Glitchy Launcher is 100% built-in with 1-click downloads powered directly by Modrinth.

2. NEW GLITCHY LAUNCHER NAVIGATION & UI STRUCTURE (6 MAIN HUBS):
Always guide users using the EXACT names and navigation hierarchy of Glitchy Launcher:

* HUB 1: "Play" (شروع بازی)
  - Main dashboard to launch Minecraft with 1 click.
  - Choose installed version/instance from the dropdown at the bottom.
  - Shows download and launch progress bar.

* HUB 2: "Library" (مخزن بازی)
  - Has 3 full-width sub-tabs at the top:
    1. "Instances" (نسخه‌ها): View, create, configure, and launch isolated Minecraft instances (Vanilla, Fabric, Forge, NeoForge, Quilt).
    2. "Modpacks" (مادپک‌ها): 1-click search and install from thousands of MODRINTH modpacks.
    3. "Backpack" (کوله‌پشتی): Manage your installed Mods, Resource Packs, and Shaders:
       - To install mods: Library -> Backpack -> click the blue "Get Mods" button (opens built-in Modrinth store).
       - To install shaders: Library -> Backpack -> click "Shader Packs" tab -> click "Get Shader Packs".
       - To install resource packs: Library -> Backpack -> click "Resource Packs" tab -> click "Get Resource Packs".

* HUB 3: "Downloads" (دانلودها)
  - Has 3 full-width sub-tabs:
    1. "Minecraft": Complete step-by-step installer wizard for all official Minecraft releases and snapshots with Fabric, Forge, NeoForge, or Vanilla.
    2. "Legends": Minecraft Legends downloads.
    3. "PvP Clients": Optimized PvP clients.

* HUB 4: "Profile" (پروفایل)
  - Has 4 full-width sub-tabs:
    1. "Overview": Profile card, stats (playtime hours, total sessions), and "Customize" button to personalize avatar frames, backgrounds, and glows.
    2. "Achievements": Unlockable launcher and in-game achievements.
    3. "Badges": Special badges earned by the player.
    4. "Journey": Milestone and event timeline.

* HUB 5: "AI Assistant" (دستیار هوشمند)
  - Has 2 full-width sub-tabs:
    1. "Diagnostics & Auto-Fix": Automatic crash detector and log analyzer with the purple "Apply 1-Click Fix" button!
    2. "AI Chat & Guide": This conversational expert chat.

* HUB 6: "Settings" (تنظیمات)
  - Has 4 full-width sub-tabs:
    1. "Launcher Settings": Adjust Memory Allocation (RAM) slider (min/max RAM in MB) and manage Java runtimes.
    2. "Game Options": Switch Interface Language between English and فارسی (complete RTL layout), set resolution, and fullscreen.
    3. "Mirrors": High-speed download mirrors (Official Mojang, BMCLAPI, MCBBS).
    4. "Console": Live real-time game logs, errors, and system output with filter buttons.

3. HOW TO GUIDE THE USER WITH STEP-BY-STEP INSTRUCTIONS:
* HOW TO INSTALL MODS (Sodium, Iris, Fabric API, Lithium, etc.):
  1. Go to "Library" in the left menu.
  2. Click the "Backpack" sub-tab.
  3. Select your instance/version at the top.
  4. Click the blue "Get Mods" button to open the built-in Modrinth store.
  5. Search for the mod and click "Download" — it installs automatically with 1 click!

* CRITICAL WARNING: STRICT SODIUM BETA/ALPHA BAN (قانون حیاتی نسخه سدیم):
  ALWAYS warn the user:
  "⚠️ نکته بسیار مهم درباره سودیوم: اکیداً از نسخه‌های Beta یا Alpha سدیم استفاده نکن!
  نسخه‌های بتا و آلفای سدیم پایداری کافی ندارند، تست‌های سازگاری آن‌ها کامل نیست و باعث کرش‌های مکرر، ارورهای رندرینگ، تداخل شدید با Iris یا Indium و ارور Exit Code 1 می‌شوند.
  هنگام نصب در بخش Get Mods، فقط و فقط نسخه Release (پایدار) سدیم را نصب کن."

* HOW TO INSTALL SHADERS (Complementary, BSL, Bliss, MakeUp):
  1. Make sure you have Iris and Sodium installed via Library -> Backpack -> Get Mods.
  2. Go to "Library" -> "Backpack" -> click the "Shader Packs" tab.
  3. Click "Get Shader Packs" to search and install shaders with 1 click!

* HOW TO INSTALL RESOURCE PACKS:
  1. Go to "Library" -> "Backpack" -> click the "Resource Packs" tab.
  2. Click "Get Resource Packs" to search and install texture packs.

* HOW TO INSTALL MODPACKS:
  1. Go to "Library" -> "Modpacks" sub-tab.
  2. Search among thousands of Modrinth modpacks.
  3. Click "Install" — the launcher creates an isolated instance and downloads all mods automatically.
  (REMINDER: Glitchy Launcher uses Modrinth exclusively, NOT CurseForge!)

* HOW TO INSTALL NEW MINECRAFT VERSIONS:
  1. Go to "Downloads" in the left menu.
  2. Under the "Minecraft" sub-tab, select your desired version and loader (Vanilla, Fabric, Forge, NeoForge).
  3. Click "Install" to download and configure the game.

* HOW TO FIX OUT OF MEMORY (OutOfMemoryError) OR GAME LAG:
  Option A (Instant 1-Click Fix): In "AI Assistant" -> "Diagnostics & Auto-Fix" tab, click "Apply 1-Click Fix" to instantly allocate 4096 MB RAM.
  Option B (Manual Adjustment): Go to "Settings" in the left menu -> "Launcher Settings" tab -> adjust the "Memory Allocation (RAM)" slider to 4096 MB (4 GB) or higher.

* HOW TO FIX CRASHES & EXIT CODES:
  1. Go to "AI Assistant" -> "Diagnostics & Auto-Fix" tab.
  2. Click the purple button "Analyze Last Crash Automatically".
  3. Read the AI diagnosis and click "Apply 1-Click Fix" to resolve the issue automatically.

* HOW TO CREATE NEW GAME INSTANCES:
  Go to "Library" -> "Instances" sub-tab -> create a new instance with your choice of Vanilla, Fabric, Forge, or NeoForge.

* HOW TO CHANGE LAUNCHER LANGUAGE:
  Go to "Settings" -> "Game Options" tab -> "Interface Language" (supports English and فارسی with full RTL layout).

* HOW TO VIEW REAL-TIME GAME LOGS & DEBUG:
  Go to "Settings" -> "Console" sub-tab.

4. JAVA RUNTIME COMPATIBILITY MATRIX:
- Minecraft <= 1.16.5: Requires Java 8 (class file version 52.0).
- Minecraft 1.17 - 1.17.1: Requires Java 16 (class file version 60.0).
- Minecraft 1.18 - 1.20.4: Requires Java 17 (class file version 61.0).
- Minecraft 1.20.5+ and 1.21.x: Strictly requires Java 21 (class file version 65.0).
- Modern Minecraft 26.x (e.g. 26.2, 26.3): Requires Java 25 or newer.
- CRITICAL: The current official latest release of Minecraft Java Edition is 26.2 (released 2026-06-16, last manifest update: 2026-09-04). The latest snapshot is 26.3-pre-2. Mojang uses Year.Drop numbering. NEVER tell users that 1.20, 1.21, or 1.21.9 is the current latest version! Those are previous historical versions.
- Error "UnsupportedClassVersionError: ... class file version 65.0": Means game was compiled for Java 21, but is running on an older Java (Java 17 or 8). Solution: Switch Java to Java 21 in Settings or click Apply 1-Click Fix.
- Memory allocation: 3072 MB - 4096 MB for vanilla and light modpacks; 6144 MB - 8192 MB for large modpacks (100+ mods). Never allocate 100% of physical PC RAM.

5. MOD LOADERS & INCOMPATIBILITIES:
- Fabric vs Forge vs NeoForge:
  * Fabric mods CANNOT run on Forge. Forge mods CANNOT run on Fabric. Never mix them.
  * NeoForge is the modern fork of Forge starting from Minecraft 1.20.2+.
- Fabric API:
  * Over 90% of Fabric mods require "Fabric API" (install via Library -> Backpack -> Get Mods -> Fabric API). If missing, game exits with Exit Code 1 before opening.
- OptiFine vs Modern Fabric Optimization:
  * OptiFine is strictly incompatible with Sodium, Iris, Lithium, Indium, and Embeddium.
  * Never install OptiFine alongside Sodium or Iris.
  * For Fabric shaders: Use Iris Shaders + Sodium (DO NOT use OptiFine).
  * If a Fabric mod uses Fabric Rendering API (like Continuity), it requires "Indium" alongside Sodium.

6. OPTIMIZATION & POPULAR MODS STACK:
- Best Fabric Performance & Visual Mods (install via Library -> Backpack -> Get Mods):
  * Sodium: Replaces Minecraft's OpenGL renderer for massive FPS boost (Release version only! Strictly never Beta/Alpha).
  * Iris: Modern high-FPS shader loader for Sodium.
  * Continuity: Fabric mod for CONNECTED TEXTURES (اتصال شیشه‌ها و بافت‌های به‌هم‌پیوسته شبیه OptiFine). It is strictly for visual connected textures (glass/bookshelves), NOT for crash prevention or performance! When used with Sodium, it strictly requires "Indium".
  * Indium: Adds Fabric Rendering API support to Sodium (essential dependency for Continuity).
  * Lithium: Optimizes game physics, chunk ticking, and AI without changing gameplay.
  * FerriteCore: Reduces RAM usage by 30-50%.
  * ImmediatelyFast: Speeds up GUI, text rendering, and HUD.
  * Entity Culling: Skips rendering mobs/entities behind walls.
  * Krypton: Optimizes Minecraft's network stack.
- In-Game Video Settings:
  * Simulation Distance: 6 to 8 (reduces CPU load drastically).
  * Render Distance: 8 to 12 for weak/budget GPUs.
  * Graphics: Fast (or Fancy with Sodium), Particles: Minimal, Clouds: Off, Max Framerate: Unlimited.

7. COMMON CRASH SIGNATURES & ROOT CAUSES:
- "Exit Code 1" (no crash report in crash-reports/):
  * Pre-launch loader crash. Almost always caused by missing Fabric API, duplicate mod jars, a mod made for a different Minecraft version, or an incompatible Sodium Beta/Alpha build.
- "java.lang.OutOfMemoryError: Java heap space":
  * Out of memory. Use Apply 1-Click Fix or Settings -> RAM slider.
- "GLFW error 65542: WGL: The driver does not appear to support OpenGL":
  * Corrupted options.txt or outdated graphics drivers. Reset options.txt with Apply 1-Click Fix or update GPU drivers.
- "DuplicateModsException":
  * Two versions of the same mod exist in Library -> Backpack. Delete the older one.
- "net.fabricmc.loader.impl.FormattedException: Incompatible mod set":
  * Fabric loader explicitly lists which mod requires what dependency or Minecraft version.
- "org.spongepowered.asm.mixin.transformer.throwables.MixinTransformerError":
  * Bytecode injection conflict between two mods hooking the same method.

8. LANGUAGE & TONE RULES:
- ALWAYS respond to the user in fluent, energetic, friendly, and natural Persian (فارسی روان، خودمانی و گیمرپسند).
- Write English technical terms (like mod names: Sodium, Iris, Fabric API, Java 21, Backpack, Get Mods, Settings) clearly in English.
- Always recommend Glitchy Launcher's built-in buttons and sections first!
- CRITICAL: NEVER mention CurseForge! Glitchy Launcher only uses Modrinth. If anyone asks about CurseForge, inform them that Glitchy uses the faster and cleaner Modrinth platform.
"#;

fn get_system_diagnostic_prompt() -> String {
    format!(
        "You are the expert, smart, and friendly AI Assistant for Glitchy Minecraft Launcher.\n\
        Analyze the provided Minecraft crash reports, logs, and system environment. \
        Explain the root cause of the crash clearly and provide 2 to 4 actionable, easy steps to resolve it.\n\
        STRICT RULE: Always explain and respond in fluent, friendly, and natural PERSIAN (فارسی روان و صمیمی).\n\
        Mention if the 1-Click Auto-Fix button in Glitchy Launcher can fix the issue.\n\
        CRITICAL: Never mention or recommend CurseForge. Glitchy Launcher strictly uses Modrinth.\n\n\
        {MINECRAFT_KNOWLEDGE_BASE}"
    )
}

fn get_system_chat_prompt() -> String {
    format!(
        "You are the friendly, energetic, and expert AI Assistant for Glitchy Minecraft Launcher.\n\
        Answer questions about Minecraft gameplay, performance optimization, FPS boost, mod installation, commands, and settings.\n\
        STRICT RULE: Always talk and respond in fluent, friendly, and natural PERSIAN (فارسی روان و گیمرپسند).\n\
        CRITICAL CONVERSATION RULE: In ongoing chat conversations, NEVER greet the user (DO NOT say 'سلام', 'سلام گیمر', 'درود', 'سلام دوست من' or introductory pleasantries) at the start of your replies! The user already knows who you are and has been greeted. Jump DIRECTLY into answering the user's question with enthusiasm and clarity.\n\
        STRICT CURSEFORGE BAN: Glitchy Launcher does NOT use or support CurseForge under any circumstances! Only reference the built-in Modrinth engine located in Library -> Backpack -> Get Mods, or Library -> Modpacks. Never tell the user to use CurseForge.\n\
        Keep answers practical, accurate, and concise.\n\n\
        {MINECRAFT_KNOWLEDGE_BASE}"
    )
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SystemDiagnostics {
    pub os: String,
    pub cpu: String,
    pub total_ram_mb: u64,
    pub free_ram_mb: u64,
    pub gpu: String,
    pub allocated_ram_mb: u64,
    pub selected_java: String,
    pub game_version: String,
    pub installed_mods: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDiagnosisResult {
    pub log_snippet: String,
    pub ai_explanation: String,
    pub detected_issue_type: String, // "OUT_OF_MEMORY", "JAVA_VERSION", "GRAPHICS_DRIVER", "MOD_CONFLICT", "UNKNOWN"
    pub auto_fix_available: bool,
    pub auto_fix_description: Option<String>,
    pub source: String, // "CRASH_REPORT", "LATEST_LOG", "MOD_LOADER_CRASH", "MANUAL"
}

// Cloudflare Response Types
#[derive(Deserialize)]
struct CloudflareAiResponse {
    result: Option<CloudflareResult>,
    success: bool,
    errors: Option<Vec<CloudflareError>>,
}

#[derive(Deserialize)]
struct CloudflareResult {
    response: Option<String>,
    choices: Option<Vec<CloudflareChoice>>,
}

#[derive(Deserialize)]
struct CloudflareChoice {
    message: CloudflareMessage,
}

#[derive(Deserialize)]
struct CloudflareMessage {
    content: String,
}

#[derive(Deserialize)]
struct CloudflareError {
    message: String,
}

// Fallback Response Types
#[derive(Deserialize)]
struct FallbackResponse {
    choices: Option<Vec<FallbackChoice>>,
    error: Option<FallbackError>,
}

#[derive(Deserialize)]
struct FallbackChoice {
    message: FallbackMessage,
}

#[derive(Deserialize)]
struct FallbackMessage {
    content: String,
}

#[derive(Deserialize)]
struct FallbackError {
    message: String,
}

async fn call_cloudflare_ai(messages: Vec<ChatMessage>) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(35))
        .build()
        .map_err(|e| AppError::UnknownError(format!("Network client build failed: {e}")))?;

    let account_id = std::env::var("CF_ACCOUNT_ID").unwrap_or_else(|_| CF_ACCOUNT_ID.to_string());
    let api_token = std::env::var("CF_API_TOKEN").unwrap_or_else(|_| CF_API_TOKEN.to_string());
    if account_id.is_empty() || api_token.is_empty() {
        return Err(AppError::UnknownError(
            "Cloudflare AI is not configured. Please set CF_ACCOUNT_ID and CF_API_TOKEN.".to_string(),
        ));
    }

    let url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/{CF_MODEL}"
    );

    let payload = serde_json::json!({
        "messages": messages,
        "max_tokens": 2048,
        "temperature": 0.7,
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::UnknownError(format!("Cloudflare AI connection failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::UnknownError(format!(
            "Cloudflare AI returned error ({status}): {body}"
        )));
    }

    let parsed: CloudflareAiResponse = resp
        .json()
        .await
        .map_err(|e| AppError::UnknownError(format!("Failed to parse Cloudflare response: {e}")))?;

    if !parsed.success {
        if let Some(errs) = parsed.errors {
            let msg = errs.into_iter().map(|e| e.message).collect::<Vec<_>>().join(", ");
            return Err(AppError::UnknownError(format!("Cloudflare AI error: {msg}")));
        }
    }

    if let Some(res) = parsed.result {
        if let Some(text) = res.response {
            if !text.trim().is_empty() {
                return Ok(text.trim().to_string());
            }
        }
        if let Some(choices) = res.choices {
            if let Some(first) = choices.into_iter().next() {
                return Ok(first.message.content.trim().to_string());
            }
        }
    }

    Err(AppError::UnknownError("No response received from Cloudflare AI.".to_string()))
}

async fn call_fallback_api(messages: Vec<ChatMessage>) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(40))
        .build()
        .map_err(|e| AppError::UnknownError(format!("Network client build failed: {e}")))?;

    let payload = serde_json::json!({
        "model": FALLBACK_MODEL,
        "messages": messages,
        "max_tokens": 2048,
        "temperature": 0.7
    });

    let api_key = std::env::var("FALLBACK_API_KEY").unwrap_or_else(|_| FALLBACK_API_KEY.to_string());
    if api_key.is_empty() {
        return Err(AppError::UnknownError(
            "Fallback AI service is not configured. Please set FALLBACK_API_KEY.".to_string(),
        ));
    }

    let resp = client
        .post(FALLBACK_API_URL)
        .header("Content-Type", "application/json")
        .header("Accept-Language", "fa-IR,fa,en-US,en")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::UnknownError(format!("Failed to connect to fallback AI service: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::UnknownError(format!(
            "Fallback AI service returned error status ({status}): {body}"
        )));
    }

    let parsed: FallbackResponse = resp
        .json()
        .await
        .map_err(|e| AppError::UnknownError(format!("Failed to parse fallback AI response: {e}")))?;

    if let Some(err) = parsed.error {
        return Err(AppError::UnknownError(format!("AI error: {}", err.message)));
    }

    if let Some(choices) = parsed.choices {
        if let Some(first) = choices.into_iter().next() {
            return Ok(first.message.content.trim().to_string());
        }
    }

    Err(AppError::UnknownError("No response received from fallback AI service.".to_string()))
}

/// Dispatches AI requests to Cloudflare Workers AI with seamless secondary fallback.
async fn call_ai(messages: Vec<ChatMessage>) -> Result<String, AppError> {
    match call_cloudflare_ai(messages.clone()).await {
        Ok(reply) => Ok(reply),
        Err(err) => {
            log::warn!("Cloudflare AI request failed: {err}. Falling back to secondary provider...");
            call_fallback_api(messages).await
        }
    }
}

fn detect_gpu_and_cpu() -> (String, String) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let gpu = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_VideoController | Select-Object -ExpandProperty Name) -join ', '"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Unknown GPU".to_string());

        let cpu = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                format!("{} Cores", sys_info::cpu_num().unwrap_or(4))
            });

        (gpu, cpu)
    }
    #[cfg(not(windows))]
    {
        ("Standard Display Adapter".to_string(), format!("{} Cores", sys_info::cpu_num().unwrap_or(4)))
    }
}

fn get_installed_mods() -> Vec<String> {
    let mc_dir = get_minecraft_directory();
    let mods_dir = mc_dir.join("mods");
    let mut mods = Vec::new();

    if mods_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "jar" || ext == "disabled" {
                            if let Some(name) = path.file_name() {
                                mods.push(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    mods.sort();
    mods
}

/// Retrieve hardware and game environment diagnostics.
#[command]
pub async fn get_system_diagnostics(state: State<'_, AppState>) -> Result<SystemDiagnostics, AppError> {
    let mem = sys_info::mem_info().ok();
    let total_ram_mb = mem.as_ref().map(|m| m.total / 1024).unwrap_or(0);
    let free_ram_mb = mem.as_ref().map(|m| m.free / 1024).unwrap_or(0);

    let os = format!(
        "{} {}",
        sys_info::os_type().unwrap_or_else(|_| "Windows".to_string()),
        sys_info::os_release().unwrap_or_default()
    );

    let (gpu, cpu) = detect_gpu_and_cpu();

    let cfg = state.config.read().await;
    let allocated_ram_mb = cfg.launch_options.ram_usage_max;
    let selected_java = cfg.launch_options.java_override.clone().unwrap_or_else(|| "Auto-detect".to_string());
    let installed_mods = get_installed_mods();
    let game_version = "Minecraft (Active Instance)".to_string();

    Ok(SystemDiagnostics {
        os,
        cpu,
        total_ram_mb,
        free_ram_mb,
        gpu,
        allocated_ram_mb,
        selected_java,
        game_version,
        installed_mods,
    })
}

fn extract_search_keyword(query: &str) -> String {
    // 1. Extract any Latin/English words (like "Continuity", "Sodium", "Iris", "FerriteCore")
    let latin_words: Vec<&str> = query
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|w| w.chars().any(|c| c.is_ascii_alphabetic()))
        .collect();

    let filtered_latin: Vec<&str> = latin_words
        .into_iter()
        .filter(|w| {
            let l = w.to_lowercase();
            l != "what"
                && l != "is"
                && l != "the"
                && l != "how"
                && l != "to"
                && l != "can"
                && l != "i"
                && l != "mod"
                && l != "mods"
                && l != "for"
                && l != "and"
        })
        .collect();

    if !filtered_latin.is_empty() {
        return filtered_latin.join(" ");
    }

    // 2. Map common Persian Minecraft terms
    let lower = query.to_lowercase();
    if lower.contains("سودیوم") || lower.contains("سدیم") {
        return "sodium".to_string();
    }
    if lower.contains("ایریس") || lower.contains("آیریس") {
        return "iris".to_string();
    }
    if lower.contains("اپتیفاین") || lower.contains("اپتی‌فاین") {
        return "optifine".to_string();
    }
    if lower.contains("ایندیوم") {
        return "indium".to_string();
    }
    if lower.contains("لیتیوم") {
        return "lithium".to_string();
    }
    if lower.contains("فریت") {
        return "ferritecore".to_string();
    }
    if lower.contains("کانتینیویتی") || lower.contains("کانتینیوتی") {
        return "continuity".to_string();
    }

    query.trim().to_string()
}

#[derive(Deserialize)]
struct ManifestVersionEntry {
    id: String,
    #[serde(rename = "type")]
    version_type: String,
    url: String,
    time: String,
    #[serde(rename = "releaseTime")]
    release_time: Option<String>,
}

#[derive(Deserialize)]
struct ManifestLatestInfo {
    release: String,
    snapshot: String,
}

#[derive(Deserialize)]
struct VersionManifestV2 {
    latest: ManifestLatestInfo,
    versions: Vec<ManifestVersionEntry>,
}

#[derive(Deserialize)]
struct ClientDownloadInfo {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct PackageDownloadsInfo {
    client: Option<ClientDownloadInfo>,
}

#[derive(Deserialize)]
struct PackageDetail {
    downloads: Option<PackageDownloadsInfo>,
}

fn clean_html_snippet(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    let unescaped = result
        .replace("&#x27;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn query_ddg_html(client: &reqwest::Client, query: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    let url = match reqwest::Url::parse_with_params("https://html.duckduckgo.com/html/", &[("q", query)]) {
        Ok(u) => u,
        Err(_) => return snippets,
    };

    let resp = match client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36")
        .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header("Accept-Language", "fa-IR,fa;q=0.9,en-US;q=0.8,en;q=0.7")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return snippets,
    };

    if !resp.status().is_success() {
        return snippets;
    }

    let html = match resp.text().await {
        Ok(h) => h,
        Err(_) => return snippets,
    };

    let mut idx = 0;
    while snippets.len() < 4 {
        if let Some(pos) = html[idx..].find("class=\"result__snippet") {
            let start = idx + pos;
            if let Some(tag_close) = html[start..].find('>') {
                let text_start = start + tag_close + 1;
                if let Some(end_tag) = html[text_start..].find("</a>") {
                    let text_end = text_start + end_tag;
                    let raw = &html[text_start..text_end];
                    let clean = clean_html_snippet(raw);
                    if !clean.is_empty() {
                        snippets.push(clean);
                    }
                    idx = text_end + 4;
                    continue;
                }
            }
        }
        break;
    }

    snippets
}

async fn search_duckduckgo(client: &reqwest::Client, query: &str) -> Option<String> {
    let mut snippets = query_ddg_html(client, query).await;
    if snippets.is_empty() {
        let latin = extract_search_keyword(query);
        if !latin.is_empty() && latin != query {
            snippets = query_ddg_html(client, &latin).await;
        }
    }

    if snippets.is_empty() {
        None
    } else {
        let text = snippets
            .into_iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s))
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!("=== Live Web Search Results (DuckDuckGo Search) ===\n{text}"))
    }
}

async fn search_mojang_manifest(client: &reqwest::Client) -> Option<String> {
    let manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    let resp = client
        .get(manifest_url)
        .header("User-Agent", "GlitchyLauncher/1.2.1")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let manifest: VersionManifestV2 = resp.json().await.ok()?;

    let latest_release_id = manifest.latest.release.clone();
    let latest_snapshot_id = manifest.latest.snapshot.clone();

    // 1. Find latest release entry
    let rel_entry = manifest.versions.iter().find(|v| v.id == latest_release_id)?;
    let rel_time = rel_entry.time.clone();
    let rel_url = rel_entry.url.clone();

    let mut rel_client_sha1 = String::new();
    let mut rel_client_url = String::new();
    let mut rel_client_size = 0u64;

    if let Ok(pkg_resp) = client.get(&rel_url).header("User-Agent", "GlitchyLauncher/1.2.1").send().await {
        if pkg_resp.status().is_success() {
            if let Ok(pkg) = pkg_resp.json::<PackageDetail>().await {
                if let Some(dl) = pkg.downloads.and_then(|d| d.client) {
                    rel_client_sha1 = dl.sha1;
                    rel_client_url = dl.url;
                    rel_client_size = dl.size;
                }
            }
        }
    }

    // 2. Check the very first entry in the manifest (overall latest change/snapshot)
    let first_entry = manifest.versions.first();
    let mut first_info = String::new();
    if let Some(first) = first_entry {
        let mut first_client_sha1 = String::new();
        if let Ok(pkg_resp) = client.get(&first.url).header("User-Agent", "GlitchyLauncher/1.2.1").send().await {
            if pkg_resp.status().is_success() {
                if let Ok(pkg) = pkg_resp.json::<PackageDetail>().await {
                    if let Some(dl) = pkg.downloads.and_then(|d| d.client) {
                        first_client_sha1 = dl.sha1;
                    }
                }
            }
        }
        first_info = format!(
            "- Most Recent Entry in Manifest (Latest Dev / Snapshot): {}\n  * Type: {}\n  * Registered/Modified Time in Manifest: {}\n  * Client JAR SHA-1 Hash: {}\n",
            first.id, first.version_type, first.time, first_client_sha1
        );
    }

    let mut out = String::from("=== Mojang Official Version Manifest Live Data (piston-meta.mojang.com/mc/game/version_manifest_v2.json) ===\n");
    out.push_str(&format!(
        "- Latest Official Release: {}\n  * Last Registered/Modified Time in Manifest: {}\n  * Client JAR SHA-1 Hash: {}\n  * Client JAR Download URL: {}\n  * Client JAR Size: {} bytes\n",
        latest_release_id, rel_time, rel_client_sha1, rel_client_url, rel_client_size
    ));
    if !first_info.is_empty() {
        out.push_str(&first_info);
    }
    out.push_str(&format!("- Latest Snapshot ID: {}\n", latest_snapshot_id));
    out.push_str("\nCRITICAL RULE ON CURRENT MINECRAFT GAME VERSION:\n\
        Mojang's official manifest above is the absolute SINGLE SOURCE OF TRUTH.\n\
        - The latest official release of Minecraft Java Edition is 26.2.\n\
        - The latest snapshot/development version is 26.3-pre-2.\n\
        NEVER say 1.20, 1.21, or 1.21.9 is the current latest version! Those are previous historical versions.\n");

    Some(out)
}

fn normalize_query(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'آ' | 'أ' | 'إ' => 'ا',
            'ي' | 'ئ' => 'ی',
            'ك' => 'ک',
            'ة' => 'ه',
            'ؤ' => 'و',
            _ => c,
        })
        .collect::<String>()
        .to_lowercase()
}

async fn search_modrinth(client: &reqwest::Client, keyword: &str) -> Option<String> {
    if keyword.trim().is_empty() {
        return None;
    }

    let url = reqwest::Url::parse_with_params(
        "https://api.modrinth.com/v2/search",
        &[("query", keyword), ("limit", "3")],
    ).ok()?;

    let resp = client
        .get(url)
        .header("User-Agent", "GlitchyLauncher/1.2.1")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json = resp.json::<serde_json::Value>().await.ok()?;
    let hits = json.get("hits")?.as_array()?;
    if hits.is_empty() {
        return None;
    }

    let mut results = String::from("=== Modrinth Real-time Mod Data ===\n");
    for hit in hits.iter().take(3) {
        let title = hit.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown");
        let desc = hit.get("description").and_then(|d| d.as_str()).unwrap_or("");
        let versions = hit.get("versions").and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str()).take(3).collect::<Vec<_>>().join(", "))
            .unwrap_or_default();
        results.push_str(&format!("- {title}: {desc} (Versions: {versions})\n"));
    }

    Some(results)
}

/// Universal real-time search engine querying Mojang Official Manifest, Modrinth, and DuckDuckGo
async fn perform_web_search(query: &str) -> Option<String> {
    let clean_query = query.trim();
    if clean_query.is_empty() {
        return None;
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(4500))
        .build()
        .ok()?;

    let normalized = normalize_query(clean_query);

    let is_version_query = normalized.contains("ورژن")
        || normalized.contains("نسخه")
        || normalized.contains("اپدیت")
        || normalized.contains("اسنپ")
        || normalized.contains("مانیفست")
        || normalized.contains("manifest")
        || normalized.contains("version")
        || normalized.contains("release")
        || normalized.contains("snapshot")
        || normalized.contains("sha1")
        || normalized.contains("sha-1")
        || normalized.contains("piston")
        || normalized.contains("جدیدترین")
        || (normalized.contains("اخرین") && (normalized.contains("بازی") || normalized.contains("ماینکرفت") || normalized.contains("ماینکرافت")));

    let is_mod_query = normalized.contains("mod")
        || normalized.contains("ماد")
        || normalized.contains("shader")
        || normalized.contains("شیدر")
        || normalized.contains("fabric")
        || normalized.contains("فبریک")
        || normalized.contains("forge")
        || normalized.contains("فورج")
        || normalized.contains("sodium")
        || normalized.contains("سود")
        || normalized.contains("iris")
        || normalized.contains("ایریس")
        || normalized.contains("continuity")
        || normalized.contains("کانتینیویتی")
        || normalized.contains("optifine")
        || normalized.contains("اپتیفاین");

    let mut combined_results = Vec::new();

    // 1. Mojang Official Version Manifest & Client SHA-1 lookup
    if is_version_query {
        log::info!("[AI Web Search] Detected version/manifest query ('{}'), querying piston-meta live data...", clean_query);
        if let Some(manifest_data) = search_mojang_manifest(&client).await {
            log::info!("[AI Web Search] Successfully fetched live Mojang manifest data.");
            combined_results.push(manifest_data);
        }
    }

    // 2. Modrinth Mod/Shader search
    if is_mod_query {
        let search_keyword = extract_search_keyword(clean_query);
        log::info!("[AI Web Search] Detected mod query, querying Modrinth for '{}'...", search_keyword);
        if let Some(mod_data) = search_modrinth(&client, &search_keyword).await {
            log::info!("[AI Web Search] Successfully fetched live Modrinth data.");
            combined_results.push(mod_data);
        }
    }

    // 3. DuckDuckGo global web search (runs for general queries, avoiding version queries to prevent outdated search snippets)
    if combined_results.is_empty() || (!is_version_query && !is_mod_query) {
        log::info!("[AI Web Search] Querying DuckDuckGo HTML for '{}'...", clean_query);
        if let Some(ddg_data) = search_duckduckgo(&client, clean_query).await {
            log::info!("[AI Web Search] Successfully fetched DuckDuckGo web results.");
            combined_results.push(ddg_data);
        }
    }

    if combined_results.is_empty() {
        // Fallback: try DuckDuckGo search anyway
        log::info!("[AI Web Search] Fallback DuckDuckGo query for '{}'...", clean_query);
        if let Some(ddg_data) = search_duckduckgo(&client, clean_query).await {
            combined_results.push(ddg_data);
        }
    }

    if combined_results.is_empty() {
        None
    } else {
        Some(combined_results.join("\n\n"))
    }
}

/// Send a multi-turn chat message to Glitchy AI with optional system context and live web search.
#[command]
pub async fn ai_chat(
    mut messages: Vec<ChatMessage>,
    system_context: Option<String>,
) -> Result<String, AppError> {
    // 1. Enforce 20 requests per 5-hour limit
    check_and_consume_rate_limit()?;

    let mut base_system_prompt = get_system_chat_prompt();
    if let Some(ctx) = system_context {
        if !ctx.trim().is_empty() {
            base_system_prompt.push_str("\n\nUser System & Game Context:\n");
            base_system_prompt.push_str(&ctx);
        }
    }

    // Check if we can perform a live web search for the user's latest query
    let last_user_query = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    if !last_user_query.trim().is_empty() {
        if let Some(search_info) = perform_web_search(&last_user_query).await {
            log::info!("[AI Web Search] Injected real-time search context into prompt for user query: '{}'", &last_user_query);
            base_system_prompt.push_str("\n\n[Live Web Search Context]:\n");
            base_system_prompt.push_str(&search_info);
            base_system_prompt.push_str("\nCRITICAL DIRECTIVE: The above [Live Web Search Context] was just fetched live from the internet in real time. Use these real-time search facts naturally to answer accurately. DO NOT append any repetitive footer lines, links, or verification disclaimers to your message.\n");
        }
    }

    if messages.is_empty() || messages[0].role != "system" {
        messages.insert(
            0,
            ChatMessage {
                role: "system".to_string(),
                content: base_system_prompt,
            },
        );
    } else {
        messages[0].content = base_system_prompt;
    }

    call_ai(messages).await
}

/// Helper function to find the most recent crash file or latest.log
fn find_latest_crash_or_log() -> Result<(String, String), AppError> {
    let mc_dir = get_minecraft_directory();
    let crash_dir = mc_dir.join("crash-reports");

    // 1. Check crash-reports directory for newest file
    if crash_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&crash_dir) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "txt"))
                .collect();

            files.sort_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });

            if let Some(latest) = files.last() {
                if let Ok(metadata) = fs::metadata(latest) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = modified.elapsed() {
                            // If crash report was generated within the last 4 hours, prioritize it
                            if elapsed < Duration::from_secs(4 * 3600) {
                                if let Ok(content) = fs::read_to_string(latest) {
                                    let snippet = if content.len() > 3500 {
                                        content[content.len() - 3500..].to_string()
                                    } else {
                                        content
                                    };
                                    return Ok((snippet, "CRASH_REPORT".to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Check logs/latest.log for Fabric/Forge loader crashes or errors
    let latest_log = mc_dir.join("logs").join("latest.log");
    if latest_log.is_file() {
        if let Ok(content) = fs::read_to_string(&latest_log) {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 150 { lines.len() - 150 } else { 0 };
            let snippet = lines[start..].join("\n");

            let lower = snippet.to_lowercase();
            // Check for Fabric / Forge mod-loader pre-launch signatures
            if lower.contains("formattedexception")
                || lower.contains("incompatible mod set")
                || lower.contains("mixin apply failed")
                || lower.contains("could not find required mod")
                || lower.contains("missing or unsupported mandatory dependencies")
                || lower.contains("modloadingexception")
                || lower.contains("duplicatemodsexception")
            {
                return Ok((snippet, "MOD_LOADER_CRASH".to_string()));
            }

            return Ok((snippet, "LATEST_LOG".to_string()));
        }
    }

    Err(AppError::FileNotFound(
        "No crash report or game log found in the Minecraft directory.".to_string(),
    ))
}

/// Analyze a Minecraft crash report or log using offline heuristics + Glitchy AI.
#[command]
pub async fn ai_diagnose_log(
    manual_log: Option<String>,
    system_context: Option<String>,
) -> Result<AiDiagnosisResult, AppError> {
    // Shared rate limit: consumes 1 request
    check_and_consume_rate_limit()?;

    let (raw_log, source) = match manual_log {
        Some(text) if !text.trim().is_empty() => (text, "MANUAL".to_string()),
        _ => find_latest_crash_or_log()?,
    };

    // --- Offline Heuristic Classification ---
    let lower = raw_log.to_lowercase();
    let (issue_type, auto_fix_available, auto_fix_desc) = if lower.contains("outofmemory")
        || lower.contains("heap space")
        || lower.contains("java heap")
    {
        (
            "OUT_OF_MEMORY".to_string(),
            true,
            Some("افزایش خودکار رم اختصاص‌یافته به بازی به ۴ گیگابایت (4096MB)".to_string()),
        )
    } else if lower.contains("unsupportedclassversionerror")
        || lower.contains("has been compiled by a more recent version of the java")
    {
        (
            "JAVA_VERSION".to_string(),
            true,
            Some("تنظیم خودکار نسخه جاوا متناسب با نسخه ماینکرفت".to_string()),
        )
    } else if lower.contains("glfw error 65542")
        || lower.contains("wgl: the driver does not appear to support opengl")
    {
        (
            "GRAPHICS_DRIVER".to_string(),
            true,
            Some("پشتیبان‌گیری و ریست تنظیمات گرافیکی بازی (options.txt)".to_string()),
        )
    } else if lower.contains("duplicatemodsexception")
        || lower.contains("modresolutionexception")
        || lower.contains("incompatiblemodsexception")
        || lower.contains("incompatible mod set")
        || lower.contains("mixin apply failed")
        || lower.contains("could not find required mod")
        || lower.contains("missing or unsupported mandatory dependencies")
    {
        (
            "MOD_CONFLICT".to_string(),
            false,
            None,
        )
    } else {
        (
            "UNKNOWN".to_string(),
            false,
            None,
        )
    };

    // --- Prepare AI Prompt ---
    let mut prompt_content = String::new();
    if let Some(ctx) = system_context {
        if !ctx.trim().is_empty() {
            prompt_content.push_str("مشخصات سیستم و محیط ماینکرفت کاربر:\n");
            prompt_content.push_str(&ctx);
            prompt_content.push_str("\n\n");
        }
    }

    prompt_content.push_str(&format!(
        "این لاگ خطای کرش ماینکرفت است ({source}). لطفاً علت دقیق کرش را بررسی کن و در ۲ الی ۴ گام ساده و شفاف به زبان فارسی صمیمی توضیح بده کاربر دقیقاً چه کار باید بکند:\n\n```\n{}\n```",
        if raw_log.len() > 3200 {
            &raw_log[raw_log.len() - 3200..]
        } else {
            &raw_log
        }
    ));

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: get_system_diagnostic_prompt(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: prompt_content,
        },
    ];

    let ai_explanation = match call_ai(messages).await {
        Ok(explanation) => explanation,
        Err(e) => format!("خطا در ارتباط با هوش مصنوعی: {e}. (تشخیص آفلاین: نوع خطای احتمالی {issue_type} است)"),
    };

    Ok(AiDiagnosisResult {
        log_snippet: if raw_log.len() > 1500 {
            raw_log[raw_log.len() - 1500..].to_string()
        } else {
            raw_log
        },
        ai_explanation,
        detected_issue_type: issue_type,
        auto_fix_available,
        auto_fix_description: auto_fix_desc,
        source,
    })
}

/// Automatically apply a fix for a recognized issue.
#[command]
pub async fn ai_apply_autofix(
    state: State<'_, AppState>,
    action: String,
) -> Result<String, AppError> {
    match action.as_str() {
        "OUT_OF_MEMORY" => {
            let mut cfg = state.config.write().await;
            cfg.launch_options.ram_usage_max = 4096;
            cfg.write_to_file()?;
            Ok("رم اختصاص‌یافته به بازی با موفقیت به ۴۰۹۶ مگابایت افزایش یافت.".to_string())
        }
        "JAVA_VERSION" => {
            let mut cfg = state.config.write().await;
            cfg.launch_options.java_override = None; // Auto-select recommended runtime
            cfg.write_to_file()?;
            Ok("تنظیمات جاوا روی حالت انتخاب خودکار قرار گرفت تا نسخه سازگار لود شود.".to_string())
        }
        "GRAPHICS_DRIVER" => {
            let mc_dir = get_minecraft_directory();
            let options_path = mc_dir.join("options.txt");
            if options_path.exists() {
                let backup_path = mc_dir.join("options.txt.bak");
                let _ = fs::copy(&options_path, &backup_path);
                let _ = fs::remove_file(&options_path);
                Ok("فایل تنظیمات گرافیک بازی (options.txt) ریست شد و نسخه پشتیبان ساخته شد.".to_string())
            } else {
                Ok("فایل تنظیمات گرافیک یافت نشد یا قبلاً ریست شده بود.".to_string())
            }
        }
        _ => Err(AppError::UnknownError(format!(
            "No automatic fix available for this issue type: {action}"
        ))),
    }
}
