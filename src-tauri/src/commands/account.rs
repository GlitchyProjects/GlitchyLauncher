use std::path::PathBuf;
use std::time::Duration;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use tauri::{command, State};

use crate::models::error::AppError;
use crate::services::directory_manager::get_falcon_launcher_directory;
use crate::AppState;

const PRIMARY_API_URL: &str = "https://api.glitchyteam.ir";
const FALLBACK_API_URL: &str = "https://glitchy-api.hosun1451.workers.dev";

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

fn get_session_file_path() -> PathBuf {
    get_falcon_launcher_directory().join("glitchy_account_session.json")
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
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

async fn post_json<T: Serialize, R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    body: &T,
    token: Option<&str>,
) -> Result<R, AppError> {
    let client = build_client(10);
    let urls = [
        format!("{}{}", PRIMARY_API_URL, endpoint),
        format!("{}{}", FALLBACK_API_URL, endpoint),
    ];

    let mut last_err = String::from("Network error");
    for url in urls {
        let mut req = client.post(&url).header(USER_AGENT, "GlitchyLauncher");
        if let Some(tok) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", tok));
        }
        match req.json(body).send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() || status.as_u16() == 400 || status.as_u16() == 409 || status.as_u16() == 401 {
                    if let Ok(parsed) = serde_json::from_str::<R>(&text) {
                        return Ok(parsed);
                    }
                }
                last_err = format!("Server returned {}: {}", status, text);
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(AppError::UnknownError(last_err))
}

async fn get_json<R: for<'de> Deserialize<'de>>(
    endpoint: &str,
    token: Option<&str>,
) -> Result<R, AppError> {
    let client = build_client(6);
    let urls = [
        format!("{}{}", PRIMARY_API_URL, endpoint),
        format!("{}{}", FALLBACK_API_URL, endpoint),
    ];

    let mut last_err = String::from("Network error");
    for url in urls {
        let mut req = client.get(&url).header(USER_AGENT, "GlitchyLauncher");
        if let Some(tok) = token {
            req = req.header(AUTHORIZATION, format!("Bearer {}", tok));
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                if status.is_success() || status.as_u16() == 400 || status.as_u16() == 401 {
                    if let Ok(parsed) = serde_json::from_str::<R>(&text) {
                        return Ok(parsed);
                    }
                }
                last_err = format!("Server returned {}: {}", status, text);
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(AppError::UnknownError(last_err))
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
    #[derive(Serialize)]
    struct RegBody {
        username: String,
        email: String,
        password: String,
    }

    let body = RegBody {
        username: username.trim().to_string(),
        email: email.trim().to_string(),
        password,
    };

    let res: GlitchyAuthResponse = post_json("/api/auth/register", &body, None).await?;
    if res.success {
        if let (Some(token), Some(user)) = (&res.token, &res.user) {
            save_session(token, user);
            sync_with_launcher_profile(&state, &user.username).await;
        }
    }
    Ok(res)
}

#[command]
pub async fn glitchy_account_login(
    state: State<'_, AppState>,
    login: String,
    password: String,
) -> Result<GlitchyAuthResponse, AppError> {
    #[derive(Serialize)]
    struct LoginBody {
        login: String,
        password: String,
    }

    let body = LoginBody {
        login: login.trim().to_string(),
        password,
    };

    let res: GlitchyAuthResponse = post_json("/api/auth/login", &body, None).await?;
    if res.success {
        if let (Some(token), Some(user)) = (&res.token, &res.user) {
            save_session(token, user);
            sync_with_launcher_profile(&state, &user.username).await;
        }
    }
    Ok(res)
}

#[command]
pub async fn glitchy_account_logout() -> Result<(), AppError> {
    if let Some(session) = load_saved_session() {
        #[derive(Serialize)]
        struct EmptyBody {}
        let _: Result<serde_json::Value, _> =
            post_json("/api/auth/logout", &EmptyBody {}, Some(&session.token)).await;
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
            // Offline mode: allow play using cached session!
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
        skin_data,
        model,
        cape_data,
    };

    #[derive(Deserialize)]
    struct SkinRes {
        success: bool,
        error: Option<String>,
    }

    let res: SkinRes = post_json("/api/skins/upload", &body, Some(&saved.token)).await?;
    if !res.success {
        return Err(AppError::UnknownError(
            res.error.unwrap_or_else(|| "خطا در آپلود اسکین".to_string()),
        ));
    }

    let mut updated_user = saved.user;
    updated_user.skin_data = Some(body.skin_data);
    updated_user.skin_model = body.model;
    updated_user.cape_data = body.cape_data;
    save_session(&saved.token, &updated_user);

    Ok(())
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
    Ok(())
}

#[command]
pub async fn glitchy_account_sync_load() -> Result<(Option<String>, Option<String>), AppError> {
    let saved = match load_saved_session() {
        Some(s) => s,
        None => return Ok((None, None)),
    };

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

    Ok((None, None))
}
