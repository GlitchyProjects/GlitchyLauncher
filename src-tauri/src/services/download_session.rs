//! Background download sessions.
//!
//! A download started from the UI no longer runs inside the Tauri
//! command (which died visually the moment the user navigated away).
//! Instead the command registers a [`SessionHandle`] here, spawns the
//! install on a tokio task and returns the session id immediately.
//!
//! The handle is the single source of truth for phase / percent /
//! current-file / state. Every mutation emits a `download-session`
//! event (plus the legacy `progress` / `progressBar` events so older
//! UI code keeps working), and the registry keeps the last
//! [`MAX_SESSIONS`] sessions so the frontend can re-sync with
//! `get_active_downloads` at any time.

use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::services::download_manager::DownloadToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub label: String,
    pub version_id: String,
    pub created_at: u64,
    pub phase: String,
    pub percent: u8,
    pub current_file: String,
    pub state: SessionState,
    pub error: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_second: u64,
}

/// Fraction of a pass assigned to each phase. Assets deliberately get
/// the whole second half of the bar so the readout matches user
/// expectations (java + libraries + client ≈ first half, assets 50–100%).
fn phase_weight(phase: &str) -> (f64, f64) {
    match phase {
        "java" => (0.02, 0.20),
        "libraries" => (0.20, 0.45),
        "client" => (0.45, 0.50),
        "assets" => (0.50, 1.00),
        "modpack" => (0.00, 1.00),
        _ => (0.00, 0.02), // prepare / loader / done
    }
}

/// Human-readable text for a phase id (used for the legacy `progress`
/// event and logs).
pub fn phase_label(phase: &str) -> &'static str {
    match phase {
        "prepare" => "Preparing download...",
        "loader" => "Running loader installer...",
        "java" => "Downloading Java runtime...",
        "libraries" => "Downloading libraries...",
        "client" => "Downloading Minecraft client...",
        "assets" => "Downloading assets...",
        "modpack" => "Installing modpack files...",
        "done" => "Finishing up...",
        _ => "Working...",
    }
}

struct SessionInner {
    info: SessionInfo,
    /// Sub-range of the overall bar this pass may use. A vanilla
    /// install is a single pass over [0,1]; loader installs (Forge /
    /// Fabric) download an inherited version first over [0,0.5].
    pass: (f64, f64),
    /// Absolute [start,end] fraction of the current phase inside the pass.
    range: (f64, f64),
    // Emit throttling state.
    last_emit_percent: u8,
    last_emit_phase: String,
    last_emit_state: SessionState,
    last_emit_at: Option<Instant>,
    last_emit_downloaded_bytes: u64,
    last_transfer_at: Option<Instant>,
    last_transfer_bytes: u64,
}

#[derive(Clone)]
pub struct SessionHandle {
    id: String,
    app: AppHandle,
    token: Arc<DownloadToken>,
    inner: Arc<Mutex<SessionInner>>,
}

static REGISTRY: LazyLock<Mutex<Vec<SessionHandle>>> = LazyLock::new(|| Mutex::new(Vec::new()));
const MAX_SESSIONS: usize = 20;
/// Minimum time between throttled progress emissions.
const EMIT_INTERVAL: Duration = Duration::from_millis(150);

pub fn create_session(app: &AppHandle, label: &str, version_id: &str) -> SessionHandle {
    let label = if label.trim().is_empty() { version_id } else { label };
    let id = Uuid::new_v4().to_string();
    let (w0, w1) = phase_weight("prepare");
    let handle = SessionHandle {
        id: id.clone(),
        app: app.clone(),
        token: Arc::new(DownloadToken::new()),
        inner: Arc::new(Mutex::new(SessionInner {
            info: SessionInfo {
                id,
                label: label.to_string(),
                version_id: version_id.to_string(),
                created_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                phase: "prepare".to_string(),
                percent: 0,
                current_file: String::new(),
                state: SessionState::Running,
                error: None,
                downloaded_bytes: 0,
                total_bytes: 0,
                bytes_per_second: 0,
            },
            pass: (0.0, 1.0),
            range: (w0, w1),
            last_emit_percent: 0,
            last_emit_phase: String::new(),
            last_emit_state: SessionState::Running,
            last_emit_at: None,
            last_emit_downloaded_bytes: 0,
            last_transfer_at: None,
            last_transfer_bytes: 0,
        })),
    };
    // Keep the registry bounded: drop the oldest terminal sessions.
    {
        let mut reg = REGISTRY.lock().unwrap();
        if reg.len() >= MAX_SESSIONS {
            let mut excess = reg.len() + 1 - MAX_SESSIONS;
            reg.retain(|s| {
                let terminal = s.state_is_terminal();
                if terminal && excess > 0 {
                    excess -= 1;
                    false
                } else {
                    true
                }
            });
        }
        reg.push(handle.clone());
    }
    handle.emit(true);
    handle
}

impl SessionHandle {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn token(&self) -> Arc<DownloadToken> {
        self.token.clone()
    }

    pub fn set_version_id(&self, version_id: &str) {
        self.inner.lock().unwrap().info.version_id = version_id.to_string();
        self.emit(true);
    }

    pub fn info(&self) -> SessionInfo {
        self.inner.lock().unwrap().info.clone()
    }

    fn state_is_terminal(&self) -> bool {
        matches!(
            self.inner.lock().unwrap().info.state,
            SessionState::Completed | SessionState::Failed | SessionState::Cancelled
        )
    }

    /// Narrow the overall bar to `[a,b]` (0.0–1.0) for the upcoming
    /// pass. Must be called before `set_phase`.
    pub fn set_pass_range(&self, a: f64, b: f64) {
        let mut inner = self.inner.lock().unwrap();
        inner.pass = (a.min(b), b.max(a));
    }

    /// Enter a phase; the bar jumps to the phase's start fraction.
    pub fn set_phase(&self, phase: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            let (a, b) = inner.pass;
            let (w0, w1) = phase_weight(phase);
            inner.range = (a + w0 * (b - a), a + w1 * (b - a));
            inner.info.phase = phase.to_string();
            inner.info.percent = (inner.range.0 * 100.0).round().clamp(0.0, 100.0) as u8;
            inner.info.current_file.clear();
            inner.info.downloaded_bytes = 0;
            inner.info.total_bytes = 0;
            inner.info.bytes_per_second = 0;
            inner.last_transfer_at = None;
            inner.last_transfer_bytes = 0;
        }
        self.emit(true);
    }

    /// Update live byte counters for the current transfer batch. Speed is
    /// calculated from consecutive samples and smoothed to avoid a jumpy UI.
    pub fn transfer(&self, downloaded_bytes: u64, total_bytes: u64, current_file: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            let now = Instant::now();
            let batch_restarted = downloaded_bytes < inner.last_transfer_bytes
                || total_bytes != inner.info.total_bytes;
            if batch_restarted {
                inner.last_transfer_at = Some(now);
                inner.last_transfer_bytes = downloaded_bytes;
                inner.info.bytes_per_second = 0;
            } else if let Some(previous_at) = inner.last_transfer_at {
                let elapsed = now.duration_since(previous_at).as_secs_f64();
                if elapsed >= 0.1 {
                    let delta = downloaded_bytes.saturating_sub(inner.last_transfer_bytes);
                    let instant_speed = (delta as f64 / elapsed) as u64;
                    inner.info.bytes_per_second = if inner.info.bytes_per_second == 0 {
                        instant_speed
                    } else {
                        (inner.info.bytes_per_second * 2 + instant_speed) / 3
                    };
                    inner.last_transfer_at = Some(now);
                    inner.last_transfer_bytes = downloaded_bytes;
                }
            } else {
                inner.last_transfer_at = Some(now);
                inner.last_transfer_bytes = downloaded_bytes;
            }
            inner.info.downloaded_bytes = downloaded_bytes;
            inner.info.total_bytes = total_bytes;
            if !current_file.is_empty() {
                inner.info.current_file = current_file.to_string();
            }
        }
        self.emit(false);
    }

    /// Report progress (0.0–1.0) within the current phase plus the
    /// file currently being transferred.
    pub fn progress(&self, fraction: f64, current_file: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            let (r0, r1) = inner.range;
            let abs = r0 + fraction.clamp(0.0, 1.0) * (r1 - r0);
            inner.info.percent = (abs * 100.0).round().clamp(0.0, 100.0) as u8;
            inner.info.current_file = current_file.to_string();
        }
        self.emit(false);
    }

    /// Update just the current-file label (used for single large files).
    pub fn set_current_file(&self, file: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.current_file = file.to_string();
        }
        self.emit(false);
    }

    pub fn complete(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Completed;
            inner.info.phase = "done".to_string();
            inner.info.percent = 100;
            inner.info.current_file.clear();
            inner.info.error = None;
            inner.info.bytes_per_second = 0;
        }
        self.emit(true);
    }

    pub fn fail(&self, error: &str) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Failed;
            inner.info.error = Some(error.to_string());
            inner.info.bytes_per_second = 0;
        }
        self.emit(true);
    }

    pub fn mark_cancelled(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Cancelled;
            inner.info.bytes_per_second = 0;
        }
        self.emit(true);
    }

    pub fn pause(&self) {
        self.token.pause();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Paused;
            inner.info.bytes_per_second = 0;
        }
        self.emit(true);
    }

    pub fn resume(&self) {
        self.token.resume();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Running;
        }
        self.emit(true);
    }

    pub fn cancel(&self) {
        self.token.cancel();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.info.state = SessionState::Cancelled;
        }
        self.emit(true);
    }

    /// Broadcast the session state. Throttled unless `force` (phase /
    /// state change or completion) — asset batches settle thousands of
    /// files and we do not want to flood the IPC channel.
    fn emit(&self, force: bool) {
        let (info, should_emit) = {
            let mut inner = self.inner.lock().unwrap();
            let now = Instant::now();
            let changed =
                inner.info.percent != inner.last_emit_percent
                    || inner.info.phase != inner.last_emit_phase
                    || inner.info.state != inner.last_emit_state
                    || inner.info.downloaded_bytes != inner.last_emit_downloaded_bytes;
            let due = inner
                .last_emit_at
                .map(|t| now.duration_since(t) >= EMIT_INTERVAL)
                .unwrap_or(true);
            if !force && !(changed && due) {
                (inner.info.clone(), false)
            } else {
                inner.last_emit_percent = inner.info.percent;
                inner.last_emit_phase = inner.info.phase.clone();
                inner.last_emit_state = inner.info.state;
                inner.last_emit_downloaded_bytes = inner.info.downloaded_bytes;
                inner.last_emit_at = Some(now);
                (inner.info.clone(), true)
            }
        };
        if !should_emit {
            return;
        }
        let _ = self.app.emit("download-session", &info);
        let _ = self.app.emit("progressBar", info.percent as i64);
        let text = if info.current_file.is_empty() {
            phase_label(&info.phase).to_string()
        } else {
            format!("{} ({})", phase_label(&info.phase), info.current_file)
        };
        let _ = self.app.emit("progress", text);
    }
}

/// All sessions, newest first.
pub fn list_sessions() -> Vec<SessionInfo> {
    let reg = REGISTRY.lock().unwrap();
    let mut infos: Vec<SessionInfo> = reg.iter().map(|s| s.info()).collect();
    infos.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    infos
}

/// Look up a session and run `f` on it. Returns `None` when the id is
/// unknown.
pub fn with_session<F: FnOnce(&SessionHandle)>(id: &str, f: F) -> Option<()> {
    let reg = REGISTRY.lock().unwrap();
    let handle = reg.iter().find(|s| s.id == id)?;
    f(handle);
    Some(())
}

/// Remove terminal sessions from the registry (frontend "clear" button).
pub fn clear_finished() {
    REGISTRY.lock().unwrap().retain(|s| !s.state_is_terminal());
}
