use std::path::PathBuf;
use std::time::Duration;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{command, State};

use crate::models::error::AppError;
use crate::services::directory_manager::get_falcon_launcher_directory;
use crate::AppState;

const PRIMARY_API_URL: &str = "https://api.glitchyteam.ir";

// Anti-Disposable Email Blacklist
const DISPOSABLE_EMAIL_DOMAINS: &[&str] = &[
    "10minutemail.com",
    "10minutemail.net",
    "temp-mail.org",
    "tempmail.com",
    "tempmail.net",
    "guerrillamail.com",
    "guerrillamail.net",
    "guerrillamail.biz",
    "guerrillamailblock.com",
    "mailinator.com",
    "yopmail.com",
    "throwawaymail.com",
    "fakemail.net",
    "trashmail.com",
    "trashmail.net",
    "trashmail.me",
    "getairmail.com",
    "dispostable.com",
    "crazymailing.com",
    "nada.ltd",
    "mohmal.com",
    "emailondeck.com",
    "sharklasers.com",
    "grr.la",
    "burnermail.io",
    "dropmail.me",
    "tempail.com",
    "mytemp.email",
    "fakemailgenerator.com",
    "zillamail.com",
    "mailsac.com",
    "spambox.us",
    "inboxbear.com",
    "generator.email",
    "disposablemail.com",
    "armyspy.com",
    "cuvox.de",
    "dayrep.com",
    "fleckens.hu",
    "gustr.com",
    "jourrapide.com",
    "rhyta.com",
    "superrito.com",
    "teleworm.us",
    "tinmail.net",
];

fn is_disposable_email(email: &str) -> bool {
    let domain = match email.split('@').nth(1) {
        Some(d) => d.trim().to_lowercase(),
        None => return false,
    };
    DISPOSABLE_EMAIL_DOMAINS.iter().any(|d| domain == *d || domain.ends_with(&format!(".{d}")))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GlitchyUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub badge: String,
    pub skin_data: Option<String>,
    pub skin_model: String,
    pub cape_data: Option<String>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GlitchyAuthResponse {
    pub success: bool,
    pub token: Option<String>,
    pub user: Option<GlitchyUser>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct SavedSession {
    token: String,
    user: GlitchyUser,
    saved_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct LocalAccountRecord {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub role: String,
    pub badge: String,
    pub skin_data: Option<String>,
    pub skin_model: String,
    pub cape_data: Option<String>,
    pub created_at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct LocalVault {
    pub accounts: Vec<LocalAccountRecord>,
}

fn get_session_file_path() -> PathBuf {
    get_falcon_launcher_directory().join("glitchy_account_session.json")
}

fn get_vault_file_path() -> PathBuf {
    get_falcon_launcher_directory().join("glitchy_accounts_vault.json")
}

fn get_sync_file_path() -> PathBuf {
    get_falcon_launcher_directory().join("glitchy_account_sync.json")
}

fn load_vault() -> LocalVault {
    let path = get_vault_file_path();
    if !path.exists() {
        return LocalVault::default();
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn save_vault(vault: &LocalVault) {
    let path = get_vault_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(vault) {
        let _ = std::fs::write(path, json);
    }
}

fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn load_saved_session() -> Option<SavedSession> {
    let path = get_session_file_path();
    if !path.exists() {
        return None;
    }
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_session(token: &str, user: &GlitchyUser) {
    let path = get_session_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let sess = SavedSession {
        token: token.to_string(),
        user: user.clone(),
        saved_at: now,
    };
    if let Ok(json) = serde_json::to_string_pretty(&sess) {
        let _ = std::fs::write(path, json);
    }
}

fn clear_session() {
    let path = get_session_file_path();
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

fn build_client(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn post_json<T: Serialize, R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    body: &T,
    token: Option<&str>,
) -> Result<R, AppError> {
    let client = build_client(3);
    let url = format!("{}{}", PRIMARY_API_URL, endpoint);
    let mut req = client.post(&url).header(USER_AGENT, "GlitchyLauncher/1.3.1");
    if let Some(tok) = token {
        req = req.header(AUTHORIZATION, format!("Bearer {}", tok));
    }
    let resp = req.json(body).send().await.map_err(|e| AppError::UnknownError(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() || status.as_u16() == 400 || status.as_u16() == 409 || status.as_u16() == 401 {
        if let Ok(parsed) = serde_json::from_str::<R>(&text) {
            return Ok(parsed);
        }
    }
    Err(AppError::UnknownError(format!("Server returned {}: {}", status, text)))
}

async fn get_json<R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    token: Option<&str>,
) -> Result<R, AppError> {
    let client = build_client(3);
    let url = format!("{}{}", PRIMARY_API_URL, endpoint);
    let mut req = client.get(&url).header(USER_AGENT, "GlitchyLauncher/1.3.1");
    if let Some(tok) = token {
        req = req.header(AUTHORIZATION, format!("Bearer {}", tok));
    }
    let resp = req.send().await.map_err(|e| AppError::UnknownError(e.to_string()))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if status.is_success() || status.as_u16() == 400 || status.as_u16() == 401 {
        if let Ok(parsed) = serde_json::from_str::<R>(&text) {
            return Ok(parsed);
        }
    }
    Err(AppError::UnknownError(format!("Server returned {}: {}", status, text)))
}

async fn sync_with_launcher_profile(state: &State<'_, AppState>, username: &str) {
    if let Ok(profile) = crate::models::profiles::create_new_profile(username.to_string(), false) {
        let mut cfg = state.config.write().await;
        cfg.launch_options.selected_profile = profile.uuid;
        let _ = cfg.write_to_file();
    }
}

#[command]
pub async fn glitchy_account_register(
    state: State<'_, AppState>,
    username: String,
    email: String,
    password: String,
) -> Result<GlitchyAuthResponse, AppError> {
    let u = username.trim().to_string();
    let em = email.trim().to_lowercase();

    if u.len() < 3 || u.len() > 16 || !u.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("نام کاربری باید ۳ تا ۱۶ حرف و فقط شامل حروف انگلیسی، اعداد یا زیرخط باشد.".to_string()),
        });
    }

    if !em.contains('@') || !em.contains('.') {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("لطفاً یک آدرس ایمیل معتبر وارد کنید.".to_string()),
        });
    }

    if is_disposable_email(&em) {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("استفاده از ایمیل‌های موقت (فیک) مجاز نمی‌باشد. لطفاً از ایمیل اصلی خود استفاده کنید.".to_string()),
        });
    }

    if password.len() < 6 {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("رمز عبور باید حداقل ۶ کاراکتر باشد.".to_string()),
        });
    }

    #[derive(Serialize)]
    struct RegBody {
        username: String,
        email: String,
        password: String,
    }

    let body = RegBody {
        username: u.clone(),
        email: em.clone(),
        password: password.clone(),
    };

    // Try remote API first if configured
    if let Ok(res) = post_json::<RegBody, GlitchyAuthResponse>("/api/auth/register", &body, None).await {
        if res.success {
            if let (Some(token), Some(user)) = (&res.token, &res.user) {
                save_session(token, user);
                sync_with_launcher_profile(&state, &user.username).await;
            }
        }
        return Ok(res);
    }

    // Seamless Local Vault fallback (no server / offline / no domain required)
    log::info!("Remote API unavailable, registering in local Glitchy vault...");
    let mut vault = load_vault();
    if vault.accounts.iter().any(|a| a.username.eq_ignore_ascii_case(&u)) {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("این نام کاربری قبلاً ثبت شده است.".to_string()),
        });
    }
    if vault.accounts.iter().any(|a| a.email.eq_ignore_ascii_case(&em)) {
        return Ok(GlitchyAuthResponse {
            success: false,
            token: None,
            user: None,
            error: Some("این آدرس ایمیل قبلاً ثبت شده است.".to_string()),
        });
    }

    let salt = uuid::Uuid::new_v4().to_string();
    let password_hash = hash_password(&password, &salt);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let user_id = uuid::Uuid::new_v4().to_string();

    let record = LocalAccountRecord {
        id: user_id.clone(),
        username: u.clone(),
        email: em.clone(),
        password_hash,
        salt,
        role: "user".to_string(),
        badge: "عضو گلیچی".to_string(),
        skin_data: None,
        skin_model: "classic".to_string(),
        cape_data: None,
        created_at: now,
    };

    vault.accounts.push(record.clone());
    save_vault(&vault);

    let token = format!("local_{}", uuid::Uuid::new_v4());
    let user = GlitchyUser {
        id: record.id,
        username: record.username,
        email: record.email,
        role: record.role,
        badge: record.badge,
        skin_data: record.skin_data,
        skin_model: record.skin_model,
        cape_data: record.cape_data,
        created_at: record.created_at,
    };

    save_session(&token, &user);
    sync_with_launcher_profile(&state, &user.username).await;

    Ok(GlitchyAuthResponse {
        success: true,
        token: Some(token),
        user: Some(user),
        error: None,
    })
}

#[command]
pub async fn glitchy_account_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Result<GlitchyAuthResponse, AppError> {
    let clean_login = login.trim().to_string();

    #[derive(Serialize)]
    struct LoginBody {
        login: String,
        password: String,
    }

    let body = LoginBody {
        login: clean_login.clone(),
        password: password.clone(),
    };

    // Try remote API first
    if let Ok(res) = post_json::<LoginBody, GlitchyAuthResponse>("/api/auth/login", &body, None).await {
        if res.success {
            if let (Some(token), Some(user)) = (&res.token, &res.user) {
                save_session(token, user);
                sync_with_launcher_profile(&state, &user.username).await;
            }
        }
        return Ok(res);
    }

    // Seamless Local Vault fallback
    log::info!("Remote API unavailable, attempting local vault login...");
    let vault = load_vault();
    let account = vault.accounts.iter().find(|a| {
        a.username.eq_ignore_ascii_case(&clean_login) || a.email.eq_ignore_ascii_case(&clean_login)
    });

    match account {
        Some(acc) => {
            let hashed = hash_password(&password, &acc.salt);
            if hashed == acc.password_hash {
                let token = format!("local_{}", uuid::Uuid::new_v4());
                let user = GlitchyUser {
                    id: acc.id.clone(),
                    username: acc.username.clone(),
                    email: acc.email.clone(),
                    role: acc.role.clone(),
                    badge: acc.badge.clone(),
                    skin_data: acc.skin_data.clone(),
                    skin_model: acc.skin_model.clone(),
                    cape_data: acc.cape_data.clone(),
                    created_at: acc.created_at,
                };
                save_session(&token, &user);
                sync_with_launcher_profile(&state, &user.username).await;
                Ok(GlitchyAuthResponse {
                    success: true,
                    token: Some(token),
                    user: Some(user),
                    error: None,
                })
            } else {
                Ok(GlitchyAuthResponse {
                    success: false,
                    token: None,
                    user: None,
                    error: Some("رمز عبور وارد شده نادرست است.".to_string()),
                })
            }
        }
        None => {
            Ok(GlitchyAuthResponse {
                success: false,
                token: None,
                user: None,
                error: Some("حساب کاربری با این نام کاربری یا ایمیل یافت نشد.".to_string()),
            })
        }
    }
}

#[command]
pub async fn glitchy_account_logout() -> Result<(), AppError> {
    if let Some(session) = load_saved_session() {
        if !session.token.starts_with("local_") {
            #[derive(Serialize)]
            struct EmptyBody {}
            let _: Result<serde_json::Value, _> =
                post_json("/api/auth/logout", &EmptyBody {}, Some(&session.token)).await;
        }
    }
    clear_session();
    Ok(())
}

#[command]
pub async fn glitchy_account_get_current() -> Result<Option<GlitchyUser>, AppError> {
    let saved = match load_saved_session() {
        Some(s) => s,
        None => return Ok(None),
    };

    if saved.token.starts_with("local_") {
        return Ok(Some(saved.user));
    }

    #[derive(Deserialize)]
    struct MeResponse {
        success: bool,
        user: Option<GlitchyUser>,
    }

    match get_json::<MeResponse>("/api/auth/me", Some(&saved.token)).await {
        Ok(res) => {
            if res.success && res.user.is_some() {
                let u = res.user.unwrap();
                save_session(&saved.token, &u);
                Ok(Some(u))
            } else {
                clear_session();
                Ok(None)
            }
        }
        Err(_) => {
            // Offline fallback: allow cached user session
            Ok(Some(saved.user))
        }
    }
}

#[command]
pub async fn glitchy_account_upload_skin(
    skin_data: String,
    model: String,
    cape_data: Option<String>,
) -> Result<(), AppError> {
    let saved = load_saved_session().ok_or_else(|| {
        AppError::UnknownError("لطفاً ابتدا وارد حساب کاربری گلیچی شوید".to_string())
    })?;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SkinBody {
        skin_data: String,
        model: String,
        cape_data: Option<String>,
    }

    let body = SkinBody {
        skin_data: skin_data.clone(),
        model: model.clone(),
        cape_data: cape_data.clone(),
    };

    if !saved.token.starts_with("local_") {
        #[derive(Deserialize)]
        struct SkinRes {
            success: bool,
        }
        let _: Result<SkinRes, _> = post_json("/api/skins/upload", &body, Some(&saved.token)).await;
    }

    let mut updated_user = saved.user;
    updated_user.skin_data = Some(skin_data.clone());
    updated_user.skin_model = model.clone();
    updated_user.cape_data = cape_data.clone();
    save_session(&saved.token, &updated_user);

    let mut vault = load_vault();
    if let Some(acc) = vault.accounts.iter_mut().find(|a| a.id == updated_user.id) {
        acc.skin_data = Some(skin_data);
        acc.skin_model = model;
        acc.cape_data = cape_data;
        save_vault(&vault);
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Default)]
struct LocalSyncData {
    settings: Option<String>,
    instances: Option<String>,
    updated_at: i64,
}

#[command]
pub async fn glitchy_account_sync_save(
    settings_json: Option<String>,
    instances_json: Option<String>,
) -> Result<(), AppError> {
    let saved = match load_saved_session() {
        Some(s) => s,
        None => return Ok(()),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let sync_data = LocalSyncData {
        settings: settings_json.clone(),
        instances: instances_json.clone(),
        updated_at: now,
    };
    if let Ok(json) = serde_json::to_string_pretty(&sync_data) {
        let _ = std::fs::write(get_sync_file_path(), json);
    }

    if !saved.token.starts_with("local_") {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SyncBody {
            settings_json: Option<String>,
            instances_json: Option<String>,
        }

        let body = SyncBody {
            settings_json,
            instances_json,
        };

        #[derive(Deserialize)]
        struct SyncRes {
            success: bool,
        }

        let _: Result<SyncRes, _> = post_json("/api/sync", &body, Some(&saved.token)).await;
    }

    Ok(())
}

#[command]
pub async fn glitchy_account_sync_load() -> Result<(Option<String>, Option<String>), AppError> {
    let saved = match load_saved_session() {
        Some(s) => s,
        None => return Ok((None, None)),
    };

    if !saved.token.starts_with("local_") {
        #[derive(Deserialize)]
        struct SyncRes {
            success: bool,
            settings: Option<String>,
            instances: Option<String>,
        }

        if let Ok(res) = get_json::<SyncRes>("/api/sync", Some(&saved.token)).await {
            if res.success {
                return Ok((res.settings, res.instances));
            }
        }
    }

    // Load local sync
    let sync_path = get_sync_file_path();
    if sync_path.exists() {
        if let Ok(data) = std::fs::read_to_string(sync_path) {
            if let Ok(local) = serde_json::from_str::<LocalSyncData>(&data) {
                return Ok((local.settings, local.instances));
            }
        }
    }

    Ok((None, None))
}
