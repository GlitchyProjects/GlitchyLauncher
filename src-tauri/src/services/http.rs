//! Shared HTTP client for all direct (non-file-download) requests.
//!
//! Every `reqwest::get` in the codebase used the global default client,
//! which has no timeout and no retry — a single transient network blip
//! aborted the whole install with a bare "error sending request for url".
//! All ad-hoc requests now go through [`HTTP_CLIENT`] (timeouts, keepalive,
//! User-Agent) and [`get`]/[`get_json`] which retry transient failures
//! with exponential backoff.

use std::sync::LazyLock;
use std::time::Duration;

use log::warn;
use reqwest::Client;
use serde::de::DeserializeOwned;

use crate::models::error::AppError;

pub static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_nodelay(true)
        .user_agent(format!("GlitchyLauncher/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("failed to build shared HTTP client")
});

const MAX_ATTEMPTS: u32 = 3;

fn is_transient(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}

async fn backoff(attempt: u32) {
    let exp = 400u64.saturating_mul(1u64 << (attempt.min(4) - 1));
    tokio::time::sleep(Duration::from_millis(exp.min(4_000))).await;
}

/// GET `url` with the shared client, retrying transient transport
/// failures up to [`MAX_ATTEMPTS`] times. Callers still need to check
/// the response status themselves.
pub async fn get(url: &str) -> Result<reqwest::Response, AppError> {
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        match HTTP_CLIENT.get(url).send().await {
            Ok(resp) => return Ok(resp),
            Err(e) if attempt < MAX_ATTEMPTS && is_transient(&e) => {
                warn!("GET {url} failed (attempt {attempt}): {e} — retrying");
                backoff(attempt).await;
            }
            Err(e) => {
                return Err(AppError::NetworkRequestFailed(format!("{url}: {e}")));
            }
        }
    }
}

/// GET `url` and deserialize the JSON body as `T`, with retries on
/// transient failures. Non-2xx statuses are errors.
pub async fn get_json<T: DeserializeOwned>(url: &str) -> Result<T, AppError> {
    let resp = get(url).await?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AppError::NetworkRequestFailed(format!(
            "HTTP {status} for {url}"
        )));
    }
    resp.json::<T>()
        .await
        .map_err(|e| AppError::NetworkRequestFailed(format!("{url}: {e}")))
}
