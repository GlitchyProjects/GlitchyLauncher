use std::sync::Arc;

use log::{info, warn};
use tauri::{command, AppHandle, State};
use tokio::sync::{mpsc::UnboundedSender, RwLock};

use crate::models::config::Config;
use crate::models::downloader::{OptifineVersion, VersionInfo, VersionLoader};
use crate::models::error::AppError;
use crate::models::logger::LogLine;
use crate::models::mirror;
use crate::models::versions::VersionBase::{FABRIC, FORGE, NEOFORGE, OPTIFINE};
use crate::models::versions::{MinecraftVersion, VersionBase, VersionCategory, VersionType};
use crate::services::download_session::{self, SessionHandle, SessionInfo};
use crate::services::game_downloader;
use crate::services::game_downloader::{
    download_fabric, download_forge_version, get_available_fabric_versions,
    get_available_forge_versions,
};
use crate::services::http;
use crate::services::repair::{repair_version as do_repair, RepairReport};
use crate::services::version_manager::{load_version_manifest, reload_installed_versions};
use crate::{AppState, GLOBAL_CACHE};

#[command]
pub async fn get_vanilla_versions(
    _app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<VersionCategory>, AppError> {
    let cfg = state.config.read().await;
    let preferred = cfg.download_settings.mirror.clone();
    drop(cfg);
    let mirror = mirror::resolve_effective(&preferred).await;
    let manifest = load_version_manifest(&mirror).await?;
    let mut result: Vec<VersionCategory> = Vec::new();
    let versions: Vec<&VersionInfo> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::Release))
        .collect();
    let snapshots: Vec<VersionLoader> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::Snapshot))
        .map(|x| VersionLoader {
            id: x.id.to_string(),
            base: VersionBase::VANILLA,
            date: x.time.to_string(),
        })
        .collect();
    let old_beta: Vec<VersionLoader> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::OldBeta))
        .map(|x| VersionLoader {
            id: x.id.to_string(),
            base: VersionBase::VANILLA,
            date: x.time.to_string(),
        })
        .collect();
    let old_alpha: Vec<VersionLoader> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::OldAlpha))
        .map(|x| VersionLoader {
            id: x.id.to_string(),
            base: VersionBase::VANILLA,
            date: x.time.to_string(),
        })
        .collect();

    for ver in versions {
        let id = ver.id.clone();
        let category = version_category(&id);
        if result.iter_mut().find(|x| x.name == category).is_none() {
            result.push(VersionCategory {
                name: category.clone(),
                versions: Vec::new(),
            });
        }
        let cat = result.iter_mut().find(|x| x.name == category).unwrap();
        cat.versions.push(VersionLoader {
            id: id.clone(),
            base: VersionBase::VANILLA,
            date: ver.release_time.clone(),
        });
    }
    for cat in &mut result {
        cat.versions.sort_by(|left, right| {
            minecraft_semver_key(&right.id).cmp(&minecraft_semver_key(&left.id))
        });
    }
    result.sort_by(|left, right| {
        minecraft_semver_key(&right.name).cmp(&minecraft_semver_key(&left.name))
    });
    result.push(VersionCategory {
        name: "Snapshot".to_string(),
        versions: snapshots,
    });
    result.push(VersionCategory {
        name: "Beta".to_string(),
        versions: old_beta,
    });
    result.push(VersionCategory {
        name: "Alpha".to_string(),
        versions: old_alpha,
    });
    Ok(result)
}

#[command]
pub async fn get_forge_versions(
    _app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<VersionCategory>, AppError> {
    let cfg = state.config.read().await;
    let preferred = cfg.download_settings.mirror.clone();
    drop(cfg);
    let mirror = mirror::resolve_effective(&preferred).await;
    let manifest = load_version_manifest(&mirror).await?;
    let mut result: Vec<VersionCategory> = Vec::new();
    let versions: Vec<&VersionInfo> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::Release))
        .collect();
    for ver in versions {
        let id = ver.id.clone();
        let category = version_category(&id);
        let cat_opt = result.iter_mut().find(|x| x.name == category.clone());
        let cat = if cat_opt.is_none() {
            let c = VersionCategory {
                versions: vec![],
                name: category.clone(),
            };
            result.push(c);
            result.iter_mut().find(|x| x.name == category).unwrap()
        } else {
            cat_opt.unwrap()
        };
        let mut available = get_available_forge_versions(&id, &mirror).await?;
        available.sort_by(|left, right| {
            let left_key = parse_forge_version_key(left);
            let right_key = parse_forge_version_key(right);
            right_key.cmp(&left_key)
        });
        cat.versions.extend(
            available
                .iter()
                .map(|x| VersionLoader {
                    id: x.clone(),
                    base: FORGE,
                    date: "FORGE".to_string(),
                })
                .collect::<Vec<_>>(),
        );
    }
    for category in &mut result {
        category.versions.sort_by(|left, right| {
            let left_key = parse_forge_version_key(&left.id);
            let right_key = parse_forge_version_key(&right.id);
            right_key.cmp(&left_key)
        });
    }
    result.sort_by(|left, right| {
        minecraft_semver_key(&right.name).cmp(&minecraft_semver_key(&left.name))
    });
    Ok(result)
}

#[command]
pub async fn get_fabric_versions(
    _app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<VersionCategory>, AppError> {
    let cfg = state.config.read().await;
    let preferred = cfg.download_settings.mirror.clone();
    drop(cfg);
    let mirror = mirror::resolve_effective(&preferred).await;
    let manifest = load_version_manifest(&mirror).await?;
    let mut result: Vec<VersionCategory> = Vec::new();
    let versions: Vec<&VersionInfo> = manifest
        .versions
        .iter()
        .filter(|x| matches!(x.version_type, VersionType::Release))
        .collect();
    for ver in versions {
        let id = ver.id.clone();
        let category = version_category(&id);
        let cat_opt = result.iter_mut().find(|x| x.name == category.clone());
        let cat = if cat_opt.is_none() {
            let c = VersionCategory {
                versions: vec![],
                name: category.clone(),
            };
            result.push(c);
            result.iter_mut().find(|x| x.name == category).unwrap()
        } else {
            cat_opt.unwrap()
        };

        cat.versions.extend(
            get_available_fabric_versions(&id)
                .await?
                .iter()
                .map(|x| VersionLoader {
                    id: x.clone(),
                    base: FABRIC,
                    date: "FABRIC".to_string(),
                })
                .collect::<Vec<_>>(),
        );
    }
    for category in &mut result {
        category.versions.sort_by(|left, right| {
            let left_key = parse_fabric_version_key(left);
            let right_key = parse_fabric_version_key(right);
            right_key.cmp(&left_key)
        });
    }
    result.sort_by(|left, right| {
        minecraft_semver_key(&right.name).cmp(&minecraft_semver_key(&left.name))
    });
    Ok(result)
}

#[command]
pub async fn get_optifine_versions() -> Result<Vec<VersionCategory>, AppError> {
    let catalog: Vec<OptifineVersion> =
        http::get_json("https://bmclapi2.bangbang93.com/optifine/versionList").await?;
    let mut result: Vec<VersionCategory> = Vec::new();

    for release in catalog {
        if !release
            .mcversion
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '.')
            || !release.kind.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
            || !release.patch.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
        {
            warn!("ignoring invalid OptiFine catalog entry: {:?}", release);
            continue;
        }

        let mut version_parts = release.mcversion.split('.');
        let major = version_parts.next().unwrap_or(&release.mcversion);
        let minor = version_parts.next();
        let category = minor.map_or_else(
            || major.to_string(),
            |minor_version| format!("{major}.{minor_version}"),
        );
        let category_index = result.iter().position(|item| item.name == category);
        let target = if let Some(index) = category_index {
            &mut result[index]
        } else {
            result.push(VersionCategory {
                name: category,
                versions: Vec::new(),
            });
            result.last_mut().expect("category was just inserted")
        };
        let profile_id = release.profile_id();
        if target
            .versions
            .iter()
            .any(|version| version.id == profile_id)
        {
            continue;
        }
        target.versions.push(VersionLoader {
            id: profile_id,
            base: OPTIFINE,
            date: format!(
                "OPTIFINE|{}|{}|{}",
                release.kind, release.patch, release.filename
            ),
        });
    }

    for category in &mut result {
        category.versions.sort_by(|left, right| {
            optifine_version_key(right).cmp(&optifine_version_key(left))
        });
    }
    result.sort_by(|left, right| {
        minecraft_semver_key(&right.name).cmp(&minecraft_semver_key(&left.name))
    });

    Ok(result)
}

fn minecraft_semver_key(version: &str) -> Vec<u32> {
    let mut parts: Vec<u32> = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
    while parts.len() < 3 {
        parts.push(0);
    }
    parts
}

fn parse_forge_version_key(version: &str) -> (Vec<u32>, Vec<u32>) {
    let (mc_str, build_str) = version.split_once('-').unwrap_or((version, ""));
    let mc_key = minecraft_semver_key(mc_str);
    let build_key: Vec<u32> = build_str
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
    (mc_key, build_key)
}

fn parse_fabric_version_key(loader: &VersionLoader) -> (Vec<u32>, Vec<u32>) {
    let mc_key = minecraft_semver_key(&loader.get_fabric_version_id());
    let loader_key: Vec<u32> = loader
        .get_fabric_loader_id()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<u32>().unwrap_or(0))
        .collect();
    (mc_key, loader_key)
}

fn optifine_version_key(loader: &VersionLoader) -> (Vec<u32>, u8, Vec<u32>, String) {
    let mc = loader.get_optifine_version_id();
    let mc_key = minecraft_semver_key(&mc);

    let metadata = loader.date.strip_prefix("OPTIFINE|").unwrap_or("");
    let parts: Vec<&str> = metadata.split('|').collect();
    let kind = parts.first().copied().unwrap_or("");
    let patch = parts.get(1).copied().unwrap_or("");
    let filename = parts.get(2).copied().unwrap_or("");

    // 1 for official stable release, 0 for preview build
    let is_stable = if filename.starts_with("preview_") || patch.starts_with("pre") {
        0u8
    } else {
        1u8
    };

    let mut numbers = Vec::new();
    for token in [kind, patch] {
        for segment in token.split(|c: char| !c.is_ascii_digit()) {
            if let Ok(n) = segment.parse::<u32>() {
                numbers.push(n);
            }
        }
    }

    (mc_key, is_stable, numbers, patch.to_string())
}

fn version_number_key(version: &str) -> Vec<u32> {
    version
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse::<u32>()
                .unwrap_or(0)
        })
        .collect()
}

fn version_category(version: &str) -> String {
    let mut parts = version.split('.');
    let major = parts.next().unwrap_or(version);
    parts
        .next()
        .map_or_else(|| major.to_string(), |minor| format!("{major}.{minor}"))
}

#[command]
pub async fn download_version(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_loader: VersionLoader,
    name: String,
) -> Result<String, AppError> {
    // Everything the background task needs is cloned out of State here;
    // `State<'_, _>` cannot cross the spawn boundary.
    let config = state.config.clone();
    let logger = state.log_tx.clone();

    let mut version_id = version_loader.get_installed_id();
    let label = if name.trim().is_empty() {
        version_id.clone()
    } else {
        name.clone()
    };
    let session = download_session::create_session(&app_handle, &label, &version_id);
    let session_id = session.id().to_string();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_install(
            app_handle,
            config,
            logger,
            version_loader,
            name.clone(),
            &session,
            &mut version_id,
        )
        .await
        {
            if matches!(e, AppError::Cancelled) {
                info!("download session {} cancelled", session.id());
                session.mark_cancelled();
            } else {
                warn!("download session {} failed: {e:?}", session.id());
                session.fail(&e.to_string());
            }
            return;
        }
        session.complete();
        // Give the freshly installed version its own isolated instance
        // folder (game dir + mods) so per-version mod management works
        // immediately after download.
        if let Err(e) = crate::services::directory_manager::ensure_instance_dirs(&version_id).await
        {
            warn!("failed to create instance dirs for {version_id}: {e:?}");
        }
        if !name.trim().is_empty() {
            let mut settings = crate::services::instance_manager::get_instance_settings(&version_id);
            settings.display_name = name.trim().to_string();
            let _ = crate::services::instance_manager::save_instance_settings(&version_id, settings);
        }
        // Re-scan the versions directory so the new install shows up in
        // the version list immediately.
        let _ = reload_installed_versions().await;
        info!("download session {} completed", session.id());
    });

    Ok(session_id)
}

/// The actual install pipeline, executed on a background task so the
/// UI stays responsive and the download survives page navigation.
pub(crate) async fn run_install(
    app_handle: AppHandle,
    config: Arc<RwLock<Config>>,
    logger: UnboundedSender<LogLine>,
    version_loader: VersionLoader,
    name: String,
    session: &SessionHandle,
    version_id: &mut String,
) -> Result<(), AppError> {
    let cfg = config.read().await;
    let preferred = cfg.download_settings.mirror.clone();
    drop(cfg);

    // Probe the configured mirror and silently fall back to 9Craft when
    // Mojang CDNs are unreachable (common in filtered regions).
    let mir = mirror::resolve_effective(&preferred).await;

    info!(
        "Downloading version {} (base={:?}) from {} mirror",
        version_loader.id, version_loader.base, mir.name
    );

    // ── Step 1: Run loader installer (Forge / Fabric) ──────────────
    // The installer creates the version JSON on disk. For vanilla we
    // skip this step and download the JSON from the Mojang manifest
    // in Step 2 instead.
    match version_loader.base {
        FORGE => {
            let minecraft_version = version_loader.get_forge_version_id();
            if minecraft_version.is_empty() {
                return Err(AppError::VersionNotFound(format!(
                    "invalid Forge version id '{}'",
                    version_loader.id
                )));
            }

            // Forge's official client installer refuses a fresh custom
            // launcher directory unless the vanilla version and a launcher
            // profile already exist. Prepare the parent first, which also
            // provisions the Java runtime required by that Minecraft release.
            info!("Preparing vanilla parent for Forge: {minecraft_version}");
            let manifest = load_version_manifest(&mir).await?;
            game_downloader::download_from_manifest(&minecraft_version, &manifest, &mir).await?;
            let parent = MinecraftVersion::from_id(minecraft_version);
            let cfg = config.read().await;
            session.set_pass_range(0.0, 0.55);
            game_downloader::download_version(&parent, &name, &app_handle, &logger, &cfg, session)
                .await?;
            drop(cfg);

            info!(
                "Forge parent prepared; running installer {}",
                version_loader.id
            );
            session.set_pass_range(0.55, 0.75);
            download_forge_version(
                &version_loader.id,
                &app_handle,
                &logger,
                &mir,
                version_id,
                session,
            )
            .await?;
        }
        FABRIC => {
            info!(
                "Fabric version detected: {}, running installer",
                version_loader.id
            );
            download_fabric(&version_loader, &logger, &mir, session).await?;
        }
        OPTIFINE => {
            let minecraft_version = version_loader.get_optifine_version_id();
            if minecraft_version.is_empty() {
                return Err(AppError::VersionNotFound(format!(
                    "invalid OptiFine version id '{}'",
                    version_loader.id
                )));
            }

            info!("Preparing vanilla parent for OptiFine: {minecraft_version}");
            let manifest = load_version_manifest(&mir).await?;
            game_downloader::download_from_manifest(&minecraft_version, &manifest, &mir).await?;
            let parent = MinecraftVersion::from_id(minecraft_version);
            let cfg = config.read().await;
            session.set_pass_range(0.0, 0.55);
            game_downloader::download_version(&parent, &name, &app_handle, &logger, &cfg, session)
                .await?;
            drop(cfg);
            session.set_pass_range(0.55, 0.75);
            game_downloader::download_optifine(&version_loader, session).await?;
        }
        _ => {
            // Vanilla — the version JSON will be downloaded from the
            // Mojang manifest in Step 2 below.
        }
    }

    // ── Step 2: Download the version JSON ──────────────────────────
    // For vanilla versions, download the JSON from the Mojang manifest.
    // For Fabric/Forge, the JSON was already created by the installer
    // in Step 1 — skip this to avoid searching the Mojang manifest for
    // a loader profile ID that doesn't exist there.
    let is_loader_profile = matches!(version_loader.base, FORGE | FABRIC | NEOFORGE | OPTIFINE);
    if !is_loader_profile {
        info!("Downloading {}.json from Mojang manifest", version_id);
        let manifest = load_version_manifest(&mir).await?;
        game_downloader::download_from_manifest(version_id, &manifest, &mir).await?;
    } else {
        info!(
            "Version {}.json already created by loader installer, skipping manifest lookup",
            version_id
        );
    }

    let version = MinecraftVersion::from_id(version_id.clone());

    // ── Step 3: Download the inherited (parent) version if any ─────
    // Fabric/Forge profiles have `inheritsFrom` pointing to the base
    // Minecraft version. We need to download that too. It occupies the
    // first half of the progress bar; the main pass gets the rest.
    // A vanilla install is a single pass over the whole bar.
    let inherited_version = version.get_inherited();
    let cfg = config.read().await;
    if inherited_version.id != version.id && !matches!(version_loader.base, FORGE | OPTIFINE) {
        info!("Downloading inherited version: {}", inherited_version.id);
        session.set_pass_range(0.0, 0.5);
        game_downloader::download_version(
            &inherited_version,
            &name,
            &app_handle,
            &logger,
            &cfg,
            session,
        )
        .await?;
        session.set_pass_range(0.5, 1.0);
    } else {
        let pass_start = if matches!(version_loader.base, FORGE | OPTIFINE) {
            0.75
        } else {
            0.0
        };
        session.set_pass_range(pass_start, 1.0);
    }

    // ── Step 4: Download the main version (libraries, assets, Java) ─
    let result =
        game_downloader::download_version(&version, &name, &app_handle, &logger, &cfg, session)
            .await;
    drop(cfg);
    result
}

/// All download sessions (running, paused and recently finished),
/// newest first. Lets the UI re-sync after navigation or reload.
#[command]
pub async fn get_active_downloads() -> Result<Vec<SessionInfo>, AppError> {
    Ok(download_session::list_sessions())
}

#[command]
pub async fn pause_download(session_id: String) -> Result<(), AppError> {
    download_session::with_session(&session_id, |s| s.pause())
        .ok_or_else(|| AppError::UnknownError(format!("unknown download session {session_id}")))
}

#[command]
pub async fn resume_download(session_id: String) -> Result<(), AppError> {
    download_session::with_session(&session_id, |s| s.resume())
        .ok_or_else(|| AppError::UnknownError(format!("unknown download session {session_id}")))
}

#[command]
pub async fn cancel_download(session_id: String) -> Result<(), AppError> {
    download_session::with_session(&session_id, |s| s.cancel())
        .ok_or_else(|| AppError::UnknownError(format!("unknown download session {session_id}")))
}

/// Drop terminal (completed/failed/cancelled) sessions from the registry.
#[command]
pub async fn clear_finished_downloads() -> Result<(), AppError> {
    download_session::clear_finished();
    Ok(())
}

/// Gives the available versions to download
#[command]
pub async fn get_versions() -> Result<Vec<String>, AppError> {
    let global = GLOBAL_CACHE.lock().await;
    Ok(global
        .versions
        .iter()
        .map(|x| x.id.to_string())
        .clone()
        .collect())
}

#[command]
pub async fn get_non_installed_versions() -> Result<Vec<String>, AppError> {
    let global = GLOBAL_CACHE.lock().await;
    let versions = global.versions.clone();
    Ok(versions
        .iter()
        .filter(|x| !x.is_installed())
        .map(|x| x.id.clone())
        .collect())
}

#[command]
pub async fn get_installed_versions() -> Result<Vec<String>, AppError> {
    let global = GLOBAL_CACHE.lock().await;
    let versions = global.versions.clone();
    Ok(versions
        .iter()
        .filter(|x| x.is_installed())
        .map(|x| x.id.clone())
        .collect())
}

/// Verify and optionally repair an installed version.
///
/// When `apply_repair` is `false`, this is a read-only operation that
/// returns a report of missing/corrupt files. When `true`, the launcher
/// re-downloads only the broken artifacts via the central
/// DownloadManager (with hash verification).
#[command]
pub async fn repair_version(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
    apply_repair: bool,
) -> Result<RepairReport, AppError> {
    do_repair(&version_id, apply_repair, &app_handle, &state).await
}

#[cfg(test)]
mod tests {
    use super::version_category;

    #[test]
    fn version_category_handles_release_and_unusual_ids() {
        assert_eq!(version_category("1.21.11"), "1.21");
        assert_eq!(version_category("26.1"), "26.1");
        assert_eq!(version_category("combat-test"), "combat-test");
    }
}
