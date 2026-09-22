pub mod commands;
pub mod models;
pub mod services;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, LazyLock, Mutex};

use log::info;
use models::config::Config;
use models::error::AppError;
use models::fabric::{FabricInstaller, FabricLoader, FabricMinecraftVersion};
use models::launch::LaunchResult;
use models::logger::{init_log_bridge, LogLine};
use models::mirror::mojang_mirror;
use models::mods::ModInfo;
use models::versions::MinecraftVersion;
use services::config::load;
use services::directory_manager::{create_necessary_dirs, get_falcon_launcher_directory};
use services::download_manager::DownloadManager;
use services::game_launcher::launch_game;
use services::version_manager::{download_version_manifest, reload_installed_versions};
use tauri::async_runtime::{block_on, spawn};
use tauri::{command, AppHandle, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy};
use tokio::sync::{self, mpsc, RwLock};

pub struct FalconLauncher {
    pub name: String,
    pub version: String,
}

pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub launcher_details: FalconLauncher,
    pub log_tx: mpsc::UnboundedSender<LogLine>,
    pub log_history: Arc<Mutex<VecDeque<LogLine>>>,
    pub downloader: Arc<DownloadManager>,
}

pub struct Global {
    pub forge: Option<HashMap<String, Vec<String>>>,
    pub fabric_loaders: Option<Vec<FabricLoader>>,
    pub fabric_installers: Option<Vec<FabricInstaller>>,
    pub fabric_mc_versions: Option<Vec<FabricMinecraftVersion>>,
    pub versions: Vec<MinecraftVersion>,
}

pub static GLOBAL_CACHE: LazyLock<sync::Mutex<Global>> = LazyLock::new(|| {
    sync::Mutex::new(Global {
        forge: None,
        fabric_loaders: None,
        fabric_installers: None,
        fabric_mc_versions: None,
        versions: Vec::new(),
    })
});

pub const LAUNCHER_NAME: &str = "GlitchyLauncher";
pub const LAUNCHER_VERSION: &str = "1.3.1";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = dotenvy::dotenv();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Folder {
                        path: get_falcon_launcher_directory(),
                        file_name: Some("latest".to_string()),
                    }),
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                ])
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .setup(move |app| {
            info!("Glitchy Launcher initialization starting...");

            // Log the canonical directory hierarchy at startup so path
            // issues are immediately visible in the log file.
            info!("=== Canonical Directory Hierarchy ===");
            info!("  minecraft_root: {}", crate::services::directory_manager::get_minecraft_directory().display());
            info!("  versions_dir:   {}", crate::services::directory_manager::get_versions_directory().display());
            info!("  libraries_dir:  {}", crate::services::directory_manager::get_libraries_directory().display());
            info!("  assets_dir:     {}", crate::services::directory_manager::get_assets_directory().display());
            info!("  instances_dir:  {}", crate::services::directory_manager::get_instances_directory().display());
            info!("  launcher_dir:   {}", crate::services::directory_manager::get_falcon_launcher_directory().display());
            info!("  java_dir:       {}", crate::services::directory_manager::get_launcher_java_directory().display());
            info!("  config_path:    {}", crate::services::directory_manager::get_config_directory().display());
            info!("  manifest_cache: {}", crate::services::directory_manager::version_manifest_directory().display());
            info!("=====================================");

            spawn(async {
                create_necessary_dirs().await;
                let resolved = crate::models::mirror::resolve_effective(&mojang_mirror()).await;
                if let Err(e) = download_version_manifest(&resolved).await {
                    info!("manifest download failed (will use cache if present): {e:?}");
                }
            });

            let app_handle = app.handle().clone();
            let shared_history = Arc::new(Mutex::new(VecDeque::with_capacity(10000)));
            let bridge_history = shared_history.clone();
            let log_tx = init_log_bridge(app_handle, bridge_history);

            let downloader = Arc::new(DownloadManager::new(8));
            app.manage(AppState {
                config: Arc::new(RwLock::new(load())),
                launcher_details: FalconLauncher {
                    name: LAUNCHER_NAME.to_string(),
                    version: LAUNCHER_VERSION.to_string(),
                },
                log_tx,
                log_history: shared_history,
                downloader,
            });

            block_on(async {
                reload_installed_versions().await;
            });
            info!("Installed versions reloaded.");

            let window = app.handle().get_window("main").unwrap();
            fit_window_to_work_area(&window);
            window.center().expect("Failed to center the window");
            window.set_focus().expect("Failed to set window focus");

            #[cfg(any(windows, target_os = "linux"))]
            {
                app.deep_link().register("glitchyLauncher")?;
                app.deep_link().register_all()?;
            }
            app.deep_link().on_open_url(|event| {
                info!("deep link URLs: {:?}", event.urls());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            play,
            commands::downloader::get_versions,
            commands::settings::get_maximum_ram_usage,
            commands::settings::get_minimum_ram_usage,
            commands::settings::set_maximum_ram_usage,
            commands::settings::set_minimum_ram_usage,
            commands::settings::set_language,
            commands::settings::get_language,
            commands::settings::set_exit_on_launch,
            commands::settings::should_exit_on_launch,
            commands::settings::save,
            commands::settings::set_config,
            commands::settings::get_selected_profile,
            commands::settings::set_selected_profile,
            commands::settings::get_total_ram,
            commands::mods::toggle_backpack_item,
            commands::mods::delete_backpack_item,
            commands::mods::get_backpack_items,
            commands::mods::get_instance_info,
            commands::mods::import_backpack_item,
            commands::mods::open_backpack_folder,
            commands::modrinth::modrinth_search,
            commands::modrinth::modrinth_get_project_versions,
            commands::modrinth::modrinth_download_item,
            commands::modpacks::search_modpacks,
            commands::modpacks::get_modpack_versions,
            commands::modpacks::install_modpack,
            commands::modpacks::import_modpack,
            commands::instances::list_instances,
            commands::instances::update_instance_settings,
            commands::instances::set_instance_path,
            commands::instances::reset_instance_path,
            commands::instances::clone_instance,
            commands::instances::delete_instance,
            commands::instances::reinstall_instance,
            commands::instances::list_instance_worlds,
            commands::instances::delete_instance_world,
            commands::instances::create_instance_backup,
            commands::instances::list_instance_backups,
            commands::instances::restore_instance_backup,
            commands::instances::delete_instance_backup,
            commands::instances::open_instance_folder,
            commands::downloader::download_version,
            commands::downloader::get_installed_versions,
            commands::downloader::get_non_installed_versions,
            commands::downloader::get_forge_versions,
            commands::downloader::get_fabric_versions,
            commands::downloader::get_optifine_versions,
            commands::downloader::get_vanilla_versions,
            commands::downloader::repair_version,
            commands::downloader::get_active_downloads,
            commands::downloader::pause_download,
            commands::downloader::resume_download,
            commands::downloader::cancel_download,
            commands::downloader::clear_finished_downloads,
            commands::profiles::get_profiles,
            commands::profiles::create_offline_profile,
            commands::profiles::rename_profile,
            commands::profiles::remove_profile,
            commands::logger::get_log_history,
            commands::logger::clear_log_history_channel,
            commands::logger::clear_log_history,
            commands::logger::debug,
            commands::mirrors::get_available_mirrors,
            commands::mirrors::set_mirror,
            commands::mirrors::get_mirror,
            commands::mirrors::import_mirror,
            commands::java::list_detected_javas,
            commands::java::get_selected_java,
            commands::java::set_selected_java,
            commands::glitchy::glitchy_get_achievements,
            commands::glitchy::glitchy_get_badges,
            commands::glitchy::glitchy_set_displayed_badges,
            commands::glitchy::glitchy_get_profile,
            commands::glitchy::glitchy_save_customization,
            commands::glitchy::glitchy_upload_avatar,
            commands::glitchy::glitchy_avatar_path,
            commands::glitchy::glitchy_avatar_data,
            commands::glitchy::glitchy_get_allowed_accents,
            commands::glitchy::glitchy_get_journey,
            commands::glitchy::glitchy_get_statistics,
            commands::glitchy::glitchy_get_recent_activity,
            commands::glitchy::glitchy_is_maximized,
            commands::glitchy::glitchy_toggle_maximized,
            commands::glitchy::glitchy_set_active_skin,
            commands::glitchy::glitchy_get_active_skin,
            commands::ai::ai_chat,
            commands::ai::ai_diagnose_log,
            commands::ai::ai_apply_autofix,
            commands::ai::get_system_diagnostics,
            commands::ai::get_ai_usage_status,
            commands::updater::check_launcher_update,
            commands::updater::apply_launcher_update,
            commands::account::glitchy_account_register,
            commands::account::glitchy_account_login,
            commands::account::glitchy_account_logout,
            commands::account::glitchy_account_get_current,
            commands::account::glitchy_account_upload_skin,
            commands::account::glitchy_account_sync_save,
            commands::account::glitchy_account_sync_load,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Clamp the main window so it never exceeds the primary monitor's work
/// area. On 1080p displays running 125–150% DPI scaling, the logical
/// 1280×800 default is physically larger than the screen — cap it to
/// 92% of the work area (in physical pixels) and lower the minimum size
/// to match.
fn fit_window_to_work_area(window: &tauri::Window) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let max_w = (area.size.width as f64 * 0.92).floor();
    let max_h = (area.size.height as f64 * 0.92).floor();
    let want_w = (1280.0 * scale).min(max_w);
    let want_h = (800.0 * scale).min(max_h);
    let _ = window.set_min_size(Some(tauri::PhysicalSize::new(
        (500.0 * scale).floor().min(max_w) as u32,
        (560.0 * scale).floor().min(max_h) as u32,
    )));
    let _ = window.set_size(tauri::PhysicalSize::new(
        want_w.round() as u32,
        want_h.round() as u32,
    ));
}

/// Launch the game. Returns a structured [`LaunchResult`] on success
/// (PID, version, username, start time) so the frontend can render a
/// "game is running" indicator. On failure returns a typed [`AppError`]
/// the UI can map to a user message + recovery action.
#[command]
async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    selected_version: String,
    direct_connect: Option<crate::services::game_launcher::DirectConnectTarget>,
) -> Result<LaunchResult, AppError> {
    let _ = &state; // AppState is also held by the launcher internally.
    let versions = {
        let global = GLOBAL_CACHE.lock().await;
        global.versions.clone()
    };
    let result =
        launch_game(app.clone(), selected_version.clone(), &versions, direct_connect).await?;
    commands::glitchy::on_play_session(&app, &selected_version);
    Ok(result)
}
