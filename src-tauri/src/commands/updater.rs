use std::time::Duration;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use tauri::command;

use crate::models::error::AppError;
use crate::LAUNCHER_VERSION;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LauncherUpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub download_url: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize, Debug)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Option<Vec<GithubAsset>>,
}

fn parse_semver(v: &str) -> Vec<u32> {
    v.trim_start_matches('v')
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect()
}

fn is_version_newer(latest: &str, current: &str) -> bool {
    let l_parts = parse_semver(latest);
    let c_parts = parse_semver(current);
    l_parts > c_parts
}

#[command]
pub async fn check_launcher_update() -> Result<LauncherUpdateInfo, AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| AppError::UnknownError(e.to_string()))?;

    // Check official GitHub releases
    let repo_url = "https://api.github.com/repos/GlitchyProjects/GlitchyLauncher/releases/latest";
    let resp = client
        .get(repo_url)
        .header(USER_AGENT, "GlitchyLauncher")
        .send()
        .await;

    if let Ok(res) = resp {
        if res.status().is_success() {
            if let Ok(release) = res.json::<GithubRelease>().await {
                let latest_tag = release.tag_name.trim_start_matches('v').to_string();
                let has_update = is_version_newer(&latest_tag, LAUNCHER_VERSION);
                let download_url = release.assets.and_then(|assets| {
                    assets
                        .into_iter()
                        .find(|a| a.name.ends_with(".exe") || a.name.ends_with(".msi"))
                        .map(|a| a.browser_download_url)
                });

                return Ok(LauncherUpdateInfo {
                    has_update,
                    current_version: LAUNCHER_VERSION.to_string(),
                    latest_version: latest_tag,
                    release_url: release.html_url,
                    release_notes: release.body.unwrap_or_default(),
                    download_url,
                });
            }
        }
    }

    // Fallback when GitHub API is rate-limited or offline
    Ok(LauncherUpdateInfo {
        has_update: false,
        current_version: LAUNCHER_VERSION.to_string(),
        latest_version: LAUNCHER_VERSION.to_string(),
        release_url: "https://github.com/GlitchyProjects/GlitchyLauncher/releases".to_string(),
        release_notes: String::new(),
        download_url: None,
    })
}

#[command]
pub async fn apply_launcher_update(download_url: String) -> Result<(), AppError> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::UnknownError(e.to_string()))?;

    let resp = client
        .get(&download_url)
        .header(USER_AGENT, "GlitchyLauncher")
        .send()
        .await
        .map_err(|e| AppError::UnknownError(format!("خطا در دریافت فایل آپدیت: {}", e)))?;

    if !resp.status().is_success() {
        return Err(AppError::UnknownError(format!("سرور کد خطا بازگرداند: {}", resp.status())));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::UnknownError(format!("خطا در دانلود فایل آپدیت: {}", e)))?;

    let temp_dir = std::env::temp_dir();
    let file_name = if download_url.ends_with(".msi") {
        "GlitchyLauncher_Setup.msi"
    } else {
        "GlitchyLauncher_Setup.exe"
    };
    let update_file_path = temp_dir.join(file_name);

    std::fs::write(&update_file_path, bytes)
        .map_err(|e| AppError::UnknownError(format!("خطا در ذخیره فایل آپدیت: {}", e)))?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        const DETACHED_PROCESS: u32 = 0x00000008;

        std::process::Command::new(&update_file_path)
            .creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS)
            .spawn()
            .map_err(|e| AppError::UnknownError(format!("خطا در اجرای فایل آپدیت: {}", e)))?;

        std::process::exit(0);
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(&update_file_path)
            .spawn()
            .map_err(|e| AppError::UnknownError(format!("خطا در اجرای فایل آپدیت: {}", e)))?;
        std::process::exit(0);
    }
}

