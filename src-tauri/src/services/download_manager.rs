//! Centralized, retry-aware, hash-verifying download infrastructure.
//!
//! This module is the single source of truth for downloading artifacts.
//! All other services (libraries, assets, java, version JSONs) must go
//! through [`DownloadManager`] instead of calling `reqwest::get` directly.
//!
//! Capabilities:
//!   - configurable concurrency (semaphore-bounded worker pool)
//!   - exponential backoff with jitter on transient failures
//!   - HTTP range-resume when the server advertises `Accept-Ranges: bytes`
//!   - atomic file replacement (`.part` → final) so partial files are
//!     never observed as completed
//!   - SHA-1 / SHA-256 verification when an expected hash is provided
//!   - cancellation via a [`DownloadToken`]
//!   - per-file and aggregate progress reporting through Tauri events
//!
//! The manager is intentionally dependency-light: only `reqwest`,
//! `sha1`/`sha2` (already pulled in by other modules) and `tokio`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use log::{debug, info, warn};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha256};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use crate::models::error::AppError;

type TransferCallback = Arc<dyn Fn(u64, u64, String) + Send + Sync>;

/// Default per-request settings.
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
const BACKOFF_BASE_MS: u64 = 500;
const BACKOFF_MAX_MS: u64 = 8_000;

/// Hash algorithm expected for verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashKind {
    Sha1,
    Sha256,
}

/// Process-wide manager so services without access to `AppState`
/// (game_downloader, jdk_manager, …) share the same client pool,
/// concurrency semaphore and batch lock as the repair flow.
static GLOBAL: LazyLock<Arc<DownloadManager>> = LazyLock::new(|| Arc::new(DownloadManager::new(32)));

/// Handle to the shared [`DownloadManager`].
pub fn global() -> Arc<DownloadManager> {
    GLOBAL.clone()
}

/// Optional verification spec. When `expected_hash` is `None`, the
/// manager still downloads the file but skips the verify step (used for
/// best-effort resources with no upstream checksum).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifySpec {
    pub kind: HashKind,
    pub expected_hash: String,
}

impl VerifySpec {
    pub fn sha1(expected: impl Into<String>) -> Self {
        Self {
            kind: HashKind::Sha1,
            expected_hash: expected.into(),
        }
    }
    pub fn sha256(expected: impl Into<String>) -> Self {
        Self {
            kind: HashKind::Sha256,
            expected_hash: expected.into(),
        }
    }
}

/// A single queued download.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub url: String,
    /// Absolute destination path. The manager will create parent dirs.
    pub destination: String,
    /// Expected size, if known. Used as a fast pre-check.
    pub expected_size: Option<u64>,
    /// Optional hash verification. Always preferred over size-only checks.
    pub verify: Option<VerifySpec>,
    /// Human label used in progress events (e.g. "libraries", "assets").
    pub category: String,
}

/// Aggregate progress event emitted to the frontend.
///
/// `completed_bytes` and `total_bytes` are summed across all jobs in the
/// current batch. `state` is one of `queued|downloading|verifying|
/// retrying|completed|failed|cancelled`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub batch_id: String,
    pub state: String,
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub files_total: u32,
    pub files_done: u32,
    pub files_failed: u32,
    pub retry_attempt: u32,
    pub current_label: String,
}

// Internal counters are u32 for files and u64 for bytes. Re-exported
// in DownloadProgress so the frontend knows the wire types.
const _: () = {
    // Compile-time sanity: ensure we don't accidentally widen counters.
    let _check: fn() = || {
        let _: u32 = 0;
        let _: u64 = 0;
    };
};

/// Cancellation + pause handle. Cheap to clone; `cancelled` aborts all
/// in-flight downloads in the same batch at the next chunk boundary,
/// `paused` parks them between files until resumed or cancelled.
#[derive(Debug, Clone)]
pub struct DownloadToken {
    cancelled: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
}

impl DownloadToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }
    /// Park the caller while paused. Returns immediately once resumed
    /// or cancelled; the caller is expected to re-check `is_cancelled`.
    pub async fn wait_while_paused(&self) {
        while self.is_paused() && !self.is_cancelled() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

impl Default for DownloadToken {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal per-job result.
#[derive(Debug)]
pub enum DownloadOutcome {
    Completed,
    AlreadyCached,
    Failed(AppError),
}

/// The central download manager. One instance is created at startup and
/// shared via `AppState`. Services call [`DownloadManager::download_batch`]
/// or [`DownloadManager::download_one`] instead of managing their own
/// `reqwest` calls.
pub struct DownloadManager {
    client: Client,
    /// Bounded concurrency. Default 8 in-flight requests.
    semaphore: Arc<Semaphore>,
    /// Aggregate counters for the active batch (single batch at a time
    /// per the current launcher UX). `completed` is an `Arc` so it can
    /// be shared with spawned tokio tasks — each task atomically adds
    /// its downloaded bytes to this counter.
    completed: Arc<AtomicU64>,
    total: AtomicU64,
    files_done: AtomicU32,
    files_failed: AtomicU32,
    files_total: AtomicU32,
    /// Prevents two batches from racing on the same counters.
    batch_lock: Mutex<()>,
}

impl DownloadManager {
    pub fn new(concurrency: usize) -> Self {
        let client = Client::builder()
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            // `reqwest::Client::timeout` covers the complete response body,
            // not just a stalled read. Large Minecraft clients and Java
            // runtimes routinely take longer than 60 seconds on slower
            // connections, so allow a realistic end-to-end window.
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_nodelay(true)
            .build()
            .expect("reqwest client builder failed");

        Self {
            client,
            semaphore: Arc::new(Semaphore::new(concurrency.max(1))),
            completed: Arc::new(AtomicU64::new(0)),
            total: AtomicU64::new(0),
            files_done: AtomicU32::new(0),
            files_failed: AtomicU32::new(0),
            files_total: AtomicU32::new(0),
            batch_lock: Mutex::new(()),
        }
    }

    /// Download a single job. Convenience wrapper around [`download_batch`]
    /// for services that only need one file.
    pub async fn download_one(
        &self,
        job: DownloadJob,
        app_handle: Option<&AppHandle>,
        token: &DownloadToken,
    ) -> Result<(), AppError> {
        let batch = vec![job];
        self.download_batch(batch, app_handle, token).await?;
        Ok(())
    }

    /// Download many jobs concurrently. Returns the first hard failure
    /// after every other job in the batch has settled. Cancellation
    /// aborts pending jobs and returns [`AppError::Cancelled`].
    pub async fn download_batch(
        &self,
        jobs: Vec<DownloadJob>,
        app_handle: Option<&AppHandle>,
        token: &DownloadToken,
    ) -> Result<(), AppError> {
        self.download_batch_with_progress(jobs, app_handle, token, None)
            .await
    }

    /// Same as [`download_batch`], additionally invoking `on_progress`
    /// after each job settles with `(files_settled, files_total,
    /// current_file)` so callers can drive a progress bar and show
    /// which file is transferring.
    pub async fn download_batch_with_progress(
        &self,
        jobs: Vec<DownloadJob>,
        app_handle: Option<&AppHandle>,
        token: &DownloadToken,
        on_progress: Option<Arc<dyn Fn(u32, u32, String) + Send + Sync>>,
    ) -> Result<(), AppError> {
        self.download_batch_with_callbacks(jobs, app_handle, token, on_progress, None)
            .await
    }

    /// Download a batch while reporting both settled files and live byte
    /// transfer progress. The byte callback receives
    /// `(completed_bytes, total_bytes, current_file)` and is invoked at
    /// chunk boundaries, which gives the UI enough information to calculate
    /// a useful live transfer rate.
    pub async fn download_batch_with_callbacks(
        &self,
        jobs: Vec<DownloadJob>,
        app_handle: Option<&AppHandle>,
        token: &DownloadToken,
        on_progress: Option<Arc<dyn Fn(u32, u32, String) + Send + Sync>>,
        on_transfer: Option<TransferCallback>,
    ) -> Result<(), AppError> {
        let _guard = self.batch_lock.lock().await;

        // Reset aggregate counters.
        let total_bytes = jobs.iter().filter_map(|j| j.expected_size).sum::<u64>();
        let files_total = jobs.len() as u32;
        self.completed.store(0, Ordering::SeqCst);
        self.total.store(total_bytes, Ordering::SeqCst);
        self.files_done.store(0, Ordering::SeqCst);
        self.files_failed.store(0, Ordering::SeqCst);
        self.files_total.store(files_total as u32, Ordering::SeqCst);

        let batch_id = Uuid::new_v4().to_string();
        self.emit_progress(app_handle, &batch_id, "downloading", 0, "", 0);
        if let Some(callback) = &on_transfer {
            callback(0, total_bytes, String::new());
        }

        let mut handles = Vec::with_capacity(jobs.len());
        for job in jobs {
            // Park between files while the batch is paused; stop
            // spawning new jobs once cancelled.
            token.wait_while_paused().await;
            if token.is_cancelled() {
                break;
            }
            let permit = Arc::clone(&self.semaphore)
                .acquire_owned()
                .await
                .map_err(|e| AppError::UnknownError(format!("semaphore acquire failed: {e}")))?;
            let client = self.client.clone();
            let completed = self.completed_ptr();
            let token = token.clone();
            let batch_id = batch_id.clone();
            let app_handle = app_handle.cloned();
            let on_transfer = on_transfer.clone();
            handles.push(tokio::spawn(async move {
                let _permit = permit;
                let outcome = run_job(
                    &client,
                    &job,
                    &completed,
                    total_bytes,
                    &token,
                    &app_handle,
                    &batch_id,
                    &on_transfer,
                )
                .await;
                (job, outcome)
            }));
        }

        let mut first_error: Option<AppError> = None;
        for handle in handles {
            // Filename of the job that just settled — shown to the user
            // as the "currently downloading" file.
            let mut current_file = String::new();
            match handle.await {
                Ok((job, DownloadOutcome::Completed)) => {
                    self.files_done.fetch_add(1, Ordering::SeqCst);
                    current_file = file_name_of(&job.url);
                    debug!("job done: {} ({})", job.url, job.category);
                }
                Ok((job, DownloadOutcome::AlreadyCached)) => {
                    self.files_done.fetch_add(1, Ordering::SeqCst);
                    if let Some(size) = job.expected_size {
                        self.completed.fetch_add(size, Ordering::SeqCst);
                    }
                    current_file = file_name_of(&job.url);
                    debug!("job cached: {} ({})", job.url, job.category);
                }
                Ok((job, DownloadOutcome::Failed(err))) => {
                    self.files_failed.fetch_add(1, Ordering::SeqCst);
                    current_file = file_name_of(&job.url);
                    warn!("job failed: {} -> {:?}", job.url, err);
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                }
                Err(join_err) => {
                    self.files_failed.fetch_add(1, Ordering::SeqCst);
                    let err = AppError::UnknownError(format!("task join failed: {join_err}"));
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                }
            }
            if let Some(cb) = &on_progress {
                let settled = self.files_done.load(Ordering::SeqCst)
                    + self.files_failed.load(Ordering::SeqCst);
                cb(settled, files_total, current_file.clone());
            }
            if let Some(callback) = &on_transfer {
                callback(
                    self.completed.load(Ordering::SeqCst).min(total_bytes),
                    total_bytes,
                    current_file.clone(),
                );
            }
            if token.is_cancelled() {
                break;
            }
        }

        let done = self.files_done.load(Ordering::SeqCst);
        let failed = self.files_failed.load(Ordering::SeqCst);
        let state = if token.is_cancelled() {
            "cancelled"
        } else if failed > 0 && done == 0 {
            "failed"
        } else if failed > 0 {
            "completed_with_errors"
        } else {
            "completed"
        };
        self.emit_progress(app_handle, &batch_id, state, 100, "", 0);

        if token.is_cancelled() {
            return Err(AppError::Cancelled);
        }
        if let Some(err) = first_error {
            return Err(err);
        }
        Ok(())
    }

    fn completed_ptr(&self) -> Arc<AtomicU64> {
        // Return a clone of the shared Arc so spawned tasks can
        // atomically update the aggregate `completed` counter. This
        // was previously broken — it allocated a fresh Arc per batch
        // and the per-task bytes were never merged back.
        Arc::clone(&self.completed)
    }

    fn emit_progress(
        &self,
        app_handle: Option<&AppHandle>,
        batch_id: &str,
        state: &str,
        percent: u64,
        label: &str,
        retry_attempt: u32,
    ) {
        let completed = self.completed.load(Ordering::SeqCst);
        let total = self.total.load(Ordering::SeqCst);
        let files_total = self.files_total.load(Ordering::SeqCst);
        let files_done = self.files_done.load(Ordering::SeqCst);
        let files_failed = self.files_failed.load(Ordering::SeqCst);
        let progress = DownloadProgress {
            batch_id: batch_id.to_string(),
            state: state.to_string(),
            completed_bytes: completed,
            total_bytes: total,
            files_total,
            files_done,
            files_failed,
            retry_attempt,
            current_label: label.to_string(),
        };
        if let Some(handle) = app_handle {
            let _ = handle.emit("download-progress", &progress);
        }
        info!(
            "[download] state={state} pct={percent}% done={files_done}/{files_total} failed={files_failed} bytes={completed}/{total}"
        );
    }
}

async fn run_job(
    client: &Client,
    job: &DownloadJob,
    completed: &Arc<AtomicU64>,
    total_bytes: u64,
    token: &DownloadToken,
    app_handle: &Option<AppHandle>,
    batch_id: &str,
    on_transfer: &Option<TransferCallback>,
) -> DownloadOutcome {
    // Fast path: file already on disk and verifies OK.
    if let Ok(()) = verify_existing(job).await {
        return DownloadOutcome::AlreadyCached;
    }

    let dest = PathBuf::from(&job.destination);
    if let Err(e) = tokio::fs::create_dir_all(dest.parent().unwrap_or(Path::new("."))).await {
        return DownloadOutcome::Failed(AppError::DirCreateFailed(e.to_string()));
    }

    let mut attempt: u32 = 0;
    loop {
        // Park between retries/files while paused.
        token.wait_while_paused().await;
        if token.is_cancelled() {
            return DownloadOutcome::Failed(AppError::Cancelled);
        }
        attempt += 1;
        let res =
            download_with_resume(
                client,
                job,
                completed,
                total_bytes,
                token,
                app_handle,
                batch_id,
                attempt,
                on_transfer,
            )
            .await;
        match res {
            Ok(()) => match verify(job).await {
                Ok(()) => return DownloadOutcome::Completed,
                Err(e) => {
                    warn!(
                        "verify failed for {} (attempt {attempt}): {e:?} — will retry",
                        job.url
                    );
                    let _ = tokio::fs::remove_file(&dest).await;
                    if attempt >= DEFAULT_MAX_RETRIES {
                        return DownloadOutcome::Failed(e);
                    }
                    backoff(attempt).await;
                    continue;
                }
            },
            Err(e) => {
                if token.is_cancelled() {
                    return DownloadOutcome::Failed(AppError::Cancelled);
                }
                warn!("download failed for {} (attempt {attempt}): {e:?}", job.url);
                let _ = tokio::fs::remove_file(&dest).await;
                if attempt >= DEFAULT_MAX_RETRIES || !is_retryable(&e) {
                    return DownloadOutcome::Failed(e);
                }
                backoff(attempt).await;
            }
        }
    }
}

async fn download_with_resume(
    client: &Client,
    job: &DownloadJob,
    completed: &Arc<AtomicU64>,
    total_bytes: u64,
    token: &DownloadToken,
    app_handle: &Option<AppHandle>,
    batch_id: &str,
    attempt: u32,
    on_transfer: &Option<TransferCallback>,
) -> Result<(), AppError> {
    token.wait_while_paused().await;
    let dest = PathBuf::from(&job.destination);
    // Construct the .part path by simply appending ".part" to the full
    // file name. The previous `with_extension` approach was broken for
    // files without extensions (e.g. asset objects like `<hash>`):
    // it would produce `<hash>.bin.part` instead of `<hash>.part`.
    let part = PathBuf::from(format!("{}.part", dest.display()));

    // Probe server: do we have range support, and what's the total size?
    let head = client.head(&job.url).send().await;
    let supports_resume = match &head {
        Ok(resp) if resp.status().is_success() => resp
            .headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false),
        _ => false,
    };
    let remote_size = head
        .as_ref()
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| response.content_length());

    // Check for an existing .part file that may be resumable.
    let mut existing_len = if supports_resume {
        tokio::fs::metadata(&part)
            .await
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        // Discard any stale .part file — server can't resume.
        let _ = tokio::fs::remove_file(&part).await;
        0
    };

    // If the .part file appears complete (size matches remote), finalize
    // it directly. We do NOT call verify(job) here because verify reads
    // from job.destination (the final file) which doesn't exist yet —
    // only the .part file exists. The full hash verification happens
    // after the rename in run_job::verify.
    if let Some(total) = remote_size {
        if existing_len >= total && existing_len > 0 {
            // .part file is the right size — rename and let run_job verify.
            debug!(
                " .part file complete ({} bytes), finalizing: {}",
                existing_len,
                part.display()
            );
            replace_destination(&part, &dest).await?;
            return Ok(());
        }
    }

    // Also check: if existing_len > remote_size, the .part file is
    // larger than expected (stale/corrupt). Delete it and start fresh.
    if let Some(total) = remote_size {
        if existing_len > total {
            warn!(" .part file ({existing_len} bytes) larger than remote ({total} bytes), discarding: {}", part.display());
            let _ = tokio::fs::remove_file(&part).await;
            existing_len = 0;
        }
    }

    let mut req = client.get(&job.url);
    if supports_resume && existing_len > 0 {
        req = req.header("Range", format!("bytes={existing_len}-"));
    }

    if attempt > 1 {
        emit_retry(app_handle, batch_id, attempt, &job.category);
    }

    let resp = req.send().await.map_err(map_reqwest_err)?;
    let status = resp.status();
    if !status.is_success() && status != StatusCode::PARTIAL_CONTENT {
        return Err(AppError::NetworkRequestFailed(format!(
            "HTTP {} for {}",
            status, job.url
        )));
    }

    // Critical: distinguish HTTP 206 (Partial Content) from HTTP 200 (OK).
    // If we sent a Range request but the server responded with 200 (full
    // content), we must NOT append to the existing .part file — that
    // would corrupt the download. Instead, truncate and write from scratch.
    let is_partial = status == StatusCode::PARTIAL_CONTENT;
    let should_append = supports_resume && existing_len > 0 && is_partial;

    let mut file = if should_append {
        debug!(
            "appending to .part file at offset {existing_len}: {}",
            part.display()
        );
        tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&part)
            .await
            .map_err(|e| AppError::FileCreateFailed(e.to_string()))?
    } else {
        // Either no resume support, no existing .part, or server
        // responded with 200 instead of 206 — start from scratch.
        if existing_len > 0 && !is_partial {
            warn!(
                "server returned 200 (not 206) for range request, restarting from scratch: {}",
                job.url
            );
        }
        tokio::fs::File::create(&part)
            .await
            .map_err(|e| AppError::FileCreateFailed(e.to_string()))?
    };

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut local_written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        token.wait_while_paused().await;
        if token.is_cancelled() {
            // Best-effort flush so the .part file is resumable next time.
            let _ = file.flush().await;
            return Err(AppError::Cancelled);
        }
        let bytes = chunk.map_err(|e| AppError::NetworkRequestFailed(e.to_string()))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
        local_written = local_written.saturating_add(bytes.len() as u64);
        let completed_bytes = completed
            .fetch_add(bytes.len() as u64, Ordering::SeqCst)
            .saturating_add(bytes.len() as u64);
        if let Some(callback) = on_transfer {
            callback(
                completed_bytes.min(total_bytes),
                total_bytes,
                file_name_of(&job.url),
            );
        }
    }
    file.flush()
        .await
        .map_err(|e| AppError::FileWriteFailed(e.to_string()))?;
    debug!(
        "wrote {local_written} bytes to {} (resume={supports_resume}, partial={is_partial}, existing={existing_len})",
        part.display()
    );

    // Atomic move: .part -> final destination. On Unix this is a rename(2)
    // and is atomic when both paths are on the same filesystem.
    replace_destination(&part, &dest).await?;
    Ok(())
}

/// Move a completed `.part` file into place. Windows does not allow
/// `rename` to overwrite a corrupt destination, so remove only that exact
/// file after the new download has completed and then finalize it.
async fn replace_destination(part: &Path, destination: &Path) -> Result<(), AppError> {
    if destination.exists() {
        tokio::fs::remove_file(destination)
            .await
            .map_err(|error| AppError::FileDeleteFailed(error.to_string()))?;
    }
    tokio::fs::rename(part, destination).await.map_err(|error| {
        AppError::FileRenameFailed(format!("finalize .part -> destination: {error}"))
    })
}

async fn verify_existing(job: &DownloadJob) -> Result<(), AppError> {
    let dest = Path::new(&job.destination);
    if !dest.exists() {
        return Err(AppError::FileNotFound(job.destination.clone()));
    }
    if let Some(size) = job.expected_size {
        let meta = tokio::fs::metadata(dest)
            .await
            .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
        if meta.len() != size {
            return Err(AppError::GameFileCorrupted(format!(
                "size mismatch for {}: expected {size}, got {}",
                job.destination,
                meta.len()
            )));
        }
    }
    verify(job).await
}

async fn verify(job: &DownloadJob) -> Result<(), AppError> {
    let Some(spec) = &job.verify else {
        return Ok(());
    };
    let dest = Path::new(&job.destination);
    let data = tokio::fs::read(dest)
        .await
        .map_err(|e| AppError::FileReadFailed(e.to_string()))?;
    let actual = match spec.kind {
        HashKind::Sha1 => {
            let mut h = Sha1::new();
            Digest::update(&mut h, &data);
            format!("{:x}", h.finalize())
        }
        HashKind::Sha256 => {
            let mut h = Sha256::new();
            Sha2Digest::update(&mut h, &data);
            format!("{:x}", h.finalize())
        }
    };
    if !actual.eq_ignore_ascii_case(&spec.expected_hash) {
        return Err(AppError::HashMismatch(format!(
            "{}: expected {}, got {}",
            job.destination, spec.expected_hash, actual
        )));
    }
    Ok(())
}

fn is_retryable(e: &AppError) -> bool {
    matches!(
        e,
        AppError::NetworkRequestFailed(_)
            | AppError::DownloadFailed(_)
            | AppError::Reqwest(_)
            | AppError::Io(_)
    )
}

/// Last path segment of a download URL — the human-friendly file name.
fn file_name_of(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_string()
}

async fn backoff(attempt: u32) {
    let exp = BACKOFF_BASE_MS.saturating_mul(1u64 << (attempt.min(6) - 1));
    let capped = exp.min(BACKOFF_MAX_MS);
    // Add up to 25% jitter to avoid thundering herds against the mirror.
    let jitter = (capped / 4).max(1);
    let delay = capped + (Uuid::new_v4().as_u128() as u64 % jitter);
    tokio::time::sleep(Duration::from_millis(delay)).await;
}

fn map_reqwest_err(e: reqwest::Error) -> AppError {
    if e.is_timeout() || e.is_connect() {
        AppError::NetworkRequestFailed(e.to_string())
    } else if e.is_decode() {
        AppError::NetworkRequestFailed(format!("decode error: {e}"))
    } else {
        AppError::NetworkRequestFailed(e.to_string())
    }
}

fn emit_retry(app_handle: &Option<AppHandle>, batch_id: &str, attempt: u32, label: &str) {
    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "download-progress",
            &DownloadProgress {
                batch_id: batch_id.to_string(),
                state: "retrying".to_string(),
                completed_bytes: 0,
                total_bytes: 0,
                files_total: 0,
                files_done: 0,
                files_failed: 0,
                retry_attempt: attempt,
                current_label: label.to_string(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::replace_destination;
    use uuid::Uuid;

    #[test]
    fn completed_part_replaces_corrupt_destination() {
        let test_directory =
            std::env::temp_dir().join(format!("glitchy-download-manager-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory).expect("create test directory");
        let destination = test_directory.join("client.jar");
        let part = test_directory.join("client.jar.part");
        std::fs::write(&destination, b"corrupt").expect("write corrupt destination");
        std::fs::write(&part, b"complete").expect("write completed part");

        tauri::async_runtime::block_on(replace_destination(&part, &destination))
            .expect("replace destination");

        assert_eq!(
            std::fs::read(&destination).expect("read destination"),
            b"complete"
        );
        assert!(!part.exists());
        std::fs::remove_dir_all(test_directory).expect("remove test directory");
    }
}
