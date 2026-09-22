use serde::{Serialize, Serializer};
use thiserror::Error;

/// Convenience alias used across command signatures.
pub type Void = Result<(), AppError>;

/// A structured, frontend-friendly error model.
///
/// Every variant carries:
///   - a stable `code` (string token the frontend can switch on)
///   - a short human-readable message
///   - optional technical detail (never contains secrets)
///   - an optional recovery hint the UI can surface as an action button
///
/// Variants are intentionally flat and domain-oriented so the frontend does
/// not need to parse arbitrary Rust panic text.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Manifest missing")]
    ManifestNotFound,

    #[error("Manifest parse failed: {0}")]
    ManifestParseFailed(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("File creation failed: {0}")]
    FileCreateFailed(String),

    #[error("File rename failed: {0}")]
    FileRenameFailed(String),

    #[error("File read failed: {0}")]
    FileReadFailed(String),

    #[error("File copy failed: {0}")]
    FileCopyFailed(String),

    #[error("File write failed: {0}")]
    FileWriteFailed(String),

    #[error("File delete failed: {0}")]
    FileDeleteFailed(String),

    #[error("Version not found: {0}")]
    VersionNotFound(String),

    #[error("Launch arguments missing")]
    LaunchArgsNotFound,

    #[error("Buffer read failed: {0}")]
    BufferReadFailed(String),

    #[error("Invalid JSON: {0}")]
    JsonParseFailed(String),

    #[error("Invalid INI: {0}")]
    IniParseFailed(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Log history not found")]
    LogHistoryNotFound,

    #[error("Network request failed: {0}")]
    NetworkRequestFailed(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Hash verification failed for {0}")]
    HashMismatch(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("No profile selected")]
    NoProfileSelected,

    #[error("Profile invalid: {0}")]
    ProfileInvalid(String),

    #[error("Zip extraction failed: {0}")]
    ZipExtractionFailed(String),

    #[error("Zip parse failed: {0}")]
    ZipParseFailed(String),

    #[error("Directory create failed: {0}")]
    DirCreateFailed(String),

    #[error("Directory not found: {0}")]
    DirNotFound(String),

    #[error("Mirror connection failed: {0}")]
    MirrorConnectionFailed(String),

    #[error("Mod loading failed: {0}")]
    ModLoadingFailed(String),

    #[error("Java not found: {0}")]
    JavaNotFound(String),

    #[error("Java incompatible: required {required}, found {found}")]
    JavaIncompatible {
        required: String,
        found: String,
    },

    #[error("Java executable invalid: {0}")]
    JavaExecutableInvalid(String),

    #[error("Game file corrupted: {0}")]
    GameFileCorrupted(String),

    #[error("Launch failed: {0}")]
    LaunchFailed(String),

    #[error("Path validation failed: {0}")]
    PathValidationFailed(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Unknown error: {0}")]
    UnknownError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    SerdeIni(#[from] serde_ini::ser::Error),
}

impl AppError {
    /// Stable string token used by the frontend to look up a localized
    /// message and decide which recovery action (if any) to offer.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::ManifestNotFound => "ERROR_MANIFEST_NOT_FOUND",
            AppError::ManifestParseFailed(_) => "ERROR_MANIFEST_PARSE_FAILED",
            AppError::FileNotFound(_) => "ERROR_FILE_NOT_FOUND",
            AppError::FileCreateFailed(_) => "ERROR_FILE_CREATE_FAILED",
            AppError::FileRenameFailed(_) => "ERROR_FILE_RENAME_FAILED",
            AppError::FileReadFailed(_) => "ERROR_FILE_READ_FAILED",
            AppError::FileCopyFailed(_) => "ERROR_FILE_COPY_FAILED",
            AppError::FileWriteFailed(_) => "ERROR_FILE_WRITE_FAILED",
            AppError::FileDeleteFailed(_) => "ERROR_FILE_DELETE_FAILED",
            AppError::VersionNotFound(_) => "ERROR_VERSION_NOT_FOUND",
            AppError::LaunchArgsNotFound => "ERROR_LAUNCH_ARGS_NOT_FOUND",
            AppError::BufferReadFailed(_) => "ERROR_BUFFER_READ_FAILED",
            AppError::JsonParseFailed(_) => "ERROR_JSON_PARSE_FAILED",
            AppError::IniParseFailed(_) => "ERROR_INI_PARSE_FAILED",
            AppError::AccessDenied(_) => "ERROR_ACCESS_DENIED",
            AppError::LogHistoryNotFound => "ERROR_LOG_HISTORY_NOT_FOUND",
            AppError::NetworkRequestFailed(_) => "ERROR_NETWORK_REQUEST_FAILED",
            AppError::DownloadFailed(_) => "ERROR_DOWNLOAD_FAILED",
            AppError::HashMismatch(_) => "ERROR_HASH_MISMATCH",
            AppError::NotImplemented(_) => "ERROR_NOT_IMPLEMENTED",
            AppError::ProfileNotFound(_) => "ERROR_PROFILE_NOT_FOUND",
            AppError::NoProfileSelected => "ERROR_NO_PROFILE_SELECTED",
            AppError::ProfileInvalid(_) => "ERROR_PROFILE_INVALID",
            AppError::ZipExtractionFailed(_) => "ERROR_ZIP_EXTRACTION_FAILED",
            AppError::ZipParseFailed(_) => "ERROR_ZIP_PARSE_FAILED",
            AppError::DirCreateFailed(_) => "ERROR_DIR_CREATE_FAILED",
            AppError::DirNotFound(_) => "ERROR_DIR_NOT_FOUND",
            AppError::MirrorConnectionFailed(_) => "ERROR_MIRROR_CONNECTION_FAILED",
            AppError::ModLoadingFailed(_) => "ERROR_MOD_LOAD_FAILED",
            AppError::JavaNotFound(_) => "ERROR_JAVA_NOT_FOUND",
            AppError::JavaIncompatible { .. } => "ERROR_JAVA_INCOMPATIBLE",
            AppError::JavaExecutableInvalid(_) => "ERROR_JAVA_EXECUTABLE_INVALID",
            AppError::GameFileCorrupted(_) => "ERROR_GAME_FILE_CORRUPTED",
            AppError::LaunchFailed(_) => "ERROR_LAUNCH_FAILED",
            AppError::PathValidationFailed(_) => "ERROR_PATH_VALIDATION_FAILED",
            AppError::Cancelled => "ERROR_CANCELLED",
            AppError::UnknownError(_) => "ERROR_UNKNOWN",
            // Transparent sources — mapped to the closest user-facing code.
            AppError::Io(_) => "ERROR_FILE_READ_FAILED",
            AppError::Anyhow(_) => "ERROR_INTERNAL",
            AppError::Reqwest(_) => "ERROR_NETWORK_REQUEST_FAILED",
            AppError::SerdeJson(_) => "ERROR_JSON_PARSE_FAILED",
            AppError::SerdeIni(_) => "ERROR_INI_PARSE_FAILED",
        }
    }

    /// Optional technical detail (file path, URL, parse error, etc.).
    /// Never contains credentials — the launcher is offline-only.
    pub fn detail(&self) -> Option<String> {
        match self {
            AppError::ManifestNotFound
            | AppError::LaunchArgsNotFound
            | AppError::LogHistoryNotFound
            | AppError::NoProfileSelected
            | AppError::Cancelled => None,
            AppError::ManifestParseFailed(s)
            | AppError::FileNotFound(s)
            | AppError::FileCreateFailed(s)
            | AppError::FileRenameFailed(s)
            | AppError::FileReadFailed(s)
            | AppError::FileCopyFailed(s)
            | AppError::FileWriteFailed(s)
            | AppError::FileDeleteFailed(s)
            | AppError::VersionNotFound(s)
            | AppError::BufferReadFailed(s)
            | AppError::JsonParseFailed(s)
            | AppError::IniParseFailed(s)
            | AppError::AccessDenied(s)
            | AppError::NetworkRequestFailed(s)
            | AppError::DownloadFailed(s)
            | AppError::HashMismatch(s)
            | AppError::NotImplemented(s)
            | AppError::ProfileNotFound(s)
            | AppError::ProfileInvalid(s)
            | AppError::ZipExtractionFailed(s)
            | AppError::ZipParseFailed(s)
            | AppError::DirCreateFailed(s)
            | AppError::DirNotFound(s)
            | AppError::MirrorConnectionFailed(s)
            | AppError::ModLoadingFailed(s)
            | AppError::JavaNotFound(s)
            | AppError::JavaExecutableInvalid(s)
            | AppError::GameFileCorrupted(s)
            | AppError::LaunchFailed(s)
            | AppError::PathValidationFailed(s)
            | AppError::UnknownError(s) => Some(s.clone()),
            AppError::JavaIncompatible { required, found } => {
                Some(format!("required={required}, found={found}"))
            }
            AppError::Io(e) => Some(e.to_string()),
            AppError::Anyhow(e) => Some(e.to_string()),
            AppError::Reqwest(e) => Some(e.to_string()),
            AppError::SerdeJson(e) => Some(e.to_string()),
            AppError::SerdeIni(e) => Some(e.to_string()),
        }
    }

    /// Optional recovery action token. The frontend maps this to a button
    /// (e.g. "Repair Installation", "Install Java"). `None` means the
    /// error is informational and no specific action is suggested.
    pub fn recovery(&self) -> Option<&'static str> {
        match self {
            AppError::JavaNotFound(_) | AppError::JavaExecutableInvalid(_) => {
                Some("ACTION_INSTALL_JAVA")
            }
            AppError::JavaIncompatible { .. } => Some("ACTION_INSTALL_JAVA"),
            AppError::GameFileCorrupted(_) | AppError::HashMismatch(_) => {
                Some("ACTION_REPAIR_INSTALLATION")
            }
            AppError::VersionNotFound(_) => Some("ACTION_OPEN_DOWNLOADS"),
            AppError::NoProfileSelected | AppError::ProfileNotFound(_) | AppError::ProfileInvalid(_) => {
                Some("ACTION_CREATE_PROFILE")
            }
            AppError::MirrorConnectionFailed(_) => Some("ACTION_OPEN_MIRROR_SETTINGS"),
            _ => None,
        }
    }

    /// Short human-readable summary suitable for a toast title. This is
    /// the `Display` impl from `thiserror` — it's the `#[error("...")]`
    /// string. The frontend can use this directly or look up a
    /// localized version via `code()`.
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// Whether the user can reasonably retry the operation that failed.
    /// Network errors, download failures, and mirror issues are
    /// recoverable. Programming errors, missing files, and invalid
    /// config are not.
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            AppError::NetworkRequestFailed(_)
                | AppError::DownloadFailed(_)
                | AppError::MirrorConnectionFailed(_)
                | AppError::Reqwest(_)
                | AppError::Cancelled
        )
    }
}

/// Wire format sent to the frontend:
/// `{ code, message, details, recoverable, recovery }`.
///
/// - `code` — stable token the frontend switches on (e.g. "ERROR_JAVA_NOT_FOUND")
/// - `message` — short human-readable summary
/// - `details` — optional technical detail (file path, URL, parse error)
/// - `recoverable` — whether the user can retry (true for network errors,
///   false for programming errors)
/// - `recovery` — optional action token (e.g. "ACTION_INSTALL_JAVA")
///
/// This shape is the single API contract for all errors. The frontend
/// (`src/messages/errors.json` + `useBackend` hook) maps `code` to a
/// localized message and `recovery` to a button.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("AppError", 5)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.message())?;
        state.serialize_field("details", &self.detail())?;
        state.serialize_field("recoverable", &self.is_recoverable())?;
        state.serialize_field("recovery", &self.recovery())?;
        state.end()
    }
}
