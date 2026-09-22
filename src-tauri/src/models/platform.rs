use log::warn;

pub enum OperatingSystem {
    Windows,
    Linux,
    MacOS,
}

/// Normalize OS name: lowercases and converts `darwin` -> `osx` so
/// manifest rule-matching works the same on every platform.
pub fn parse_os(os: String) -> String {
    os.to_lowercase().replace("darwin", "osx")
}

/// Return the current OS as a manifest-compatible string:
/// `"windows"`, `"linux"`, or `"osx"`.
///
/// Falls back to `"linux"` when `sys_info` fails (extremely rare; e.g.
/// running inside a minimal container without `/etc/os-release`). The
/// fallback is logged so the user can see why their platform was
/// misdetected.
pub fn get_current_os() -> String {
    match sys_info::os_type() {
        Ok(os) => parse_os(os),
        Err(e) => {
            warn!("sys_info::os_type failed ({e}); defaulting to linux");
            "linux".to_string()
        }
    }
}

/// Return the OS+arch identifier used to index Mojang's
/// `java-runtime/all.json`. Used by the JDK manager to pick the right
/// runtime binary for the current platform.
///
/// Note: arch detection is currently best-effort and defaults to x64
/// on Windows and arm64 on macOS (matching Mojang's runtime naming).
pub fn get_current_os_with_architecture() -> String {
    if cfg!(target_os = "windows") {
        "windows-x64".to_string()
    } else if cfg!(target_os = "macos") {
        // Mojang labels macOS arm64 runtimes as `mac-os-arm64` since
        // 2024; the legacy `mac-os` bucket still ships x64 binaries.
        if cfg!(target_arch = "aarch64") {
            "mac-os-arm64".to_string()
        } else {
            "mac-os".to_string()
        }
    } else {
        // Mojang's runtime manifest keys Linux runtimes as
        // `linux-i386` / `linux-x64` / `linux-arm64` — a bare "linux"
        // key does not exist, so Java downloads would fail on Linux.
        if cfg!(target_arch = "aarch64") {
            "linux-arm64".to_string()
        } else if cfg!(target_arch = "x86") {
            "linux-i386".to_string()
        } else {
            "linux-x64".to_string()
        }
    }
}
