use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flate2::read::GzDecoder;
use tauri::{AppHandle, Emitter, State};

/// The compressed executable for Glitchy Launcher, embedded at compile time.
const LAUNCHER_GZ: &[u8] = include_bytes!("../resources/launcher.gz");

struct InstallState {
    cancelled: Arc<Mutex<bool>>,
}

#[tauri::command]
fn is_uninstall_mode() -> bool {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(stem) = exe.file_stem() {
            let name = stem.to_string_lossy().to_lowercase();
            if name.contains("uninstall") {
                return true;
            }
        }
    }
    std::env::args().any(|a| a == "--uninstall")
}

#[tauri::command]
fn get_installed_dir() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.to_string_lossy().to_string();
        }
    }
    default_install_dir()
}

#[tauri::command]
fn default_install_dir() -> String {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| {
        std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string())
    });
    format!("{base}\\Programs\\Glitchy Launcher")
}

#[tauri::command]
fn installed_exe_path(dir: String) -> String {
    format!("{}\\Glitchy Launcher.exe", dir.trim_end_matches('\\'))
}

fn create_shortcuts(exe_path: &str) {
    if let Ok(sl) = mslnk::ShellLink::new(exe_path) {
        if let Some(desktop) = dirs::desktop_dir() {
            let _ = sl.create_lnk(desktop.join("Glitchy Launcher.lnk"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            let start_menu = PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs");
            let _ = std::fs::create_dir_all(&start_menu);
            let _ = sl.create_lnk(start_menu.join("Glitchy Launcher.lnk"));
        }
    }
}

fn remove_shortcuts() {
    if let Some(desktop) = dirs::desktop_dir() {
        let _ = std::fs::remove_file(desktop.join("Glitchy Launcher.lnk"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let start_menu = PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs");
        let _ = std::fs::remove_file(start_menu.join("Glitchy Launcher.lnk"));
    }
}

fn register_uninstaller(install_dir: &str) {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Glitchy Launcher";
    if let Ok((key, _)) = hkcu.create_subkey(path) {
        let exe_path = format!("{install_dir}\\Glitchy Launcher.exe");
        let uninst_path = format!("\"{install_dir}\\uninstall.exe\"");
        let _ = key.set_value("DisplayName", &"Glitchy Launcher");
        let _ = key.set_value("DisplayVersion", &"1.2.1");
        let _ = key.set_value("Publisher", &"Glitchy Team");
        let _ = key.set_value("DisplayIcon", &format!("{exe_path},0"));
        let _ = key.set_value("InstallLocation", &install_dir);
        let _ = key.set_value("UninstallString", &uninst_path);
        let _ = key.set_value("QuietUninstallString", &format!("{uninst_path} --silent"));
        let _ = key.set_value("NoModify", &1u32);
        let _ = key.set_value("NoRepair", &1u32);
    }
}

fn unregister_uninstaller() {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Glitchy Launcher");
}

#[tauri::command]
fn start_install(
    app: AppHandle,
    state: State<InstallState>,
    dir: String,
) -> Result<(), String> {
    let dir = dir.trim_end_matches('\\').to_string();
    if dir.len() < 3 || !dir.contains(':') {
        return Err(format!("Invalid install directory: {dir}"));
    }

    let cancelled = Arc::clone(&state.cancelled);
    *cancelled.lock().unwrap() = false;

    let app = app.clone();
    std::thread::spawn(move || {
        let emit_stage = |stage: &str, pct: u32| {
            let _ = app.emit(
                "setup-progress",
                serde_json::json!({ "percent": pct, "stage": stage }),
            );
        };

        emit_stage("Preparing installation directory…", 15);
        if let Err(_e) = std::fs::create_dir_all(&dir) {
            let _ = app.emit("setup-finished", serde_json::json!({ "code": -1 }));
            return;
        }

        if *cancelled.lock().unwrap() {
            let _ = app.emit("setup-finished", serde_json::json!({ "code": 1223 }));
            return;
        }

        emit_stage("Extracting Glitchy Launcher files…", 40);
        let target_exe = PathBuf::from(&dir).join("Glitchy Launcher.exe");

        let decompress_result = (|| -> Result<(), std::io::Error> {
            let mut decoder = GzDecoder::new(LAUNCHER_GZ);
            let mut dest = std::fs::File::create(&target_exe)?;
            let mut buffer = [0u8; 64 * 1024];
            let mut total_read = 0usize;
            loop {
                let n = decoder.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                dest.write_all(&buffer[..n])?;
                total_read += n;
                let pct = 40 + ((total_read as f64 / 26_000_000.0) * 40.0).min(40.0) as u32;
                emit_stage("Extracting Glitchy Launcher…", pct);
                if *cancelled.lock().unwrap() {
                    return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
                }
            }
            dest.flush()?;
            Ok(())
        })();

        if let Err(_e) = decompress_result {
            if *cancelled.lock().unwrap() {
                let _ = std::fs::remove_file(&target_exe);
                let _ = app.emit("setup-finished", serde_json::json!({ "code": 1223 }));
            } else {
                let _ = app.emit("setup-finished", serde_json::json!({ "code": -1 }));
            }
            return;
        }

        emit_stage("Creating uninstaller…", 82);
        if let Ok(current_exe) = std::env::current_exe() {
            let uninst_dest = PathBuf::from(&dir).join("uninstall.exe");
            let _ = std::fs::copy(&current_exe, &uninst_dest);
        }
        register_uninstaller(&dir);

        emit_stage("Creating shortcuts…", 90);
        create_shortcuts(&target_exe.to_string_lossy());
        std::thread::sleep(Duration::from_millis(200));

        emit_stage("Finalizing installation…", 100);
        std::thread::sleep(Duration::from_millis(200));

        let _ = app.emit("setup-finished", serde_json::json!({ "code": 0 }));
    });

    Ok(())
}

#[tauri::command]
fn start_uninstall(
    app: AppHandle,
    dir: String,
    remove_user_data: bool,
) -> Result<(), String> {
    let app = app.clone();
    std::thread::spawn(move || {
        let emit_stage = |stage: &str, pct: u32| {
            let _ = app.emit(
                "setup-progress",
                serde_json::json!({ "percent": pct, "stage": stage }),
            );
        };

        emit_stage("Closing running instances…", 20);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = Command::new("taskkill")
                .args(["/F", "/IM", "Glitchy Launcher.exe"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
        std::thread::sleep(Duration::from_millis(300));

        emit_stage("Removing shortcuts…", 45);
        remove_shortcuts();

        emit_stage("Cleaning up registry…", 65);
        unregister_uninstaller();

        emit_stage("Removing launcher files…", 85);
        let target_dir = PathBuf::from(&dir);
        if target_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Some(name) = p.file_name() {
                        if name.to_string_lossy().to_lowercase().contains("uninstall") {
                            continue;
                        }
                    }
                    if p.is_dir() {
                        let _ = std::fs::remove_dir_all(&p);
                    } else {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }

        if remove_user_data {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let data_dir = PathBuf::from(appdata).join("ir.glitchy");
                let _ = std::fs::remove_dir_all(&data_dir);
            }
        }

        emit_stage("Uninstallation complete!", 100);
        std::thread::sleep(Duration::from_millis(300));

        let _ = app.emit("setup-finished", serde_json::json!({ "code": 0, "is_uninstall": true }));
    });

    Ok(())
}

#[tauri::command]
fn finish_uninstall(app: AppHandle, dir: String) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let script = format!("ping 127.0.0.1 -n 3 > nul & rmdir /s /q \"{}\"", dir);
        let _ = Command::new("cmd")
            .args(["/C", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    app.exit(0);
}

#[tauri::command]
fn cancel_install(app: AppHandle, state: State<InstallState>) -> Result<(), String> {
    *state.cancelled.lock().unwrap() = true;
    let _ = app.emit(
        "setup-finished",
        serde_json::json!({ "code": 1223 }),
    );
    Ok(())
}

#[tauri::command]
fn launch_installed(dir: String) -> Result<(), String> {
    let exe = installed_exe_path(dir);
    Command::new(&exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to launch {exe}: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(InstallState {
            cancelled: Arc::new(Mutex::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            is_uninstall_mode,
            get_installed_dir,
            default_install_dir,
            installed_exe_path,
            start_install,
            cancel_install,
            launch_installed,
            start_uninstall,
            finish_uninstall
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Glitchy web setup");
}

