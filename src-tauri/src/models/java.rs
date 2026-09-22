use std::path::PathBuf;

use log::warn;
use serde::{Deserialize, Serialize};

use crate::models::platform;
use crate::services::directory_manager::get_launcher_java_directory;

/// Represents an installed Java runtime used to launch Minecraft.
///
/// `version` is the raw `JAVA_VERSION` string from the runtime's
/// `release` file (e.g. `"17.0.1"` or `"1.8.0_291"`). Use
/// [`version_major_from_release`] for a normalized major version.
#[derive(Debug, Serialize, Deserialize)]
pub struct Java {
    pub path: PathBuf,
    pub version: String,
}

impl Java {
    /// Construct a `Java` handle from a JRE root directory.
    ///
    /// Reads the `release` file to populate `version`. If the file is
    /// missing or malformed, `version` defaults to `"unknown"` — the
    /// launcher then falls back to `java -version` detection.
    pub fn new(path: PathBuf) -> Java {
        let release = path.join("release");
        let version = std::fs::read_to_string(&release)
            .ok()
            .and_then(|content| {
                content
                    .lines()
                    .find(|line| line.starts_with("JAVA_VERSION="))
                    .and_then(|line| line.strip_prefix("JAVA_VERSION="))
                    .map(|s| s.trim_matches('"').to_string())
            })
            .unwrap_or_else(|| {
                warn!(
                    "release file missing or unreadable at {}, version unknown",
                    path.display()
                );
                "unknown".to_string()
            });
        Java { path, version }
    }

    /// Path to the Java GUI executable (`javaw.exe` on Windows, `java` elsewhere).
    pub fn get_bin_file(&self) -> PathBuf {
        let os = platform::get_current_os();
        let bin = if os == "windows" { "javaw.exe" } else { "java" };
        self.path.join("bin").join(bin)
    }

    /// Path to the Java CLI executable (`java.exe` on Windows, `java` elsewhere)
    /// suitable for installers and command-line patchers that write to stdout/stderr.
    pub fn get_cli_bin_file(&self) -> PathBuf {
        let os = platform::get_current_os();
        let bin = if os == "windows" { "java.exe" } else { "java" };
        self.path.join("bin").join(bin)
    }

    /// Best-effort parse of the major Java version from the `release`
    /// file's `JAVA_VERSION` field. Returns `None` on parse failure or
    /// when the runtime is unknown.
    pub fn version_major_from_release(&self) -> Option<u32> {
        if self.version == "unknown" || self.version.is_empty() {
            return None;
        }
        let major = self.version.split('.').next()?;
        let major: u32 = major.parse().ok()?;
        // Java 8 and earlier report as `1.8.x`; modern Java reports
        // the major version directly (`17.0.1`, `21`, etc.).
        Some(if major == 1 { 8 } else { major })
    }
}

/// Locate the launcher-managed Java directory. Mojang-style runtimes
/// live under `<mc>/runtime/<component>/`.
pub fn java_runtime_root(component: &str) -> PathBuf {
    get_launcher_java_directory().join(component)
}
