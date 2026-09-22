#![allow(deprecated)]

use crate::models::versions::MinecraftVersion;
use crate::services::directory_manager::{
    get_assets_directory, get_falcon_launcher_directory, get_libraries_directory,
    get_minecraft_directory, get_natives_folder, get_temp_directory, get_version_directory,
    get_version_manifest, get_versions_directory,
};
use crate::services::utils::{
    convert_to_full_path, convert_to_full_url, fetch_library_path, fetch_rules,
    fetch_unofficial_library_repos, is_legacy, verify_file_existence,
};
use crate::services::version_manager::load_version_manifest;

use crate::models::config::Config;
use crate::models::downloader::{
    AssetIndex, AssetObjects, DownloadDetail, ForgeInstallProfile, ForgeVersionJsonInfo, Library,
    LibraryArtifact, Manifest, MinecraftManifestVersion, VersionLoader,
};
use crate::models::error::AppError;
use crate::models::fabric::{FabricInstaller, FabricLoader, FabricMinecraftVersion};
use crate::models::logger::LogLine;
use crate::models::mirror::{self, Mirror};
use crate::models::platform::get_current_os;
use crate::models::utils::{LowerCaseStartsWith, ParseWithMirror};
use crate::services::download_manager::{self, DownloadJob, DownloadToken, VerifySpec};
use crate::services::download_session::SessionHandle;
use crate::services::http;
use crate::services::jdk_manager::{download_java, get_java};
use crate::GLOBAL_CACHE;
use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::fs::{create_dir_all, exists, File};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::mpsc::UnboundedSender;
use zip::ZipArchive;
use zip_extract::extract;

pub async fn download_version(
    version: &MinecraftVersion,
    name: &String,
    app_handle: &AppHandle,
    logger: &UnboundedSender<LogLine>,
    cfg: &Config,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let id = &version.id;
    // Probe the configured mirror and fall back to the built-in 9Craft
    // mirror when Mojang's CDNs are unreachable (common in filtered
    // regions) — otherwise every downstream download fails.
    let resolved_mirror = mirror::resolve_effective(&cfg.download_settings.mirror).await;
    let mirror = &resolved_mirror;
    let name = if name == "" { &version.id } else { name };

    info!("Downloading version {} with name of {name}", &version.id);

    session.set_phase("prepare");
    let manifest = load_version_manifest(&mirror).await?;
    download_from_manifest(id, &manifest, &mirror)
        .await
        .or_else(|x| {
            // If the version JSON already exists on disk (e.g. it was
            // created by a loader installer like Fabric/Forge), the
            // manifest lookup failure is expected — continue.
            if get_version_manifest(id).exists() {
                Ok(())
            } else {
                Err(x)
            }
        })?;
    let content = fs::read_to_string(PathBuf::from(version.get_json())).map_err(|x| {
        let p = version.get_json();
        let parent = PathBuf::from(&p)
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        AppError::FileReadFailed(format!(
            "version json: {x} | path={p} | parent={parent} | exists={} | parent_exists={}",
            PathBuf::from(&p).exists(),
            PathBuf::from(&parent).exists(),
        ))
    })?;
    let json: MinecraftManifestVersion =
        serde_json::from_str(&content).map_err(|x| AppError::JsonParseFailed(x.to_string()))?;
    let java_version = if json.inherits_from.is_none() {
        json.java_version.clone().ok_or_else(|| {
            AppError::JsonParseFailed(format!(
                "version {} has no javaVersion field and no inheritsFrom",
                id
            ))
        })?
    } else {
        let inherited_id = json.inherits_from.as_ref().unwrap();
        let dir = get_version_manifest(inherited_id);
        let inherited_content = fs::read_to_string(&dir).map_err(|x| {
            AppError::FileReadFailed(format!(
                "inherited version {} json at {}: {x}",
                inherited_id,
                dir.display()
            ))
        })?;
        let m: MinecraftManifestVersion =
            serde_json::from_str(&inherited_content).map_err(|x| {
                AppError::JsonParseFailed(format!("inherited version {inherited_id}: {x}"))
            })?;
        m.java_version.clone().ok_or_else(|| {
            AppError::JsonParseFailed(format!(
                "inherited version {inherited_id} has no javaVersion field"
            ))
        })?
    };
    download_java(
        &java_version.component.to_string(),
        &java_version.major_version.to_string(),
        logger,
        &mirror,
        Some(session),
    )
    .await?;

    download_libraries(&json.libraries, &id, app_handle, logger, &mirror, session).await?;

    if let Some(downloads) = &json.downloads {
        if let Some(client_download) = downloads.get("client") {
            info!("Downloading client's process has started.");
            download_client(client_download, &id, logger, &mirror, session).await?;
        }
    }

    if let Some(asset_index) = &json.asset_index {
        info!("Downloading assets process has started.");
        download_assets(asset_index, logger, &mirror, app_handle, session).await?;
    }
    if let Some(logging) = &json.logging {
        info!("Downloading logger files process has started.");
        let log_filename = logging
            .client
            .file
            .url
            .split('/')
            .last()
            .unwrap_or("log.xml");
        download_file_if_not_exists(
            &get_version_directory(id).join(log_filename),
            logging.client.file.url.clone(),
            logging.client.file.size,
        )
        .await?;
    }

    Ok(())
}

async fn download_assets(
    value: &AssetIndex,
    logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    app_handle: &AppHandle,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let id = &value.id;
    let url = mirror.parse_url(&value.url);
    let total_size = value.total_size;
    let asset_index_path = get_assets_directory()
        .join("indexes")
        .join(format!("{id}.json"));
    download_file_verified(
        &asset_index_path,
        url.to_string(),
        total_size,
        Some(value.sha1.clone()),
    )
    .await?;
    let content = fs::read_to_string(&asset_index_path)
        .map_err(|x| AppError::FileReadFailed(format!("asset index {id}: {x}")))?;

    let json: AssetObjects = serde_json::from_str(&content)
        .map_err(|x| AppError::JsonParseFailed(format!("asset index {id}: {x}")))?;
    let url_template = "https://resources.download.minecraft.net/{id}/{hash}";

    let total_objects = json.objects.len();
    if total_objects == 0 {
        warn!("asset index {id} contains 0 objects — possibly corrupt");
    }

    // Queue every asset object as a DownloadManager job: concurrent
    // (semaphore-bounded), retried with backoff, resumable and SHA-1
    // verified. The old serial loop aborted the whole install on the
    // first transient failure of any of the thousands of objects.
    let mut jobs: Vec<DownloadJob> = Vec::with_capacity(total_objects);
    for (_name, asset_entry) in json.objects.iter() {
        let hash = &asset_entry.hash;
        if hash.len() < 2 {
            warn!("asset entry has invalid hash: {hash}");
            continue;
        }
        let prefix_id = hash[0..2].to_string();
        let url = mirror.parse_url(
            &url_template
                .replace("{id}", prefix_id.as_str())
                .replace("{hash}", hash),
        );
        let path = get_assets_directory()
            .join("objects")
            .join(prefix_id.as_str())
            .join(hash);
        jobs.push(DownloadJob {
            url,
            destination: path.to_string_lossy().to_string(),
            expected_size: Some(asset_entry.size),
            verify: Some(VerifySpec::sha1(hash.clone())),
            category: "assets".to_string(),
        });
    }

    // Assets occupy the second half of the overall bar (50–100%).
    session.set_phase("assets");
    let session = session.clone();
    let token = session.token();
    let progress_session = session.clone();
    let on_progress = Arc::new(move |done: u32, total: u32, current_file: String| {
        if total > 0 {
            progress_session.progress(done as f64 / total as f64, &current_file);
        }
    });
    let transfer_session = session.clone();
    let on_transfer = Arc::new(move |downloaded: u64, total: u64, current_file: String| {
        transfer_session.transfer(downloaded, total, &current_file);
    });
    download_manager::global()
        .download_batch_with_callbacks(
            jobs,
            Some(app_handle),
            &token,
            Some(on_progress),
            Some(on_transfer),
        )
        .await
}

pub async fn download_file_if_not_exists(
    path: &PathBuf,
    url: String,
    size: u64,
) -> Result<(), AppError> {
    if !verify_file_existence(&path.to_str().unwrap().to_string(), size) {
        download_file_verified(path, url, size, None).await?;
    }
    Ok(())
}

/// Download `url` to `path` through the central [`DownloadManager`],
/// which adds connect/read timeouts, retries with exponential backoff,
/// `.part` resume, atomic replacement and — when `sha1` is known —
/// hash verification. Files that already exist and verify OK are
/// skipped.
pub(crate) async fn download_file_verified(
    path: &PathBuf,
    url: String,
    size: u64,
    sha1: Option<String>,
) -> Result<(), AppError> {
    download_file_verified_for_session(path, url, size, sha1, None).await
}

pub(crate) async fn download_file_verified_for_session(
    path: &PathBuf,
    url: String,
    size: u64,
    sha1: Option<String>,
    session: Option<&SessionHandle>,
) -> Result<(), AppError> {
    // A known hash is the authoritative check; the size is only used
    // as a fast pre-check when no hash is available.
    let verify = sha1
        .as_deref()
        .filter(|h| !h.is_empty())
        .map(VerifySpec::sha1);
    let expected_size = if size > 0 {
        Some(size)
    } else {
        None
    };
    let job = DownloadJob {
        url,
        destination: path.to_string_lossy().to_string(),
        expected_size,
        verify,
        category: "game-files".to_string(),
    };
    if let Some(session) = session {
        let transfer_session = session.clone();
        let on_transfer = Arc::new(move |downloaded: u64, total: u64, current_file: String| {
            transfer_session.transfer(downloaded, total, &current_file);
        });
        download_manager::global()
            .download_batch_with_callbacks(
                vec![job],
                None,
                &session.token(),
                None,
                Some(on_transfer),
            )
            .await
    } else {
        download_manager::global()
            .download_one(job, None, &DownloadToken::new())
            .await
    }
}

pub(crate) async fn download_from_manifest(
    id: &String,
    manifest: &Manifest,
    mir: &Mirror,
) -> Result<(), AppError> {
    let version =
        manifest
            .versions
            .iter()
            .find(|v| &v.id == id)
            .ok_or(AppError::ManifestParseFailed(format!(
                "Couldn't find version in manifest. {id}"
            )))?;
    let version_url = mir.parse_url(&version.url);
    let destination = get_version_directory(id).join(format!("{id}.json"));
    download_file_verified(&destination, version_url, 0, version.sha1.clone()).await
}

async fn download_client(
    value: &DownloadDetail,
    version: &String,
    logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let size = value.size;
    let sha1 = value.sha1.clone();
    let url = mirror.parse_url(&value.url);
    let path = get_versions_directory()
        .join(&version)
        .join(format!("{}.jar", version));
    let file_name = format!("{}.jar", version);
    session.set_phase("client");
    session.set_current_file(&file_name);
    download_file_verified_for_session(&path, url.to_string(), size, Some(sha1), Some(session))
        .await?;
    session.progress(1.0, &file_name);
    Ok(())
}

async fn download_libraries(
    libraries: &[Library],
    version: &String,
    app_handle: &AppHandle,
    logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let libraries_path = get_libraries_directory();
    session.set_phase("libraries");
    let os = get_current_os();

    let mut batch_jobs: Vec<DownloadJob> = Vec::new();
    let mut classifiers_to_download: Vec<HashMap<String, LibraryArtifact>> = Vec::new();

    for library in libraries {
        if library.downloads.is_none() {
            let name = library.name.replace(":", "/");
            let path = fetch_library_path(&name)?;
            let full_path = get_libraries_directory().join(&path);
            if let Some(base) = library.url.as_deref().filter(|u| !u.is_empty()) {
                let url = format!("{}/{}", base.trim_end_matches('/'), path);
                batch_jobs.push(DownloadJob {
                    url,
                    destination: full_path.to_string_lossy().to_string(),
                    expected_size: None,
                    verify: None,
                    category: "library".to_string(),
                });
            } else if name.starts_with_lower_case("net/minecraft") {
                let url = mirror.parse_url(&format!("https://libraries.minecraft.net/{path}"));
                batch_jobs.push(DownloadJob {
                    url,
                    destination: full_path.to_string_lossy().to_string(),
                    expected_size: None,
                    verify: None,
                    category: "library".to_string(),
                });
            } else {
                let urls = fetch_unofficial_library_repos(&path);
                for url in urls {
                    if http::get(&url)
                        .await
                        .map(|r| r.status().is_success())
                        .unwrap_or(false)
                    {
                        batch_jobs.push(DownloadJob {
                            url,
                            destination: full_path.to_string_lossy().to_string(),
                            expected_size: None,
                            verify: None,
                            category: "library".to_string(),
                        });
                        break;
                    }
                }
            }
            continue;
        }

        let downloads = library.downloads.as_ref().unwrap();
        if let Some(classifiers) = &downloads.classifiers {
            classifiers_to_download.push(classifiers.clone());
        }

        let Some(library_artifact) = library
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
        else {
            continue;
        };

        let library_path = if library_artifact.path.is_none() {
            let args = library.name.split(":").collect::<Vec<&str>>();
            let group_id = args[0].replace(".", "/");
            let artifact = args[1];
            let version = args[2];
            let artifact_version = format!("{artifact}-{version}.jar");
            format!("{group_id}/{artifact}/{version}/{artifact_version}")
        } else {
            library_artifact.path.as_ref().unwrap().to_string()
        };

        let rules = fetch_rules(library.rules.as_ref());
        if rules.allowed_oses.contains(&os) && !rules.disallowed_oses.contains(&os) {
            let path = libraries_path.join(&library_path);
            batch_jobs.push(DownloadJob {
                url: mirror.parse_url(&library_artifact.url),
                destination: path.to_string_lossy().to_string(),
                expected_size: Some(library_artifact.size),
                verify: library_artifact.sha1.clone().map(VerifySpec::sha1),
                category: "library".to_string(),
            });
        }
    }

    if !batch_jobs.is_empty() {
        let session_clone = session.clone();
        let progress_cb = Some(Arc::new(move |done: u32, total: u32, file: String| {
            session_clone.progress(done as f64 / total.max(1) as f64, &file);
        }) as Arc<dyn Fn(u32, u32, String) + Send + Sync>);
        let token = DownloadToken::new();
        download_manager::global()
            .download_batch_with_progress(batch_jobs, Some(app_handle), &token, progress_cb)
            .await?;
    }

    for classifiers in classifiers_to_download {
        download_classifiers(Some(&classifiers), version, mirror).await?;
    }

    Ok(())
}

async fn download_classifiers(
    classifiers: Option<&HashMap<String, LibraryArtifact>>,
    version: &String,
    mirror: &Mirror,
) -> Result<(), AppError> {
    let Some(classifiers_map) = classifiers else {
        return Ok(());
    };
    let os = get_current_os();
    let mut natives = classifiers_map.get(&format!("natives-{os}"));
    if natives.is_none() && os == "windows" {
        natives = classifiers_map.get(&format!("natives-{os}-64"));
    }
    if let Some(val) = natives {
        let url = mirror.parse_url(&val.url.to_string());
        let url_https_less = url.replace("https://", "").replace("http://", "");
        let path = if let Some(ref p) = val.path {
            p.clone()
        } else {
            let url_args = url_https_less.split("/").collect::<Vec<&str>>();
            url_https_less.replace(url_args[0], "")
        };

        let full_path = get_libraries_directory().join(path);
        let size = val.size;

        download_file_verified(&full_path, url.to_string(), size, val.sha1.clone()).await?;

        if let Ok(file) = File::open(&full_path) {
            let natives_path = get_natives_folder(version);
            if !natives_path.exists() {
                create_dir_all(&natives_path).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
            }
            extract(file, &natives_path, false).map_err(|_| {
                AppError::ZipExtractionFailed("Zip extraction of classifier failed".to_string())
            })?;
        }
    }
    Ok(())
}

pub async fn download_file(url: String, dest: &String) -> Result<(), AppError> {
    // Delegates to the central DownloadManager so this path also gets
    // timeouts, retries and atomic `.part` handling instead of a bare
    // `reqwest::get` that aborts on the first transient network error.
    let job = DownloadJob {
        url: url.clone(),
        destination: dest.clone(),
        expected_size: None,
        verify: None,
        category: "misc".to_string(),
    };
    download_manager::global()
        .download_one(job, None, &DownloadToken::new())
        .await
        .map_err(|e| AppError::DownloadFailed(format!("{}: {e}", url)))
}

pub async fn get_available_forge_versions(
    version_id: &String,
    mirror: &Mirror,
) -> Result<Vec<String>, AppError> {
    let cached = {
        let global_cache = GLOBAL_CACHE.lock().await;
        global_cache.forge.clone()
    };
    let map = if let Some(m) = cached {
        m
    } else {
        let url = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json"
            .parse_mirror(&mirror);

        let m: HashMap<String, Vec<String>> = http::get_json(&url).await?;
        let mut global_cache = GLOBAL_CACHE.lock().await;
        global_cache.forge = Some(m.clone());
        m
    };
    Ok(map
        .iter()
        .find(|(key, _)| key.as_str() == version_id.as_str())
        .map(|(_key, val)| val.clone())
        .unwrap_or(Vec::new()))
}

pub async fn fetch_forge_mirrors(content: String) -> Vec<String> {
    let mut vec = Vec::new();
    for line in content.split("\n") {
        let args = line.split("!").collect::<Vec<&str>>();
        vec.push(args[args.len() - 1].to_string());
    }
    vec
}
pub async fn download_forge_version(
    version: &String,
    _app_handle: &AppHandle,
    _logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    ver: &mut String,
    session: &SessionHandle,
) -> Result<(), AppError> {
    session.set_phase("loader");
    let url = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-installer.jar").parse_mirror(mirror);
    let launcher_dir = get_falcon_launcher_directory();
    info!("{}", url);
    let mut path = launcher_dir.join("temp");
    info!("{}", path.display());

    if !path.exists() {
        create_dir_all(&path).map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    }

    path = path.join(format!("forge-{version}-installer.jar"));
    let installer_sha1 = fetch_remote_sha1(&format!("{url}.sha1")).await;
    download_jar_verified(&path, url, installer_sha1).await?;

    let mc_version = version
        .split_once('-')
        .map_or(version.as_str(), |(mc, _)| mc);
    if !is_legacy(&version) {
        info!("Modern Forge version detected");

        let parent = MinecraftVersion::from_id(mc_version.to_string());
        let parent_json = parent.load_json();
        let java_component = parent_json["javaVersion"]["component"]
            .as_str()
            .unwrap_or("jre-legacy");
        let java = get_java(java_component.to_string())?;

        // Forge's official installer checks for one of the Mojang launcher
        // profile files even when invoked headlessly. A custom launcher has
        // neither on a clean install, so create the minimal compatible file.
        let launcher_profiles = get_minecraft_directory().join("launcher_profiles.json");
        if !launcher_profiles.exists() {
            fs::write(
                &launcher_profiles,
                br#"{"profiles":{},"settings":{},"version":3}"#,
            )
            .map_err(|error| {
                AppError::FileWriteFailed(format!("{}: {error}", launcher_profiles.display()))
            })?;
        }

        let mut child = Command::new(java.get_cli_bin_file())
            .arg("-jar")
            .arg(&path)
            .arg("--installClient")
            .arg(get_minecraft_directory())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(get_temp_directory())
            .spawn()
            .map_err(|error| {
                AppError::UnknownError(format!("failed to start Forge installer: {error}"))
            })?;

        if let Some(stderr) = child.stderr.take() {
            spawn_thread(stderr, format!("forge_installer_{version}"));
        }

        generate_stdout(&mut child, format!("forge_installer_{version}"));
        let status = child.wait().map_err(|error| {
            AppError::UnknownError(format!("failed to wait for Forge installer: {error}"))
        })?;
        if !status.success() {
            return Err(AppError::UnknownError(format!(
                "Forge installer exited with status {status} for {version}"
            )));
        }

        let expected_profile = get_version_manifest(ver);
        if !expected_profile.exists() {
            return Err(AppError::FileNotFound(format!(
                "Forge installer reported success but did not create {}",
                expected_profile.display()
            )));
        }
        let _ = fs::remove_file(&path);

        return Ok(());
    }
    info!("DEBUG: Legacy version detected!");
    let installer_file = File::open(&path)
        .map_err(|e| AppError::FileReadFailed(format!("{}: {e}", path.display())))?;

    let mut zip = ZipArchive::new(installer_file)
        .map_err(|e| AppError::ZipExtractionFailed(e.to_string()))?;
    let install_profile_file = zip.by_name("install_profile.json").map_err(|_| {
        AppError::ProfileNotFound("Failed to find install_profile.json".to_string())
    })?;

    let install_profile_json: ForgeInstallProfile =
        serde_json::from_reader(install_profile_file)
            .map_err(|e| AppError::JsonParseFailed(e.to_string()))?;
    if let Some(install_data) = &install_profile_json.install {
        let mut forge = zip.by_name(&install_data.file_path)
            .map_err(|e| AppError::ZipExtractionFailed(e.to_string()))?;
        let path_maven = &install_data.path;
        let args = path_maven.split(":").collect::<Vec<&str>>();
        let group_id = args[0].replace(".", "/");
        let artifact = args[1];
        let version = args[2];
        let artifact_version = format!("{artifact}-{version}");
        let full_path = get_libraries_directory().join(format!(
            "{group_id}/{artifact}/{version}/{artifact_version}.jar"
        ));
        let mirror_list = mirror.parse_url(&install_data.mirror_list);
        let resp = http::get(&mirror_list).await.map(async |x| x.text().await);
        if resp.is_ok() {
            let resp = resp.unwrap().await.map(|s| fetch_forge_mirrors(s));
            if resp.is_ok() {
                let _mirrors = resp.unwrap().await;
            }
        }
        if let Some(parent) = full_path.parent() {
            create_dir_all(parent)
                .map_err(|_| AppError::DirCreateFailed("Failed to create the path".to_string()))?;
        }

        let mut file = File::create(&full_path)
            .map_err(|e| AppError::FileCreateFailed(e.to_string()))?;
        std::io::copy(&mut forge, &mut file)
            .map_err(|_| AppError::FileCopyFailed("Failed to copy files".to_string()))?;
    }

    let version_json: ForgeVersionJsonInfo = if install_profile_json.version_info.is_none() {
        let versions_file = zip.by_name("version.json")
            .map_err(|e| AppError::ProfileNotFound(e.to_string()))?;
        serde_json::from_reader(versions_file)
            .map_err(|e| AppError::JsonParseFailed(e.to_string()))?
    } else {
        install_profile_json.version_info.clone()
            .ok_or_else(|| AppError::ProfileNotFound("missing versionInfo".to_string()))?
    };

    let version_id = &version_json.id;
    *ver = version_id.clone();
    let version_folder = get_version_directory(&version_id.to_string());
    if !version_folder.exists() {
        create_dir_all(&version_folder)
            .map_err(|e| AppError::DirCreateFailed(e.to_string()))?;
    }
    let version_json_path =
        get_version_directory(&version_id.to_string()).join(format!("{version_id}.json"));

    let serialized_json = serde_json::to_string(&version_json)
        .map_err(|e| AppError::JsonParseFailed(e.to_string()))?;
    fs::write(&version_json_path, serialized_json)
        .map_err(|_| AppError::FileWriteFailed("Failed to write to the forge json file.".to_string()))?;

    if let Some(profile_libraries) = &install_profile_json.libraries {
        for library in profile_libraries {
            if let Some(downloads) = &library.downloads {
                if let Some(artifact) = &downloads.artifact {
                    let url = &artifact.url;
                    if url == "" {
                        if let Some(path) = &artifact.path {
                            let zip_path = format!("maven/{}", path);
                            let mut f = zip.by_name(&zip_path).map_err(|_| {
                                AppError::ZipParseFailed("Parsing zip file failed.".to_string())
                            })?;
                            let lib_dest = get_libraries_directory().join(path);
                            if let Some(parent) = lib_dest.parent() {
                                create_dir_all(parent).map_err(|_| {
                                    AppError::DirCreateFailed(
                                        "Failed to create the directory".to_string(),
                                    )
                                })?;
                            }
                            let mut file = File::create(&lib_dest)
                                .map_err(|e| AppError::FileCreateFailed(e.to_string()))?;
                            std::io::copy(&mut f, &mut file).map_err(|_| {
                                AppError::FileCopyFailed("Failed to copy files".to_string())
                            })?;
                        }
                        continue;
                    }

                    let full_url = if url.ends_with("/") {
                        convert_to_full_url(url, &library.name)?
                    } else {
                        url.to_string()
                    };

                    let full_path = if artifact.path.is_none() {
                        convert_to_full_path(
                            get_libraries_directory().to_str().unwrap_or(""),
                            &library.name,
                        )?
                    } else {
                        artifact.path.as_ref().unwrap().to_string()
                    };

                    download_file_if_not_exists(&PathBuf::from(full_path), full_url, 0).await?;
                }
            }
        }
    }

    for library in &version_json.libraries {
        if let Some(url) = &library.url {
            let full_url = convert_to_full_url(url, &library.name)?;
            let full_path = convert_to_full_path(
                get_libraries_directory().to_str().unwrap_or(""),
                &library.name,
            )?;

            download_file_if_not_exists(&PathBuf::from(full_path), full_url, 0).await?;
        }
    }

    let _ = fs::remove_file(path);
    Ok(())
}

async fn fetch_remote_sha1(url: &str) -> Option<String> {
    let response = http::get(url).await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().await.ok()?;
    let checksum = body.split_whitespace().next()?.trim();
    if checksum.len() == 40
        && checksum
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        Some(checksum.to_string())
    } else {
        None
    }
}

async fn download_jar_verified(
    destination: &PathBuf,
    url: String,
    sha1: Option<String>,
) -> Result<(), AppError> {
    download_file_verified(destination, url.clone(), 0, sha1.clone()).await?;
    if validate_zip_archive(destination).is_ok() {
        return Ok(());
    }

    // A previous build may have cached an HTML error page or a truncated
    // installer without a published checksum. Remove that exact file and
    // retry once before surfacing a clear ZIP error.
    let _ = fs::remove_file(destination);
    download_file_verified(destination, url, 0, sha1).await?;
    validate_zip_archive(destination)
}

fn validate_zip_archive(path: &PathBuf) -> Result<(), AppError> {
    let file = File::open(path)
        .map_err(|error| AppError::FileReadFailed(format!("{}: {error}", path.display())))?;
    ZipArchive::new(file)
        .map(|_| ())
        .map_err(|error| AppError::ZipParseFailed(format!("{}: {error}", path.display())))
}

fn spawn_thread(stderr: ChildStderr, task_name: String) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().flatten() {
            info!("[{task_name}][stderr] {}", line);
        }
    });
}

pub async fn download_fabric(
    version_loader: &VersionLoader,
    logger: &UnboundedSender<LogLine>,
    mirror: &Mirror,
    session: &SessionHandle,
) -> Result<(), AppError> {
    let _ = (logger, mirror); // meta.fabricmc.net has no mirror remap; http::get retries
    session.set_phase("loader");

    let mc_version = version_loader.get_fabric_version_id();
    let loader_version = version_loader.get_fabric_loader_id();
    if mc_version.is_empty() || loader_version.is_empty() {
        return Err(AppError::VersionNotFound(format!(
            "invalid fabric version id '{}'",
            version_loader.id
        )));
    }

    // The profile endpoint returns the complete loader version JSON
    // (id, inheritsFrom, mainClass, libraries with their maven base
    // URLs). Writing it directly replaces the old "download
    // installer.jar and run it with Java 8" flow, which failed whenever
    // maven.fabricmc.net was unreachable from inside the installer JVM
    // or Java 8 could not be provisioned — and whose library downloads
    // bypassed our retrying DownloadManager entirely.
    let profile_url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{mc_version}/{loader_version}/profile/json"
    );
    let resp = http::get(&profile_url).await?;
    if !resp.status().is_success() {
        return Err(AppError::VersionNotFound(format!(
            "fabric profile for loader {loader_version} / minecraft {mc_version} not found (HTTP {})",
            resp.status()
        )));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| AppError::NetworkRequestFailed(format!("{profile_url}: {e}")))?;

    let profile: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| AppError::JsonParseFailed(format!("fabric profile json: {e}")))?;
    if profile["libraries"]
        .as_array()
        .map_or(true, |a| a.is_empty())
    {
        return Err(AppError::ManifestParseFailed(
            "fabric profile json has no libraries".to_string(),
        ));
    }

    let profile_id = format!("fabric-loader-{loader_version}-{mc_version}");
    let version_dir = get_version_directory(&profile_id);
    fs::create_dir_all(&version_dir)
        .map_err(|e| AppError::DirCreateFailed(format!("{}: {e}", version_dir.display())))?;
    let json_path = version_dir.join(format!("{profile_id}.json"));
    fs::write(&json_path, &body)
        .map_err(|e| AppError::FileWriteFailed(format!("{}: {e}", json_path.display())))?;
    info!("Fabric profile written: {}", json_path.display());
    session.progress(1.0, &format!("{profile_id}.json"));

    Ok(())
}

pub async fn download_optifine(
    version_loader: &VersionLoader,
    session: &SessionHandle,
) -> Result<(), AppError> {
    session.set_phase("loader");
    let minecraft_version = version_loader.get_optifine_version_id();
    let (kind, patch) = version_loader.get_optifine_release().ok_or_else(|| {
        AppError::VersionNotFound(format!(
            "invalid OptiFine version id '{}'",
            version_loader.id
        ))
    })?;
    if minecraft_version.is_empty() {
        return Err(AppError::VersionNotFound(format!(
            "invalid OptiFine version id '{}'",
            version_loader.id
        )));
    }

    let profile_id = version_loader.get_installed_id();
    let installer_path = get_temp_directory().join(format!("{profile_id}-installer.jar"));
    let download_url =
        format!("https://bmclapi2.bangbang93.com/optifine/{minecraft_version}/{kind}/{patch}");
    session.progress(0.05, "OptiFine installer");
    if let Err(mirror_error) = download_jar_verified(&installer_path, download_url, None).await {
        warn!("OptiFine mirror failed, trying official mirror: {mirror_error}");
        let filename = version_loader.get_optifine_filename().ok_or_else(|| {
            AppError::VersionNotFound(format!(
                "OptiFine catalog entry has no safe filename: '{}'",
                version_loader.id
            ))
        })?;
        download_optifine_from_official_mirror(&installer_path, &filename)
            .await
            .map_err(|official_error| {
                AppError::DownloadFailed(format!(
                    "OptiFine mirror failed ({mirror_error}); official mirror failed ({official_error})"
                ))
            })?;
    }

    let parent = MinecraftVersion::from_id(minecraft_version.clone());
    let parent_json = parent.load_json();
    let java_component = parent_json["javaVersion"]["component"]
        .as_str()
        .unwrap_or("jre-legacy");
    let java = get_java(java_component.to_string())?;
    let vanilla_jar =
        get_version_directory(&minecraft_version).join(format!("{minecraft_version}.jar"));
    if !vanilla_jar.exists() {
        return Err(AppError::FileNotFound(
            vanilla_jar.to_string_lossy().to_string(),
        ));
    }

    let library_version = format!("{minecraft_version}_{kind}_{patch}");
    let optifine_jar = get_libraries_directory()
        .join("optifine")
        .join("OptiFine")
        .join(&library_version)
        .join(format!("OptiFine-{library_version}.jar"));
    if let Some(parent_directory) = optifine_jar.parent() {
        fs::create_dir_all(parent_directory)
            .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    }

    session.progress(0.3, "Patching OptiFine");
    let patch_output = Command::new(java.get_cli_bin_file())
        .arg("-cp")
        .arg(&installer_path)
        .arg("optifine.Patcher")
        .arg(&vanilla_jar)
        .arg(&installer_path)
        .arg(&optifine_jar)
        .output()
        .map_err(|error| {
            AppError::UnknownError(format!("failed to start OptiFine patcher: {error}"))
        })?;
    if !patch_output.status.success() {
        let stderr = String::from_utf8_lossy(&patch_output.stderr);
        let stdout = String::from_utf8_lossy(&patch_output.stdout);
        return Err(AppError::UnknownError(format!(
            "OptiFine patcher failed: {} {}",
            stdout.trim(),
            stderr.trim()
        )));
    }

    let is_modern = {
        let parts: Vec<u32> = minecraft_version
            .split('.')
            .filter_map(|p| p.parse::<u32>().ok())
            .collect();
        if parts.len() >= 2 {
            parts[0] > 1 || (parts[0] == 1 && parts[1] >= 13)
        } else {
            false
        }
    };

    let now = chrono::Utc::now().to_rfc3339();
    let profile = if is_modern {
        serde_json::json!({
            "id": profile_id,
            "inheritsFrom": minecraft_version,
            "time": now,
            "releaseTime": now,
            "type": "release",
            "mainClass": "net.minecraft.client.main.Main",
            "arguments": {
                "game": [],
                "jvm": []
            },
            "libraries": [
                { "name": format!("optifine:OptiFine:{library_version}") }
            ]
        })
    } else {
        let launchwrapper = extract_optifine_launchwrapper(&installer_path)?
            .unwrap_or_else(|| "net.minecraft:launchwrapper:1.12".to_string());
        serde_json::json!({
            "id": profile_id,
            "inheritsFrom": minecraft_version,
            "time": now,
            "releaseTime": now,
            "type": "release",
            "mainClass": "net.minecraft.launchwrapper.Launch",
            "minimumLauncherVersion": 21,
            "arguments": {
                "game": ["--tweakClass", "optifine.OptiFineTweaker"],
                "jvm": []
            },
            "libraries": [
                { "name": launchwrapper },
                { "name": format!("optifine:OptiFine:{library_version}") }
            ]
        })
    };
    let profile_directory = get_version_directory(&profile_id);
    fs::create_dir_all(&profile_directory)
        .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
    let profile_path = profile_directory.join(format!("{profile_id}.json"));
    let profile_json = serde_json::to_vec_pretty(&profile)
        .map_err(|error| AppError::JsonParseFailed(error.to_string()))?;
    fs::write(&profile_path, profile_json)
        .map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
    let _ = fs::remove_file(installer_path);
    session.progress(1.0, &format!("{profile_id}.json"));
    info!("OptiFine profile written: {}", profile_path.display());
    Ok(())
}

async fn download_optifine_from_official_mirror(
    destination: &PathBuf,
    filename: &str,
) -> Result<(), AppError> {
    let landing_url = format!("https://www.optifine.net/adloadx?f={filename}");
    let response = http::get(&landing_url).await?;
    if !response.status().is_success() {
        return Err(AppError::DownloadFailed(format!(
            "OptiFine mirror page returned HTTP {}",
            response.status()
        )));
    }
    let html = response
        .text()
        .await
        .map_err(|error| AppError::NetworkRequestFailed(error.to_string()))?;
    let download_url = extract_optifine_download_url(&html).ok_or_else(|| {
        AppError::ManifestParseFailed("OptiFine mirror page has no download link".to_string())
    })?;
    download_jar_verified(destination, download_url, None).await
}

fn extract_optifine_download_url(html: &str) -> Option<String> {
    let marker = "downloadx?f=";
    let start = html.find(marker)?;
    let link = &html[start..];
    let end = link
        .find(|character: char| {
            character == '\"'
                || character == '\''
                || character == '<'
                || character.is_ascii_whitespace()
        })
        .unwrap_or(link.len());
    let relative = link[..end].replace("&amp;", "&");
    Some(format!("https://www.optifine.net/{relative}"))
}

fn extract_optifine_launchwrapper(installer_path: &PathBuf) -> Result<Option<String>, AppError> {
    let installer =
        File::open(installer_path).map_err(|error| AppError::FileReadFailed(error.to_string()))?;
    let mut archive = ZipArchive::new(installer)
        .map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::ZipExtractionFailed(error.to_string()))?;
        let Some(filename) = PathBuf::from(entry.name())
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        let Some(version) = filename
            .strip_prefix("launchwrapper-of-")
            .and_then(|name| name.strip_suffix(".jar"))
        else {
            continue;
        };
        if version.is_empty()
            || !version
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || ".-_".contains(character))
        {
            continue;
        }

        let destination = get_libraries_directory()
            .join("optifine")
            .join("launchwrapper-of")
            .join(version)
            .join(&filename);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| AppError::DirCreateFailed(error.to_string()))?;
        }
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| AppError::FileReadFailed(error.to_string()))?;
        fs::write(&destination, bytes)
            .map_err(|error| AppError::FileWriteFailed(error.to_string()))?;
        return Ok(Some(format!("optifine:launchwrapper-of:{version}")));
    }

    Ok(None)
}

pub fn generate_stdout(child: &mut Child, task_name: String) {
    if let Some(stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    info!("[{task_name}][stdout] {}", line);
                }
            }
        });
    }
}

pub async fn get_available_fabric_versions(version_id: &String) -> Result<Vec<String>, AppError> {
    let (cached_mc, cached_installers, cached_loaders) = {
        let global_cache = GLOBAL_CACHE.lock().await;
        (
            global_cache.fabric_mc_versions.clone(),
            global_cache.fabric_installers.clone(),
            global_cache.fabric_loaders.clone(),
        )
    };

    let mc_versions = if let Some(m) = cached_mc {
        m
    } else {
        let url = "https://meta.fabricmc.net/v2/versions/game";
        let map: Vec<FabricMinecraftVersion> = http::get_json(url).await?;
        let mut global_cache = GLOBAL_CACHE.lock().await;
        global_cache.fabric_mc_versions = Some(map.clone());
        map
    };

    let _installers = if let Some(i) = cached_installers {
        i
    } else {
        let url = "https://meta.fabricmc.net/v2/versions/installer";
        let map: Vec<FabricInstaller> = http::get_json(url).await?;
        let mut global_cache = GLOBAL_CACHE.lock().await;
        global_cache.fabric_installers = Some(map.clone());
        map
    };

    let loaders = if let Some(l) = cached_loaders {
        l
    } else {
        let url = "https://meta.fabricmc.net/v2/versions/loader";
        let map: Vec<FabricLoader> = http::get_json(url).await?;
        let mut global_cache = GLOBAL_CACHE.lock().await;
        global_cache.fabric_loaders = Some(map.clone());
        map
    };

    let v = mc_versions.iter().find(|x| &x.version == version_id);
    if v.is_none() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    for loader in loaders {
        result.push(format!("{}-{}", version_id.to_string(), loader.version));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::extract_optifine_download_url;

    #[test]
    fn extracts_official_optifine_download_link() {
        let html = r#"<a href="downloadx?f=preview_OptiFine_1.21_HD_U_J1_pre1.jar&amp;x=token">Download</a>"#;
        assert_eq!(
            extract_optifine_download_url(html).as_deref(),
            Some("https://www.optifine.net/downloadx?f=preview_OptiFine_1.21_HD_U_J1_pre1.jar&x=token")
        );
    }
}
