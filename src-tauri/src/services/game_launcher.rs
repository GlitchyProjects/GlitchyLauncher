use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::sync::Mutex as StdMutex;
use std::time::Instant;

use log::{error, info};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::error::AppError;
use crate::models::versions::MinecraftVersion;
use crate::models::launch::{GameExitEvent, GameExitKind, LaunchProgress, LaunchResult};
use crate::models::logger::{error as log_error, info as log_info};
use crate::models::platform::get_current_os;
use crate::models::profiles::get_profile;
use crate::services::directory_manager::*;
use crate::services::download_session;
use crate::services::game_downloader::download_version;
use crate::services::jdk_manager::{download_java, get_java};
use crate::services::utils::{
    apply_dedicated_gpu_env, extend_once, is_wayland, linux_java_permission_fix, vec_to_string,
};
use crate::AppState;
use crate::Global;

pub static RUNNING_INSTANCES: LazyLock<StdMutex<HashSet<String>>> =
    LazyLock::new(|| StdMutex::new(HashSet::new()));

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectConnectTarget {
    pub server: String,
    pub port: Option<u16>,
}

pub fn is_instance_running(instance_id: &str) -> bool {
    RUNNING_INSTANCES
        .lock()
        .map(|set| set.contains(instance_id))
        .unwrap_or(false)
}

struct RunningInstanceGuard(String);
impl Drop for RunningInstanceGuard {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            if let Ok(mut running) = RUNNING_INSTANCES.lock() {
                running.remove(&self.0);
            }
        }
    }
}

/// Launch Minecraft.
///
/// Returns a structured [`LaunchResult`] on success — never returns
/// success-as-error (the old code returned `Err(FileReadFailed("Tesat"))`
/// which was a placeholder).
///
/// On failure returns a typed [`AppError`] the frontend can map to a
/// user-facing message and recovery action.
///
/// The launched child process is monitored on a background thread; when
/// it exits, a `game-exit` event is emitted with a classification
/// (`normal_exit`, `crash`, `terminated`, `launch_failure`).
pub async fn launch_game(
    app_handle: AppHandle,
    version: String,
    versions_cache: &[MinecraftVersion],
    direct_connect: Option<DirectConnectTarget>,
) -> Result<LaunchResult, AppError> {
    info!("Launching minecraft {version}");
    emit_launch_progress(&app_handle, "Preparing to launch", 5);

    let state = &app_handle.state::<AppState>();
    let tx_err = state.log_tx.clone();
    let tx_out = state.log_tx.clone();

    let config = state.config.read().await;
    let launch_options = &config.launch_options;
    let uid = &launch_options.selected_profile;
    let profile = get_profile(uid).ok_or_else(|| {
        AppError::NoProfileSelected
    })?;
    let username = profile.username;

    let ver_res = versions_cache.iter().find(|x| x.id == version).cloned();
    let version = ver_res.unwrap_or_else(|| MinecraftVersion::from_id(version.clone()));
    let version_id = version.id.clone();

    {
        let mut running = RUNNING_INSTANCES
            .lock()
            .map_err(|_| AppError::UnknownError("running instances lock poisoned".to_string()))?;
        if running.contains(&version_id) {
            return Err(AppError::UnknownError(format!(
                "Instance '{version_id}' is already running. Please close the game first."
            )));
        }
        running.insert(version_id.clone());
    }
    let mut running_guard = RunningInstanceGuard(version_id.clone());
    let instance_settings =
        crate::services::instance_manager::get_instance_settings(&version_id);
    let xms = format!(
        "{}M",
        instance_settings
            .ram_min_mb
            .unwrap_or(launch_options.ram_usage_min)
    );
    let xmx = format!(
        "{}M",
        instance_settings
            .ram_max_mb
            .unwrap_or(launch_options.ram_usage_max)
    );
    let version_id_err_clone = version_id.clone();
    let version_id_out_clone = version_id.clone();

    let json: Value = version.load_json();
    if json.is_null() {
        return Err(AppError::LaunchFailed(format!(
            "failed to load version json for {}",
            version.id
        )));
    }

    let inherited_version = version.get_inherited();
    let inherited_json = inherited_json(&inherited_version)?;
    let inherited_id = &inherited_version.id;
    emit_launch_progress(&app_handle, "Loading version manifest", 25);

    info!("Resolving Java component for {}", inherited_id);
    let java_component = inherited_json["javaVersion"]["component"]
        .as_str()
        .unwrap_or("jre-legacy")
        .to_string();

    let java = match get_java(java_component.clone()) {
        Ok(j) if j.get_bin_file().exists() => j,
        _ => {
            // Fresh machines have no runtime installed yet — provision it
            // on demand instead of failing, and report progress so the
            // launch progress bar reflects the (long) download.
            info!("Java runtime '{java_component}' missing — downloading it now");
            emit_launch_progress(&app_handle, "Downloading Java runtime", 35);
            let required_major = inherited_json["javaVersion"]["majorVersion"].as_u64().unwrap_or(8);
            let mirror =
                crate::models::mirror::resolve_effective(&config.download_settings.mirror).await;
            download_java(
                &java_component,
                &required_major.to_string(),
                &tx_out,
                &mirror,
                None,
            )
            .await?;
            get_java(java_component.clone())?
        }
    };
    // Validate the binary actually exists before we spawn. This is a
    // cheap check that converts a confusing JVM error into a clear
    // `JavaExecutableInvalid` recovery hint.
    if !java.get_bin_file().exists() {
        return Err(AppError::JavaExecutableInvalid(format!(
            "java binary not found at {}",
            java.get_bin_file().display()
        )));
    }
    // Validate Java version compatibility from the manifest.
    if let Some(required_major) = inherited_json["javaVersion"]["majorVersion"].as_u64() {
        validate_java_version(&java, required_major as u32)?;
    }
    linux_java_permission_fix(&java)?;
    emit_launch_progress(&app_handle, "Java runtime ready", 45);

    let version_directory = PathBuf::from(&inherited_version.version_path);
    // Each installed version runs with its OWN game directory
    // (`instances/<id>`) so mods, saves and config stay isolated per
    // version instead of being mixed in the shared `.minecraft` root.
    let instance_directory = get_instance_directory(&version_id)?;
    std::fs::create_dir_all(&instance_directory).map_err(|e| {
        AppError::DirCreateFailed(format!("{}: {e}", instance_directory.display()))
    })?;
    // NOTE: ensure_instance_options is called AFTER skin injection below
    // so it can act as the final sanitizer for narrator:0 and other settings.
    let game_directory = instance_directory.display().to_string();
    let asset_directory = get_assets_directory().display().to_string();
    // Old versions (<1.6) still need this legacy path for resources.
    let resources_directory = get_minecraft_directory()
        .join("resources")
        .display()
        .to_string();

    let libraries = version.get_libraries();

    let asset_index = inherited_json["assets"]
        .as_str()
        .unwrap_or("legacy")
        .to_string();
    let main_class = json["mainClass"]
        .as_str()
        .ok_or_else(|| AppError::LaunchArgsNotFound)?;
    let class_path = version_directory
        .join(format!("{inherited_id}.jar"))
        .to_str()
        .ok_or_else(|| AppError::PathValidationFailed("version_directory not UTF-8".to_string()))?
        .to_string();
    let natives = get_natives_folder(&inherited_version.id)
        .to_str()
        .ok_or_else(|| AppError::PathValidationFailed("natives folder not UTF-8".to_string()))?
        .to_string();
    let typ = json["type"].as_str().unwrap_or("release");
    let run_args_iter = get_launch_args(&json)?;
    let mut jvm_args = get_jvm_args(&json);
    let jvm_args_inherited = get_jvm_args(&inherited_json);
    jvm_args = extend_once(jvm_args_inherited, jvm_args);
    let run_args_iter_inherited = get_launch_args(&inherited_json)?;
    let run_args_iter_sum = extend_once(run_args_iter, run_args_iter_inherited);

    let mut run_args = run_args_iter_sum
        .iter()
        .map(|v| {
            v.replace("${auth_player_name}", username.as_str())
                .replace("${version_name}", &version.id)
                .replace("${game_directory}", &game_directory)
                .replace("${assets_root}", &asset_directory)
                .replace("${game_assets}", &resources_directory)
                .replace("${assets_index_name}", &asset_index)
                .replace("${auth_uuid}", &uid.to_string())
                // Offline mode: token is a well-known placeholder, never a secret.
                .replace("${auth_access_token}", "offline-token")
                .replace("${user_properties}", "{}")
                .replace("${user_type}", "legacy")
                .replace("${version_type}", typ)
                .replace("${clientid}", &uuid::Uuid::new_v4().to_string())
                .replace("${auth_xuid}", "0")
        })
        .collect::<Vec<String>>();

    if json.pointer("/logging/client/argument").is_some()
        && json.pointer("/logging/client/file/id").is_some()
    {
        let logger_path =
            version_directory.join(json["logging"]["client"]["file"]["id"].as_str().unwrap_or(""));
        if let Some(arg) = json["logging"]["client"]["argument"].as_str() {
            if let Some(p) = logger_path.to_str() {
                run_args.push(arg.replace("{path}", p));
            }
        }
    }

    if let Some(ref dc) = direct_connect {
        run_args.push("--server".to_string());
        run_args.push(dc.server.clone());
        if let Some(port) = dc.port {
            run_args.push("--port".to_string());
            run_args.push(port.to_string());
        }
    }

    let separator = if get_current_os() == "windows" { ";" } else { ":" };
    let libraries_str = vec_to_string(libraries, separator.to_string());
    let libraries_str = libraries_str.replace('\\', std::path::MAIN_SEPARATOR_STR);

    let mut child_cmd = Command::new(java.get_bin_file());
    child_cmd
        .current_dir(&game_directory)
        .arg(format!("-Xms{xms}"))
        .arg(format!("-Xmx{xmx}"))
        .arg("-Djava.net.preferIPv4Stack=true")
        .arg("-Dminecraft.launcher.brand=GlitchyLauncher")
        .arg("-Dminecraft.launcher.version=1.2.1");

    if is_wayland() {
        // Some Java versions crash on Wayland; force X11 via GDK_BACKEND.
        child_cmd
            .env("XDG_SESSION_TYPE", "x11")
            .env("GDK_BACKEND", "x11")
            .env("__GL_THREADED_OPTIMIZATIONS", "0");
    }
    apply_dedicated_gpu_env(&mut child_cmd);

    let full_classpath = if libraries_str.is_empty() {
        class_path
    } else if class_path.is_empty() {
        libraries_str.clone()
    } else {
        format!("{libraries_str}{separator}{class_path}")
    };
    let java_major = java_version_major(&java).unwrap_or(8);
    let use_argsfile = get_current_os() == "windows" && java_major >= 9 && full_classpath.len() > 8000;

    let argsfile_path = if use_argsfile {
        let p = instance_directory.join(".glitchy-classpath.args");
        let escaped_cp = full_classpath.replace('\\', "\\\\");
        let content = format!("-cp\n\"{escaped_cp}\"\n");
        let _ = std::fs::write(&p, content);
        Some(p)
    } else {
        None
    };

    if !jvm_args.is_empty() {
        let mut skip_next = false;
        for (i, arg) in jvm_args.iter().enumerate() {
            if skip_next {
                skip_next = false;
                continue;
            }
            if let Some(ref af) = argsfile_path {
                if arg == "-cp" || arg == "-classpath" || arg == "--class-path" {
                    if i + 1 < jvm_args.len() && jvm_args[i + 1].contains("${classpath}") {
                        child_cmd.arg(format!("@{}", af.display()));
                        skip_next = true;
                        continue;
                    }
                }
            }
            child_cmd.arg(
                arg.replace(
                    "${natives_directory}",
                    get_natives_folder(&inherited_version.id)
                        .to_str()
                        .unwrap_or(""),
                )
                .replace("${launcher_name}", &state.launcher_details.name)
                .replace("${launcher_version}", &state.launcher_details.version)
                .replace("${classpath}", full_classpath.as_str()),
            );
        }
    } else {
        child_cmd.arg(format!("-Djava.library.path={natives}"));
        if let Some(ref af) = argsfile_path {
            child_cmd.arg(format!("@{}", af.display()));
        } else {
            child_cmd
                .arg("-cp")
                .arg(&full_classpath);
        }
    }
    child_cmd
        .arg(main_class)
        .args(&run_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let run_args_str = run_args.join(" ");
    let jvm_args_str = jvm_args.join(" ");

    tx_out.send(log_info(
        format!("Loaded libraries: {libraries_str}"),
        version_id_out_clone.clone(),
    ));
    tx_out.send(log_info(
        format!("Game arguments: {run_args_str}"),
        version_id_out_clone.clone(),
    ));
    tx_out.send(log_info(
        format!("JVM arguments: {jvm_args_str}"),
        version_id_out_clone.clone(),
    ));

    // Offline skin injection: if the user configured a custom skin in Glitchy Launcher,
    // generate an active resource pack in the instance so Minecraft renders it in-game.
    // The resource pack overrides the default Steve/Alex textures so offline/cracked
    // players see their chosen skin instead of the vanilla default.
    let skins_dir = get_falcon_launcher_directory().join("skins");
    let active_skin_path = skins_dir.join("active_skin.png");
    if active_skin_path.exists() {
        if let Ok(skin_bytes) = std::fs::read(&active_skin_path) {
            let game_dir = Path::new(&game_directory);
            let rp_dir = game_dir.join("resourcepacks").join("GlitchySkin");

            // Create all necessary texture directories
            let entity_dir = rp_dir
                .join("assets")
                .join("minecraft")
                .join("textures")
                .join("entity");
            let wide_dir = entity_dir.join("player").join("wide");
            let slim_dir = entity_dir.join("player").join("slim");
            let _ = std::fs::create_dir_all(&wide_dir);
            let _ = std::fs::create_dir_all(&slim_dir);

            // Write pack.mcmeta with version-appropriate pack_format so Minecraft
            // accepts the pack without marking it as incompatible.
            let pack_format = version_to_pack_format(&version_id);
            let mcmeta = serde_json::json!({
                "pack": {
                    "pack_format": pack_format,
                    "supported_formats": [1, 99],
                    "description": "Glitchy Launcher Custom Skin"
                }
            });
            let _ = std::fs::write(rp_dir.join("pack.mcmeta"), mcmeta.to_string());

            // Write skin to ALL known texture paths so it works across all MC versions:
            // Pre-1.20.2 paths:
            let _ = std::fs::write(entity_dir.join("steve.png"), &skin_bytes);
            let _ = std::fs::write(entity_dir.join("alex.png"), &skin_bytes);
            // 1.20.2+ player texture paths (generic fallback):
            let _ = std::fs::write(wide_dir.join("steve.png"), &skin_bytes);
            let _ = std::fs::write(slim_dir.join("alex.png"), &skin_bytes);
            // 1.20.2+ per-username texture resolution — Minecraft looks for
            // <username_lowercase>.png in the player texture directory:
            let username_lower = username.to_lowercase();
            let _ = std::fs::write(wide_dir.join(format!("{username_lower}.png")), &skin_bytes);
            let _ = std::fs::write(slim_dir.join(format!("{username_lower}.png")), &skin_bytes);
            // Also write default.png as catch-all for unknown texture names:
            let _ = std::fs::write(wide_dir.join("default.png"), &skin_bytes);
            let _ = std::fs::write(slim_dir.join("default.png"), &skin_bytes);

            // Ensure GlitchySkin is listed in options.txt resourcePacks
            let options_path = game_dir.join("options.txt");
            if options_path.exists() {
                if let Ok(options_content) = std::fs::read_to_string(&options_path) {
                    let mut new_lines = Vec::new();
                    let mut found_rp = false;
                    for line in options_content.lines() {
                        if line.starts_with("resourcePacks:") {
                            found_rp = true;
                            if !line.contains("GlitchySkin") {
                                if let Some(bracket_pos) = line.find('[') {
                                    let (prefix, rest) = line.split_at(bracket_pos + 1);
                                    let updated = format!("{prefix}\"file/GlitchySkin\",{rest}");
                                    new_lines.push(updated);
                                    continue;
                                }
                            }
                        }
                        new_lines.push(line.to_string());
                    }
                    if !found_rp {
                        new_lines.push("resourcePacks:[\"file/GlitchySkin\",\"vanilla\"]".to_string());
                    }
                    // Always write trailing newline to prevent Minecraft from
                    // misinterpreting the last line as incomplete.
                    let _ = std::fs::write(&options_path, new_lines.join("\n") + "\n");
                }
            } else {
                let _ = std::fs::write(&options_path, "resourcePacks:[\"file/GlitchySkin\",\"vanilla\"]\n");
            }
        }
    }

    // Run ensure_instance_options AFTER skin injection so it acts as the final
    // sanitizer — guarantees narrator:0, joinedFirstServer:true, tutorialStep:none
    // even if the skin injection code created or modified options.txt above.
    ensure_instance_options(&instance_directory);

    emit_launch_progress(&app_handle, "Starting game", 75);

    let mut child = child_cmd
        .spawn()
        .map_err(|e| AppError::LaunchFailed(format!("failed to spawn java process: {e}")))?;

    // Spawn succeeded; disarm the RAII guard so the instance remains registered in RUNNING_INSTANCES
    running_guard.0.clear();

    let pid = child.id();
    let started_at = chrono::Utc::now().timestamp();
    emit_launch_progress(&app_handle, "Game starting", 90);

    let stdout = child.stdout.take().expect("stdout was piped above");
    let stderr = child.stderr.take().expect("stderr was piped above");

    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            let _ = tx_err.send(log_error(line, version_id_err_clone.clone()));
        }
    });

    let app_handle_for_exit = app_handle.clone();
    let version_id_for_exit = version_id.clone();
    let session_start = Instant::now();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                if line.contains("Connecting to ") {
                    let _ = app_handle_for_exit.emit("server-connected", &line);
                }
                let _ = tx_out.send(log_info(line, version_id_out_clone.clone()));
            }
        }
        let status = child.wait();
        // Remove from RUNNING_INSTANCES upon process exit
        if let Ok(mut running) = RUNNING_INSTANCES.lock() {
            running.remove(&version_id_for_exit);
        }
        let duration_ms = session_start.elapsed().as_millis() as u64;
        let duration_seconds = duration_ms / 1000;
        crate::commands::glitchy::on_play_session_end(&app_handle_for_exit, &version_id_for_exit, duration_seconds);
        let kind = match status {
            Ok(s) => {
                if s.success() {
                    GameExitKind::NormalExit {
                        code: s.code().unwrap_or(0),
                    }
                } else {
                    let code = s.code().unwrap_or(-1);
                    // Heuristic: negative codes usually indicate signal-based
                    // termination (e.g. user killed the process).
                    if code < 0 {
                        GameExitKind::Terminated
                    } else {
                        GameExitKind::Crash { code }
                    }
                }
            }
            Err(e) => GameExitKind::LaunchFailure {
                reason: e.to_string(),
            },
        };
        let _ = app_handle_for_exit.emit(
            "game-exit",
            GameExitEvent {
                version_id: version_id_for_exit,
                kind,
                duration_ms,
            },
        );
    });

    Ok(LaunchResult {
        pid,
        version_id: version_id,
        username,
        started_at,
    })
}

/// Report launch preparation progress to the frontend (`launch-progress`
/// event) so the play page can render a progress bar.
fn emit_launch_progress(app: &AppHandle, phase: &str, percent: u8) {
    let _ = app.emit(
        "launch-progress",
        LaunchProgress {
            phase: phase.to_string(),
            percent,
        },
    );
}

fn inherited_json(version: &crate::models::versions::MinecraftVersion) -> Result<Value, AppError> {
    let v = version.load_json();
    if v.is_null() || v.as_str().is_some() {
        // load_json returns `Value::from("")` when the file is missing.
        return Err(AppError::FileNotFound(version.get_json()));
    }
    Ok(v)
}

/// Validate that the Java runtime is at least the major version required
/// by the manifest. Returns `JavaIncompatible` if too old, with a clear
/// `required`/`found` pair the UI can show.
fn validate_java_version(java: &crate::models::java::Java, required_major: u32) -> Result<(), AppError> {
    let found = java_version_major(java).unwrap_or(0);
    if found < required_major {
        return Err(AppError::JavaIncompatible {
            required: required_major.to_string(),
            found: found.to_string(),
        });
    }
    Ok(())
}

/// Detect the major Java version of an installed runtime. We can't trust
/// the `release` file for auto-downloaded Mojang runtimes (it sometimes
/// lacks `JAVA_VERSION`), so we run `java -version` as a fallback.
fn java_version_major(java: &crate::models::java::Java) -> Option<u32> {
    // Try the `release` file first — it's free.
    if let Some(v) = java.version_major_from_release() {
        return Some(v);
    }
    let output = Command::new(java.get_bin_file()).arg("-version").output().ok()?;
    let text = String::from_utf8_lossy(&output.stderr);
    // `java -version` prints to stderr in the form:
    //   openjdk version "17.0.1" 2021-10-19
    let line = text.lines().next()?;
    let quoted = line.split('"').nth(1)?;
    let major = quoted.split('.').next()?;
    // Java 8 and earlier report as 1.x; modern Java reports the major directly.
    let major: u32 = major.parse().ok()?;
    Some(if major == 1 { 8 } else { major })
}

pub fn get_jvm_args(json: &Value) -> Vec<String> {
    let mut vec = Vec::new();
    if let Some(arguments) = json.get("arguments") {
        if let Some(jvm_rules) = arguments.get("jvm").and_then(|v| v.as_array()) {
            for rule in jvm_rules {
                if let Some(arg_str) = rule.as_str() {
                    vec.push(arg_str.to_string());
                } else if let Some(obj) = rule.as_object() {
                    if let Some(value) = obj.get("value") {
                        if crate::services::utils::can_apply_rule(obj) {
                            match value {
                                Value::String(s) => vec.push(s.clone()),
                                Value::Array(arr) => {
                                    for item in arr {
                                        if let Some(s) = item.as_str() {
                                            vec.push(s.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    vec
}

pub fn get_launch_args(json: &Value) -> Result<Vec<String>, AppError> {
    if json.get("minecraftArguments").is_none() {
        Ok(json["arguments"]["game"]
            .as_array()
            .ok_or_else(|| AppError::LaunchArgsNotFound)?
            .iter()
            .filter(|v| v.is_string())
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<String>>())
    } else {
        Ok(json["minecraftArguments"]
            .as_str()
            .ok_or_else(|| AppError::LaunchArgsNotFound)?
            .split(' ')
            .map(|v| v.to_string())
            .collect::<Vec<String>>())
    }
}

pub fn sanitize_options_file(options_path: &std::path::Path) {
    if !options_path.exists() {
        if let Some(parent) = options_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let default_content = "narrator:0\nonboardAccessibility:false\nnarratorHotkey:false\nskipMultiplayerWarning:true\njoinedFirstServer:true\ntutorialStep:none\nstartedCleanly:true\nsoundCategory_master:1.0\nfov:0.0\n";
        let _ = std::fs::write(options_path, default_content);
        return;
    }

    if let Ok(content) = std::fs::read_to_string(options_path) {
        let has_narrator = content.lines().any(|l| l.trim() == "narrator:0");
        let has_onboard = content.lines().any(|l| l.trim() == "onboardAccessibility:false");
        let has_hotkey = content.lines().any(|l| l.trim() == "narratorHotkey:false");
        let has_joined = content.lines().any(|l| l.trim() == "joinedFirstServer:true");
        let has_tutorial = content.lines().any(|l| l.trim() == "tutorialStep:none");
        let has_skip = content.lines().any(|l| l.trim() == "skipMultiplayerWarning:true");
        let has_clean = content.lines().any(|l| l.trim() == "startedCleanly:true");

        if !has_narrator || !has_onboard || !has_hotkey || !has_joined || !has_tutorial || !has_skip || !has_clean {
            let mut lines: Vec<String> = content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.starts_with("narrator:")
                        && !t.starts_with("onboardAccessibility:")
                        && !t.starts_with("narratorHotkey:")
                        && !t.starts_with("joinedFirstServer:")
                        && !t.starts_with("tutorialStep:")
                        && !t.starts_with("skipMultiplayerWarning:")
                        && !t.starts_with("startedCleanly:")
                })
                .map(|l| l.to_string())
                .collect();

            lines.push("narrator:0".to_string());
            lines.push("onboardAccessibility:false".to_string());
            lines.push("narratorHotkey:false".to_string());
            lines.push("skipMultiplayerWarning:true".to_string());
            lines.push("joinedFirstServer:true".to_string());
            lines.push("tutorialStep:none".to_string());
            lines.push("startedCleanly:true".to_string());

            let _ = std::fs::write(options_path, lines.join("\n") + "\n");
        }
    }
}

pub fn ensure_instance_options(instance_dir: &std::path::Path) {
    let instance_options = instance_dir.join("options.txt");
    sanitize_options_file(&instance_options);

    // Also sanitize root .minecraft/options.txt in case any versions or mods read from it
    let root_options = get_minecraft_directory().join("options.txt");
    sanitize_options_file(&root_options);
}

/// Map a Minecraft version ID (e.g. "1.21.4") to the resource pack format
/// number that version expects. This ensures the GlitchySkin resource pack
/// is recognized as compatible and not moved to the "incompatible" list.
fn version_to_pack_format(version: &str) -> u32 {
    let parts: Vec<&str> = version.split('.').collect();
    let major: u32 = parts.first().and_then(|s| s.parse().ok()).unwrap_or(1);
    let minor: u32 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    match (major, minor, patch) {
        (1, 0..=8, _) => 1,
        (1, 9..=10, _) => 2,
        (1, 11..=12, _) => 3,
        (1, 13..=14, _) => 4,
        (1, 15, _) | (1, 16, 0..=1) => 5,
        (1, 16, _) => 6,
        (1, 17, _) => 7,
        (1, 18, _) => 8,
        (1, 19, 0..=2) => 9,
        (1, 19, 3) => 12,
        (1, 19, _) => 13,
        (1, 20, 0..=1) => 15,
        (1, 20, 2) => 18,
        (1, 20, 3..=4) => 22,
        (1, 20, _) => 32,
        (1, 21, 0..=1) => 34,
        (1, 21, 2..=3) => 42,
        (1, 21, _) => 46,
        _ => 34, // safe default for unknown versions
    }
}
