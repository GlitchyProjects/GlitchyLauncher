//! Filesystem path safety helpers.
//!
//! All user-controlled or archive-derived paths must be canonicalized and
//! checked against an intended root before any write or extraction happens.
//! These helpers exist to prevent:
//!   - Zip Slip (`../` sequences inside archives)
//!   - absolute path injection (`/etc/passwd`, `C:\Windows`)
//!   - symlink-based escapes (followed then boundary-checked)
//!   - cross-platform separator confusion

use std::path::{Component, Path, PathBuf};

use crate::models::error::AppError;

/// Returns `true` if `child` is inside `parent` (or equal to it).
///
/// Uses lexical normalization on the components first so it works on
/// not-yet-existing paths (which `canonicalize` would refuse). When the
/// path already exists on disk, prefer [`assert_within_existing`].
pub fn is_within(parent: &Path, child: &Path) -> bool {
    let parent = lexically_absolute(parent);
    let child = lexically_absolute(child);

    child.starts_with(&parent)
}

/// Like [`is_within`] but also resolves symlinks on disk. Use this for
/// existing paths where a symlink could otherwise escape the parent.
pub fn is_within_existing(parent: &Path, child: &Path) -> bool {
    let Ok(parent_c) = parent.canonicalize() else {
        return is_within(parent, child);
    };
    let Ok(child_c) = child.canonicalize() else {
        // Path does not exist yet — fall back to lexical check on parent
        // (which does exist) and the as-yet-uncreated child.
        let parent_lex = lexically_absolute(&parent_c);
        let child_lex = lexically_absolute(child);
        return child_lex.starts_with(parent_lex);
    };
    child_c.starts_with(&parent_c)
}

/// Normalize a path to an absolute form without touching the filesystem.
///
/// Strips `.` components and resolves `..` against earlier components.
/// Rejects paths that try to escape above the original root via `..`.
pub fn lexically_absolute(path: &Path) -> PathBuf {
    let mut out = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))
    };

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Pop only if the last segment is a normal component;
                // never escape above the original root.
                if !out.pop() {
                    // Attempted to walk above root — leave as-is, callers
                    // should reject this path via is_within.
                }
            }
            Component::RootDir => {
                out = PathBuf::from("/");
            }
            Component::Normal(part) => {
                out.push(part);
            }
            Component::Prefix(prefix) => {
                // Windows drive letter — start fresh.
                out = PathBuf::from(prefix.as_os_str());
            }
        }
    }
    out
}

/// Validate that a relative archive entry resolves strictly inside `root`.
///
/// Returns the resolved absolute path on success. Returns
/// [`AppError::PathValidationFailed`] when the entry would escape `root`
/// or contains otherwise suspicious components.
///
/// This is the core defense against Zip Slip. Every archive extraction
/// path MUST go through this function before being written.
pub fn sanitize_archive_entry(root: &Path, entry: &str) -> Result<PathBuf, AppError> {
    // Reject NUL bytes and control characters up front.
    if entry.bytes().any(|b| b == 0 || b < 0x20) {
        return Err(AppError::PathValidationFailed(format!(
            "archive entry contains control characters: {entry:?}"
        )));
    }

    // Backslashes on Windows are valid separators but `Path` does not
    // always treat them as such on Unix. Normalize to forward slashes so
    // `..` detection works identically on every platform.
    let normalized = entry.replace('\\', "/");
    let candidate = root.join(&normalized);
    let resolved = lexically_absolute(&candidate);

    if !resolved.starts_with(lexically_absolute(root)) {
        return Err(AppError::PathValidationFailed(format!(
            "archive entry escapes extraction root: {entry:?}"
        )));
    }

    Ok(resolved)
}

/// Validate a user-supplied path intended to live inside `root` (for
/// example an instance name that becomes a directory name). Rejects
/// absolute paths, parent references, and any traversal attempt.
pub fn sanitize_user_path_segment(root: &Path, segment: &str) -> Result<PathBuf, AppError> {
    if segment.is_empty() {
        return Err(AppError::PathValidationFailed("empty path segment".to_string()));
    }
    if segment.contains('\0') {
        return Err(AppError::PathValidationFailed(
            "path segment contains NUL byte".to_string(),
        ));
    }
    let path = Path::new(segment);
    if path.is_absolute() {
        return Err(AppError::PathValidationFailed(format!(
            "absolute paths are not allowed: {segment:?}"
        )));
    }
    if segment.contains("..") {
        return Err(AppError::PathValidationFailed(format!(
            "parent traversal is not allowed: {segment:?}"
        )));
    }
    // Also reject platform-illegal characters in a name segment.
    let illegal = |c: char| {
        matches!(c, '/' | '\\') || c == '\0' || c == ':' || c == '*' || c == '?' || c == '"' || c == '<' || c == '>' || c == '|'
    };
    if segment.chars().any(illegal) {
        return Err(AppError::PathValidationFailed(format!(
            "path segment contains illegal characters: {segment:?}"
        )));
    }
    let resolved = root.join(segment);
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_classic_zip_slip() {
        let root = Path::new("/tmp/extract");
        let bad = "../../../etc/passwd";
        assert!(sanitize_archive_entry(root, bad).is_err());
    }

    #[test]
    fn rejects_absolute_archive_entry() {
        let root = Path::new("/tmp/extract");
        assert!(sanitize_archive_entry(root, "/etc/passwd").is_err());
    }

    #[test]
    fn accepts_normal_relative_entry() {
        let root = Path::new("/tmp/extract");
        let ok = sanitize_archive_entry(root, "assets/index.json").unwrap();
        assert!(ok.starts_with("/tmp/extract"));
    }

    #[test]
    fn normalizes_backslashes() {
        let root = Path::new("/tmp/extract");
        assert!(sanitize_archive_entry(root, "..\\..\\evil").is_err());
    }

    #[test]
    fn rejects_user_segment_with_traversal() {
        let root = Path::new("/tmp/instances");
        assert!(sanitize_user_path_segment(root, "../escape").is_err());
        assert!(sanitize_user_path_segment(root, "/etc/passwd").is_err());
        assert!(sanitize_user_path_segment(root, "name:with:colons").is_err());
        assert!(sanitize_user_path_segment(root, "Survival").is_ok());
    }

    #[test]
    fn rejects_control_chars() {
        let root = Path::new("/tmp/extract");
        assert!(sanitize_archive_entry(root, "evil\u{0000}name").is_err());
        assert!(sanitize_archive_entry(root, "evil\nname").is_err());
    }
}
