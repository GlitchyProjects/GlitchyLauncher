//! Tauri command and event payloads for game launching.

use serde::{Deserialize, Serialize};

/// Structured success result returned by `play`.
///
/// The frontend uses `pid` to render a "Game is running" indicator and
/// `started_at` to compute session duration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub pid: u32,
    pub version_id: String,
    pub username: String,
    pub started_at: i64,
}

/// Launch preparation progress emitted via the `launch-progress` event
/// while the `play` command runs, so the main page can render a
/// progress bar from "preparing" up to "game starting".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProgress {
    pub phase: String,
    pub percent: u8,
}

/// Game process exit classification emitted via the `game-exit` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameExitKind {
    NormalExit { code: i32 },
    Crash { code: i32 },
    Terminated,
    LaunchFailure { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExitEvent {
    pub version_id: String,
    pub kind: GameExitKind,
    pub duration_ms: u64,
}
