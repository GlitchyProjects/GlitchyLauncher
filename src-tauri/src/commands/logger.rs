use log::info;
use tauri::{command, State};

use crate::models::error::{AppError, Void};
use crate::models::logger::LogLine;
use crate::AppState;

/// Return the full in-memory log history (bounded by the bridge's
/// capacity — currently 10000 lines).
#[command]
pub async fn get_log_history(state: State<'_, AppState>) -> Result<Vec<LogLine>, AppError> {
    let guard = state
        .log_history
        .lock()
        .map_err(|_| AppError::LogHistoryNotFound)?;
    Ok(guard.iter().cloned().collect())
}

/// Clear the entire log history. Always succeeds.
#[command]
pub async fn clear_log_history(state: State<'_, AppState>) -> Void {
    if let Ok(mut guard) = state.log_history.lock() {
        guard.clear();
    }
    Ok(())
}

/// Clear only the log lines tagged with a specific channel (e.g.
/// `launcher`, `minecraft`, `download`). Always succeeds.
#[command]
pub async fn clear_log_history_channel(
    state: State<'_, AppState>,
    channel: String,
) -> Void {
    if let Ok(mut guard) = state.log_history.lock() {
        guard.retain(|line| line.channel != channel);
    }
    Ok(())
}

/// Frontend debugging hook — writes a line to the launcher's log file
/// via the standard `log` crate. Kept for parity with the existing UI.
#[command]
pub async fn debug(text: String) -> Void {
    info!("[frontend] {}", text);
    Ok(())
}
