//! Tauri command layer for CookieSincraft.
//!
//! Bridges frontend IPC (`load_image`, `upscale`, `cancel_job`,
//! `export_image`) to the domain modules (`Converter`, `Upscaler`,
//! `CpuUpscaler`, `GpuUpscaler`). All command functions return
//! `Result<T, String>` so errors are surfaced to the frontend without
//! panicking.

use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

use base64::Engine;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, Manager, State};
use tracing::warn;
use tracing_appender::rolling;

use same_file::is_same_file;

use crate::converter::{Converter, ImageFormat, PublicationGate};
use crate::cpu::CpuUpscaler;
use crate::gpu::GpuUpscaler;
use crate::settings::Settings;
use crate::upscaler::{Algorithm, Backend, JobStatus, UpscaleControl, UpscaleJob, Upscaler};
use rfd::FileDialog;

// ---------------------------------------------------------------------------
// DTOs (command I/O)
// ---------------------------------------------------------------------------

/// Metadata of a loaded image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMeta {
    pub width: u32,
    pub height: u32,
    pub format: String,
    /// Base64-encoded PNG preview for the frontend to display directly.
    pub preview: String,
}

/// Result of loading an image from base64 (R1: drag-and-drop).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadImageResult {
    /// Path to the temporary file written to disk, for use by upscale.
    pub temp_path: String,
    /// Image metadata including preview.
    pub image_meta: ImageMeta,
}

/// Payload for the `upscale` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpscaleJobDto {
    pub input_path: String,
    pub output_path: String,
    pub backend: String,
    pub algorithm: String,
    pub target_width: u32,
    pub target_height: u32,
    pub jpeg_quality: Option<u8>,
    /// Whether an existing output file may be replaced. When `false`, an
    /// existing output is never clobbered (R2: no silent overwrite).
    pub overwrite: bool,
}

/// Result of the `upscale` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpscaleResultDto {
    pub job_id: String,
    pub output_path: String,
    pub width: u32,
    pub height: u32,
    pub used_fallback: bool,
    /// Reason for CPU fallback, if `used_fallback` is true.
    pub fallback_reason: Option<String>,
    /// Base64-encoded PNG preview of the upscaled result (R3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// Progress event sent via Channel during upscale.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum UpscaleProgress {
    Started {
        job_id: String,
    },
    Progress {
        stage: String,
        percent: u8,
        message: String,
    },
    Completed {
        job_id: String,
        result: UpscaleResultDto,
    },
    Cancelled {
        job_id: String,
    },
    Failed {
        job_id: String,
        error: String,
    },
}

/// Single item in a batch upscale request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemDto {
    pub input_path: String,
    pub output_path: Option<String>,
}

/// Payload for the `batch_upscale` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpscaleDto {
    pub items: Vec<BatchItemDto>,
    pub backend: String,
    pub algorithm: String,
    pub keep_aspect_ratio: bool,
    pub target_width: u32,
    pub target_height: u32,
    pub quality: Option<u8>,
}

/// Result of the `batch_upscale` command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpscaleResultDto {
    pub batch_id: String,
    pub total: usize,
}

/// Progress event sent via Channel during batch upscale.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum BatchProgress {
    Started {
        batch_id: String,
        total: usize,
    },
    Progress {
        batch_id: String,
        done: usize,
        total: usize,
        current: String,
    },
    ItemCompleted {
        batch_id: String,
        done: usize,
        total: usize,
        current: String,
        output_path: String,
    },
    ItemFailed {
        batch_id: String,
        done: usize,
        total: usize,
        current: String,
        error: String,
    },
    Completed {
        batch_id: String,
        total: usize,
        successes: usize,
        failures: usize,
    },
    Cancelled {
        batch_id: String,
    },
    Failed {
        batch_id: String,
        error: String,
    },
}

// ---------------------------------------------------------------------------
// App State (path allowlist + job tracking)
// ---------------------------------------------------------------------------

/// Tracks paths authorized by file dialogs and running jobs for cancellation.
///
/// Read and write authorizations are deliberately separated:
/// - `allowed_reads`: paths the user selected to read (file picker, drag-drop,
///   recent files). Write-only paths can never become recent.
/// - `allowed_write_folders`: output folders explicitly selected via the folder
///   picker. Writes are permitted directly underneath them.
/// - `allowed_writes`: individual save-file paths explicitly granted write via
///   the save dialog. Derived output paths are never persistently granted here,
///   so a derived path can never later authorize `convert_image`/`export_image`
///   replacement.
pub struct AppState {
    /// Paths authorized for reading.
    allowed_reads: Mutex<HashSet<String>>,
    /// Output folders explicitly selected by the user (folder picker).
    allowed_write_folders: Mutex<HashSet<String>>,
    /// Individual save-file paths explicitly granted write (save dialog).
    allowed_writes: Mutex<HashSet<String>>,
    running_jobs: Mutex<HashMap<String, Arc<PublicationGate>>>,
    /// Rate limiter for check_path_exists: atomic counter with timestamp window
    path_check_count: AtomicU32,
    path_check_window_start: Mutex<Instant>,
    /// Current session temp directory for cleanup on drop
    session_dir: Mutex<Option<PathBuf>>,
}

/// Per-job cancellation and publication state.
pub struct JobEntry {
    /// Shared cancel flag. `cancel_job` sets this to `true`; the upscaler and
    /// converter publication gate check it.
    pub cancel: Arc<PublicationGate>,
}
impl AppState {
    pub fn new() -> Self {
        // Initialize tracing for error logging (SEC9) with file appender and rotation
        let log_dir = Self::log_dir();
        let _ = std::fs::create_dir_all(&log_dir);
        let file_appender = rolling::daily(&log_dir, "cookiesincraft.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        let _ = tracing_subscriber::fmt()
            .json()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_writer(non_blocking)
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .try_init();

        // Clean up stale temp files on startup (SEC5)
        Self::cleanup_stale_temp_dirs();

        Self {
            allowed_reads: Mutex::new(HashSet::new()),
            allowed_write_folders: Mutex::new(HashSet::new()),
            allowed_writes: Mutex::new(HashSet::new()),
            running_jobs: Mutex::new(HashMap::new()),
            path_check_count: AtomicU32::new(0),
            path_check_window_start: Mutex::new(Instant::now()),
            session_dir: Mutex::new(None),
        }
    }

    /// Get platform-specific log directory (SEC9)
    fn log_dir() -> PathBuf {
        if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::temp_dir())
                .join("CookieSincraft")
                .join("logs")
        } else {
            dirs::data_dir()
                .unwrap_or_else(|| std::env::temp_dir())
                .join("CookieSincraft")
                .join("logs")
        }
    }

    /// Clean up temp directories older than 24 hours (SEC5).
    /// Also enforces quota across ALL session directories.
    fn cleanup_stale_temp_dirs() {
        let temp_root = std::env::temp_dir().join("cookiesincraft");
        if !temp_root.exists() {
            return;
        }
        let now = std::time::SystemTime::now();
        let max_age = std::time::Duration::from_secs(24 * 60 * 60);
        const MAX_TOTAL_QUOTA: u64 = 500 * 1024 * 1024; // 500 MB total across all sessions
        let mut total_size: u64 = 0;
        let mut dirs_to_check = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&temp_root) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        if let Ok(modified) = metadata.modified() {
                            if now.duration_since(modified).unwrap_or_default() > max_age {
                                let _ = std::fs::remove_dir_all(entry.path());
                            } else {
                                dirs_to_check.push(entry.path());
                            }
                        }
                    }
                }
            }
        }

        // Enforce global quota across all active session dirs
        for dir in &dirs_to_check {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        total_size += metadata.len();
                    }
                }
            }
        }

        if total_size > MAX_TOTAL_QUOTA {
            // Delete oldest directories first until under quota
            let mut dirs_with_age: Vec<_> = dirs_to_check
                .iter()
                .filter_map(|d| {
                    std::fs::metadata(d)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|m| (d.clone(), m))
                })
                .collect();
            dirs_with_age.sort_by_key(|(_, m)| *m);
            for (dir, _) in dirs_with_age {
                if total_size <= MAX_TOTAL_QUOTA {
                    break;
                }
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        if let Ok(metadata) = entry.metadata() {
                            total_size = total_size.saturating_sub(metadata.len());
                        }
                    }
                }
                let _ = std::fs::remove_dir_all(&dir);
            }
        }
    }

    /// Authorize `path` for reading (file picker, drag-drop temp, recent file).
    pub fn allow_read(&self, path: &str) {
        self.allowed_reads.lock().unwrap().insert(path.to_string());
    }

    /// Authorize an output folder for writing (folder picker). Writes are
    /// permitted directly underneath this folder.
    pub fn allow_write_folder(&self, path: &str) {
        self.allowed_write_folders
            .lock()
            .unwrap()
            .insert(path.to_string());
    }

    /// Authorize a single save-file path for writing (save dialog).
    pub fn allow_write_file(&self, path: &str) {
        self.allowed_writes.lock().unwrap().insert(path.to_string());
    }

    pub fn check_read(&self, path: &str) -> Result<(), String> {
        let paths = self.allowed_reads.lock().unwrap();
        if paths.contains(path) {
            Ok(())
        } else {
            Err(format!(
                "access denied: path '{}' was not selected through a file dialog",
                path
            ))
        }
    }

    /// Authorize `path` for writing only when it is an exact save-file grant
    /// (explicitly selected via the save dialog). Folder grants are NOT checked
    /// here — they are only usable via `is_derived_output_allowed` for derived
    /// output path validation (e.g. upscale `output` paths).
    ///
    /// Paths that are merely under a selected output folder will be rejected;
    /// callers that need folder-based derivation checks should use
    /// `is_derived_output_allowed` instead.
    pub fn check_write(&self, path: &str) -> Result<(), String> {
        // Exact explicit save-file grant only.
        {
            let writes = self.allowed_writes.lock().unwrap();
            if writes.contains(path) {
                return Ok(());
            }
        }

        Err(format!(
            "access denied: path '{}' was not selected through a save dialog",
            path
        ))
    }

    /// Register a job and return its shared cancel token. Returns `None` when
    /// the job is already running (prevents duplicate registration).
    pub fn start_job(&self, job_id: &str) -> Option<Arc<PublicationGate>> {
        let mut jobs = self.running_jobs.lock().unwrap();
        if jobs.contains_key(job_id) {
            None
        } else {
            let gate = Arc::new(PublicationGate::new());
            jobs.insert(job_id.to_string(), Arc::clone(&gate));
            Some(gate)
        }
    }

    pub fn job_gate(&self, job_id: &str) -> Result<Arc<PublicationGate>, String> {
        let jobs = self.running_jobs.lock().unwrap();
        jobs.get(job_id)
            .cloned()
            .ok_or_else(|| "job not found".to_string())
    }

    /// Request cancellation for `job_id`. Sets the shared `Arc<AtomicBool>`
    /// flag that the upscaler and converter publication gate check. Never
    /// removes the job from the registry — the job stays registered until its
    /// terminal event (Completed / Cancelled / Failed).
    pub fn cancel_job(&self, job_id: &str) -> Result<(), String> {
        let gate = self.job_gate(job_id)?;
        gate.request_cancel()
    }

    pub fn is_running(&self, job_id: &str) -> bool {
        self.running_jobs.lock().unwrap().contains_key(job_id)
    }

    /// Remove `job_id` from the registry. Called once at actual exit
    /// (Completed / Cancelled / Failed), never before.
    pub fn finish_job(&self, job_id: &str) {
        self.running_jobs.lock().unwrap().remove(job_id);
    }
    /// Set the session temp directory for cleanup on drop
    pub fn set_session_dir(&self, path: PathBuf) {
        self.session_dir.lock().unwrap().replace(path);
    }

    /// Clear session directory reference (called when session_dir is cleaned up externally)
    pub fn clear_session_dir(&self) {
        self.session_dir.lock().unwrap().take();
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // Best-effort cleanup of our session directory on process exit (SEC5)
        if let Some(dir) = self.session_dir.lock().unwrap().take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(all(test, windows))]
impl AppState {
    pub(crate) fn isolated_for_tests() -> Self {
        Self {
            allowed_reads: Mutex::new(HashSet::new()),
            allowed_write_folders: Mutex::new(HashSet::new()),
            allowed_writes: Mutex::new(HashSet::new()),
            running_jobs: Mutex::new(HashMap::new()),
            path_check_count: AtomicU32::new(0),
            path_check_window_start: Mutex::new(Instant::now()),
            session_dir: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static NEXT_JOB_ID: AtomicU64 = AtomicU64::new(1);

fn next_job_id() -> String {
    let n = NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed);
    format!("job-{n}")
}

/// True when `input` and `output` refer to the same file, including Windows
/// same-file aliases (symlinks/junctions resolved canonically) and hardlinks on
/// both Windows and Unix (via the `same_file` crate, which uses native file
/// handle information on Windows so hardlinks are detected there too).
/// Non-existent paths fall back to direct string and lexical-dot comparison.
fn same_file(input: &Path, output: &Path) -> bool {
    // Fast path for trivially identical paths.
    if input == output {
        return true;
    }
    // same_file handles canonicalization (symlinks/junctions) and hardlinks
    // cross-platform, including Windows hardlinks via native file handles.
    // Requires both files to exist.
    if is_same_file(input, output).unwrap_or(false) {
        return true;
    }
    // Lexical normalization for non-existent paths (e.g. "./" components).
    // "." segments are always safe to drop; ".." is preserved conservatively.
    normalize_dots(input) == normalize_dots(output)
}

/// Lexically drop `.` (CurDir) components from a path without touching the
/// filesystem. Safe because `.` always refers to the current directory.
fn normalize_dots(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            comp => result.push(comp),
        }
    }
    result
}

fn is_windows_reserved_name(name: &str) -> bool {
    let upper = name.to_uppercase();
    let base = upper.split('.').next().unwrap_or(&upper);
    matches!(
        base,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

/// Validate a derived output location for upscale output.
///
/// Allowed when:
/// - `output` is in the same folder as `input`, OR
/// - `output`'s parent is an explicitly selected output folder (literal or
///   canonical, tolerating symlinked/junctioned folders).
///
/// Always rejected: path traversal (`..`), Windows alternate data streams
/// (`:` in a name component), illegal filename characters, and reserved /
/// non-normal Windows file names. Existing symlink/junction escapes are
/// rejected: the canonical parent must still be the input folder or a selected
/// output folder.
///
/// Derived paths are never persistently granted to write; this function only
/// validates the location. It does not consult the (per-session) write grants.
fn is_derived_output_allowed(state: &AppState, input_path: &Path, output_path: &Path) -> bool {
    let output_parent = match output_path.parent() {
        Some(p) => p,
        None => return false,
    };
    let input_parent = match input_path.parent() {
        Some(p) => p,
        None => return false,
    };

    // Reject path traversal (`..` components).
    if output_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return false;
    }

    // Reject Windows ADS and illegal filename characters, plus reserved /
    // non-normal Windows file names.
    if let Some(name) = output_path.file_name().and_then(|s| s.to_str()) {
        if name.is_empty()
            || name.contains(':')
            || name.contains(['/', '\\', '*', '?', '"', '<', '>', '|'])
            || is_windows_reserved_name(name)
        {
            return false;
        }
    }

    // Same folder as the input file is always allowed.
    if input_parent == output_parent {
        return true;
    }

    // Folder-grant lookup (literal or canonical parent).
    let folder_allowed = |folder: &Path| {
        let set = state.allowed_write_folders.lock().unwrap();
        set.contains(&folder.to_string_lossy().to_string())
    };

    // Under an explicitly selected output folder (literal).
    if folder_allowed(output_parent) {
        return true;
    }

    // Canonical-parent checks where sensible (symlinks / junctions).
    if let Ok(canonical) = output_parent.canonicalize() {
        // Canonical parent is a selected output folder.
        if folder_allowed(&canonical) {
            return true;
        }
        // Canonical parent equals the canonical input folder → same source
        // folder through a symlink alias.
        if let Ok(input_canonical) = input_parent.canonicalize() {
            if canonical == input_canonical {
                return true;
            }
        }
        // Otherwise the parent is a symlink/junction escape → reject.
        return false;
    }

    false
}

/// Validate target dimensions against limits (R5).
/// Max dimension: 4096x4096, max buffer: 64 MB (RGBA8 = 4 bytes/pixel).
pub fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    const MAX_DIM: u32 = 4096;
    const MAX_BUFFER_BYTES: u64 = 64 * 1024 * 1024; // 64 MB

    if width == 0 || height == 0 {
        return Err("target dimensions must be non-zero".to_string());
    }
    if width > MAX_DIM || height > MAX_DIM {
        return Err(format!(
            "target dimensions exceed limit: {}x{} (max {}x{})",
            width, height, MAX_DIM, MAX_DIM
        ));
    }
    // RGBA8 = 4 bytes/pixel, use checked_mul to prevent overflow (C4)
    let buffer_bytes = (width as u64)
        .checked_mul(height as u64)
        .and_then(|v| v.checked_mul(4))
        .ok_or("dimension calculation overflow")?;
    if buffer_bytes > MAX_BUFFER_BYTES {
        return Err(format!(
            "estimated buffer size {} MB exceeds 64 MB limit",
            buffer_bytes / (1024 * 1024)
        ));
    }
    Ok(())
}

fn parse_backend(value: &str) -> Result<Backend, String> {
    match value.trim().to_lowercase().as_str() {
        "cpu" => Ok(Backend::Cpu),
        "gpu" => Ok(Backend::Gpu),
        other => Err(format!(
            "invalid backend '{other}': expected 'cpu' or 'gpu'"
        )),
    }
}

fn parse_algorithm(value: &str) -> Result<Algorithm, String> {
    match value.trim().to_lowercase().as_str() {
        "nearest" => Ok(Algorithm::Nearest),
        "bilinear" => Ok(Algorithm::Bilinear),
        "catmullrom" | "catmull-rom" => Ok(Algorithm::CatmullRom),
        "lanczos3" | "lanczos-3" => Ok(Algorithm::Lanczos3),
        "fsr1_0" | "fsr-1-0" | "fsr1" | "fsr 1.0" | "fsr-1.0" => Ok(Algorithm::Fsr1_0),
        "anime4k" | "anime-4k" => Ok(Algorithm::Anime4k),
        "nis" => Ok(Algorithm::Nis),
        other => Err(format!(
            "invalid algorithm '{other}': expected nearest, bilinear, catmullrom, lanczos3, fsr1_0, anime4k, or nis"
        )),
    }
}

fn parse_image_format(value: &str) -> Result<ImageFormat, String> {
    match value.trim().to_lowercase().as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "webp" => Ok(ImageFormat::WebP),
        "gif" => Ok(ImageFormat::Gif),
        "tiff" | "tif" => Ok(ImageFormat::Tiff),
        "bmp" => Ok(ImageFormat::Bmp),
        "ico" => Ok(ImageFormat::Ico),
        "pnm" | "pbm" | "pgm" | "ppm" | "pam" => Ok(ImageFormat::Pnm),
        other => Err(format!(
            "unsupported image format '{other}': expected png, jpeg, webp, gif, tiff, bmp, ico, or pnm"
        )),
    }
}

fn format_from_path(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "png" => "PNG",
            "jpg" | "jpeg" => "JPEG",
            "webp" => "WEBP",
            "gif" => "GIF",
            "tiff" | "tif" => "TIFF",
            "bmp" => "BMP",
            "ico" => "ICO",
            "pnm" | "pbm" | "pgm" | "ppm" | "pam" => "PNM",
            _ => "UNKNOWN",
        })
        .unwrap_or("UNKNOWN")
        .to_string()
}

fn build_job(
    job_id: &str,
    dto: &UpscaleJobDto,
    input_path: PathBuf,
    output_path: PathBuf,
    backend: Backend,
    algorithm: Algorithm,
) -> UpscaleJob {
    UpscaleJob {
        id: job_id.to_string(),
        input_path,
        output_path,
        backend,
        algorithm,
        target_width: dto.target_width,
        target_height: dto.target_height,
        jpeg_quality: dto.jpeg_quality,
        status: JobStatus::Running,
    }
}

fn send_progress(channel: &Channel<UpscaleProgress>, event: UpscaleProgress) {
    let _ = channel.send(event);
}

/// RAII guard that guarantees a job is removed from the running-jobs registry
/// exactly once, even when an early return (e.g. decode failure) skips the
/// manual `finish_job` call. Prevents registry leaks that would stall the
/// next batch item.
struct JobGuard<'a> {
    state: &'a AppState,
    job_id: String,
}

impl<'a> Drop for JobGuard<'a> {
    fn drop(&mut self) {
        self.state.finish_job(&self.job_id);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load an image and return its metadata plus a base64-encoded PNG preview.
#[tauri::command]
pub fn load_image(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<ImageMeta, String> {
    if state.check_read(&path).is_err() && crate::settings::is_recent_file(&app, &path) {
        state.allow_read(&path);
    }
    state.check_read(&path)?;
    let image = Converter::load_image(PathBuf::from(&path).as_path()).map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();

    // Validate input image dimensions against limits (R5)
    validate_dimensions(width, height)?;

    let format = format_from_path(std::path::Path::new(&path));
    let preview = image_to_base64_preview(&image);
    Ok(ImageMeta {
        width,
        height,
        format,
        preview,
    })
}

/// Check if a path exists on the filesystem (R2: overwrite confirmation).
/// This bypasses the allowlist but is rate-limited (20 requests/minute) and audited (SEC3).
/// Note: Intentionally bypasses allowlist for overwrite confirmation flow, but has rate limiting.
#[tauri::command]
pub fn check_path_exists(path: String, state: State<'_, AppState>) -> Result<bool, String> {
    // Rate limiting: 20 requests per minute (SEC3) - atomic check-and-increment with sliding window
    {
        let mut window_start = state.path_check_window_start.lock().unwrap();
        let now = Instant::now();

        // Reset window if more than 60 seconds have passed
        if now.duration_since(*window_start) >= std::time::Duration::from_secs(60) {
            *window_start = now;
            state.path_check_count.store(0, Ordering::Relaxed);
        }

        // Atomically increment and check
        let current = state.path_check_count.fetch_add(1, Ordering::AcqRel) + 1;
        if current > 20 {
            warn!("check_path_exists rate limit exceeded");
            return Err("path check rate limit exceeded".to_string());
        }
    }

    // Only check existence, no allowlist check (S2, SEC3) - intentional for overwrite flow
    // Sanitize error to avoid path disclosure (SEC4)
    std::path::Path::new(&path)
        .try_exists()
        .map_err(|_| "path check failed".to_string())
}

/// Load an image from base64, write to isolated temp directory, and return metadata (R1: drag-and-drop).
#[tauri::command]
pub fn load_image_from_base64(
    base64: String,
    suggested_name: String,
    state: State<'_, AppState>,
) -> Result<LoadImageResult, String> {
    // Input size check: base64 string must not exceed ~70 MB (SEC2)
    const MAX_BASE64_LEN: usize = 70 * 1024 * 1024;
    if base64.len() > MAX_BASE64_LEN {
        return Err("base64 input too large".to_string());
    }

    // Filename sanitization: prevent path traversal (SEC1, B1)
    // Also block Windows reserved names (CON, PRN, AUX, NUL, COM1-9, LPT1-9)
    let clean_name = std::path::Path::new(&suggested_name)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| {
            !s.is_empty()
                && !s.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
                && !is_windows_reserved_name(s)
        })
        .ok_or("invalid suggested filename")?;

    // Create isolated temp directory (cross-platform, SEC5, SEC8)
    let temp_root = std::env::temp_dir().join("cookiesincraft");
    let session_dir = temp_root.join(format!("{}_{}", std::process::id(), fastrand::u64(..)));
    std::fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

    // Register session dir for cleanup on drop (SEC5)
    state.set_session_dir(session_dir.clone());

    // Decode base64
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&base64)
        .map_err(|_| "invalid base64 data".to_string())?;

    // Load image through the same bounded decode path as file loads.
    let image = Converter::load_image_from_bytes(&bytes).map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();

    // Validate input image dimensions against limits (R5)
    validate_dimensions(width, height)?;

    let format = format_from_path(std::path::Path::new(&clean_name));

    // Write to temp file
    let temp_path = session_dir.join(&clean_name);
    image
        .save(&temp_path)
        .map_err(|e| format!("failed to write temp file: {}", e))?;

    // Temp directory quota check: single file 100 MB, global total 500 MB across all sessions (SEC2, I1, SEC11)
    // Enforced in cleanup_stale_temp_dirs and here for current session
    let mut total_size: u64 = 0;
    let mut max_file_size: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(&session_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                let size = metadata.len();
                total_size += size;
                if size > max_file_size {
                    max_file_size = size;
                }
            }
        }
    }
    // Also check global quota across all session dirs
    if let Ok(entries) = std::fs::read_dir(&temp_root) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                        for sub_entry in sub_entries.flatten() {
                            if let Ok(sub_meta) = sub_entry.metadata() {
                                total_size = total_size.saturating_add(sub_meta.len());
                            }
                        }
                    }
                }
            }
        }
    }
    const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
    const MAX_TOTAL_SIZE: u64 = 500 * 1024 * 1024; // 500 MB global
    if max_file_size > MAX_FILE_SIZE || total_size > MAX_TOTAL_SIZE {
        let _ = std::fs::remove_file(&temp_path);
        return Err("temp quota exceeded".to_string());
    }

    // Add session directory to allow_read (directory-level, SEC6, SEC9)
    state.allow_read(&session_dir.to_string_lossy());
    // Authorize the temp file itself so check_read (execute_upscale) and
    // add_recent_file (recent history) accept this session's drag-drop file.
    state.allow_read(&temp_path.to_string_lossy());

    // Generate preview
    let preview = image_to_base64_preview(&image);

    Ok(LoadImageResult {
        temp_path: temp_path.to_string_lossy().to_string(),
        image_meta: ImageMeta {
            width,
            height,
            format,
            preview,
        },
    })
}

/// Add a file to the recent-files list and persist.
///
/// R1: only accept a path that has already been authorized for reading this
/// session (present in the read allowlist). This prevents an unapproved path
/// from the renderer from becoming a trusted recent-file record.
#[tauri::command]
pub fn add_recent_file(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Settings, String> {
    if state.check_read(&path).is_err() {
        return Err(format!(
            "recent file not authorized: path '{}' was not selected through a file dialog",
            path
        ));
    }
    crate::settings::persist_recent_file(&app, &path)
}

fn image_to_base64_preview(image: &image::DynamicImage) -> String {
    const MAX_DIM: u32 = 800;
    let (w, h) = image.dimensions();
    let preview_img = if w > MAX_DIM || h > MAX_DIM {
        let scale = MAX_DIM as f32 / w.max(h) as f32;
        image.resize(
            (w as f32 * scale) as u32,
            (h as f32 * scale) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image.clone()
    };

    let mut buf = Vec::new();
    preview_img
        .write_to(&mut Cursor::new(&mut buf), image::ImageOutputFormat::Png)
        .expect("png encoding infallible");
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf)
    )
}

/// Open a native file picker dialog and return the selected image path.
/// The selected path is added to the read allowlist.
#[tauri::command]
pub fn select_image(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let selected = FileDialog::new()
        .add_filter(
            "Image",
            &[
                "png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "bmp", "ico", "pnm", "pbm",
                "pgm", "ppm", "pam",
            ],
        )
        .pick_file();
    if let Some(ref path) = selected {
        let path_str = path.to_string_lossy().to_string();
        state.allow_read(&path_str);
    }
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
}

/// Open a native save file dialog and return the selected path.
/// The selected path is added to the write allowlist.
#[tauri::command]
pub fn select_save_path(
    default_name: String,
    filters: Vec<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let mut dialog = FileDialog::new().set_file_name(&default_name);
    for filter in filters {
        if filter.len() >= 2 {
            let name = &filter[0];
            let exts: Vec<&str> = filter[1..].iter().map(|s| s.as_str()).collect();
            dialog = dialog.add_filter(name, &exts);
        }
    }
    let selected = dialog.save_file();
    if let Some(ref path) = selected {
        let path_str = path.to_string_lossy().to_string();
        state.allow_write_file(&path_str);
    }
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
}

/// Open a native folder picker dialog and return the selected folder path.
/// The selected path is added to the write allowlist so it can be used as
/// the output folder for settings persistence and batch upscale.
#[tauri::command]
pub fn select_output_folder(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let selected = FileDialog::new().pick_folder();
    if let Some(ref path) = selected {
        let path_str = path.to_string_lossy().to_string();
        state.allow_write_folder(&path_str);
    }
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
}

/// Open a native multi-file picker dialog and return the selected image paths.
/// Each selected path is added to the read allowlist so it can be used as
/// input for upscale or batch_upscale.
#[tauri::command]
pub fn select_multiple_images(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let selected: Vec<PathBuf> = FileDialog::new()
        .add_filter(
            "Image",
            &[
                "png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "bmp", "ico", "pnm", "pbm",
                "pgm", "ppm", "pam",
            ],
        )
        .pick_files()
        .unwrap_or_default();
    let mut paths = Vec::new();
    for path in selected {
        let path_str = path.to_string_lossy().to_string();
        state.allow_read(&path_str);
        paths.push(path_str);
    }
    Ok(paths)
}

/// Terminal outcome of an upscale operation, distinguishing cancellation from failure.
#[derive(Debug, Clone)]
pub enum ExecuteUpscaleError {
    Cancelled(String),
    Failed(String),
}

impl From<String> for ExecuteUpscaleError {
    fn from(msg: String) -> Self {
        ExecuteUpscaleError::Failed(msg)
    }
}

impl ExecuteUpscaleError {
    /// Convert from a crate error, preserving Cancellation status.
    /// Handles both UpscaleError::Cancelled and ConverterError::Cancelled.
    fn from_crate_error(err: crate::converter::ConverterError) -> Self {
        match err {
            crate::converter::ConverterError::Cancelled(_) => Self::Cancelled(err.to_string()),
            _ => Self::Failed(err.to_string()),
        }
    }
    /// Convert to string for Tauri command signature.
    fn to_string_error(self) -> String {
        match self {
            ExecuteUpscaleError::Cancelled(msg) => msg,
            ExecuteUpscaleError::Failed(msg) => msg,
        }
    }
}

impl From<&str> for ExecuteUpscaleError {
    fn from(msg: &str) -> Self {
        ExecuteUpscaleError::Failed(msg.to_string())
    }
}

/// Core upscale logic shared by `upscale` and `batch_upscale`.
/// `on_event` is `None` in batch mode (per-item progress is not streamed).
async fn execute_upscale(
    job_id: String,
    dto: UpscaleJobDto,
    input_path: PathBuf,
    output_path: PathBuf,
    backend: Backend,
    algorithm: Algorithm,
    on_event: Option<&Channel<UpscaleProgress>>,
    gate: Arc<PublicationGate>,
    state: &AppState,
    app: &AppHandle,
    overwrite: bool,
    keep_aspect_ratio: bool,
) -> Result<UpscaleResultDto, ExecuteUpscaleError> {
    // Build domain job from DTO
    let mut job = build_job(&job_id, &dto, input_path, output_path, backend, algorithm);

    // Check path permissions
    state.check_read(&job.input_path.to_string_lossy())?;
    // Derived output path validation. Only derived locations (same folder as
    // input, or under an explicitly selected output folder) are accepted;
    // arbitrary absolute output paths are rejected. The derived path is NOT
    // persistently granted to write, so it cannot later authorize
    // convert/export replacement.
    if !is_derived_output_allowed(state, job.input_path.as_path(), job.output_path.as_path()) {
        return Err(ExecuteUpscaleError::Failed(format!(
            "access denied: output path '{}' is not in the input folder or a selected output folder",
            job.output_path.to_string_lossy()
        )));
    }

    // R1: same-file guard — output must not overwrite the input (including
    // Windows same-file aliases and hardlinks).
    if same_file(&job.input_path, &job.output_path) {
        return Err(ExecuteUpscaleError::Failed(format!(
            "output path '{}' is the same file as the input",
            job.output_path.to_string_lossy()
        )));
    }

    // R1: Windows reserved device names must never be used as output filenames.
    if let Some(name) = job.output_path.file_name().and_then(|s| s.to_str()) {
        if is_windows_reserved_name(name) {
            return Err(ExecuteUpscaleError::Failed(format!(
                "output filename '{}' is a Windows reserved device name",
                name
            )));
        }
    }

    if let Some(channel) = on_event {
        let _ = channel.send(UpscaleProgress::Started {
            job_id: job_id.clone(),
        });
    }

    let image = Converter::load_image(job.input_path.as_path()).map_err(|e| {
        let error = ExecuteUpscaleError::from_crate_error(e);
        if let Some(channel) = on_event {
            let _ = channel.send(match &error {
                ExecuteUpscaleError::Failed(message) => UpscaleProgress::Failed {
                    job_id: job_id.clone(),
                    error: message.clone(),
                },
                ExecuteUpscaleError::Cancelled(_) => UpscaleProgress::Cancelled {
                    job_id: job_id.clone(),
                },
            });
        }
        error
    })?;
    if keep_aspect_ratio {
        let src_w = image.width() as f64;
        let src_h = image.height() as f64;
        let target_w = job.target_width as f64;
        let target_h = job.target_height as f64;
        let min_src = src_w.min(src_h);
        let min_target = target_w.min(target_h);
        let max_src = src_w.max(src_h);
        let scale = (min_target / min_src).min(4096.0 / max_src);
        job.target_width = ((src_w * scale).round() as u32).max(1).min(4096);
        job.target_height = ((src_h * scale).round() as u32).max(1).min(4096);
    }

    if let Some(channel) = on_event {
        let _ = channel.send(UpscaleProgress::Progress {
            stage: "loading".to_string(),
            percent: 5,
            message: "Image loaded, starting upscale".to_string(),
        });
    }

    // Clone channel for use in background task
    let on_event_clone = on_event.cloned();
    let job_clone = job.clone();
    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        // Access state via AppHandle (which is 'static)
        let state_clone = app_clone.state::<AppState>().inner();

        let cancel = gate.cancel_token();

        // Check for cancellation before starting via the shared token.
        if cancel.load(Ordering::Relaxed) {
            return Err(ExecuteUpscaleError::Cancelled(
                "job cancelled before start".to_string(),
            ));
        }

        // Build UpscaleControl sharing the job's cancel token and a real
        // monotonic progress callback (10% buckets, >=100ms throttle,
        // 100% only after publish).
        let last_percent = Arc::new(std::sync::atomic::AtomicU8::new(10));
        let progress_percent = Arc::clone(&last_percent);
        let on_event_for_cb = on_event_clone.clone();
        let progress_cb = Arc::new(move |completed: usize, total: usize| {
            if total == 0 {
                return;
            }
            let pct = ((10 + completed.min(total).saturating_mul(70) / total) / 10 * 10) as u8;
            if progress_percent.fetch_max(pct, Ordering::Relaxed) >= pct {
                return;
            }
            if let Some(channel) = &on_event_for_cb {
                let _ = channel.send(UpscaleProgress::Progress {
                    stage: "upscaling".to_string(),
                    percent: pct,
                    message: format!("Upscaling... {pct}%"),
                });
            }
        });
        let control = UpscaleControl {
            cancel: cancel.clone(),
            on_progress: Some(progress_cb),
        };

        if let Some(channel) = &on_event_clone {
            let _ = channel.send(UpscaleProgress::Progress {
                stage: "upscaling".to_string(),
                percent: 10,
                message: "Upscaling...".to_string(),
            });
        }

        let (upscaled, used_fallback, fallback_reason) = match backend {
            Backend::Gpu => match GpuUpscaler::new()
                .and_then(|gpu| gpu.upscale_with_control(&job_clone, &image, control.clone()))
            {
                Ok(img) => (img, false, None),
                Err(crate::upscaler::UpscaleError::Cancelled(msg)) => {
                    return Err(ExecuteUpscaleError::Cancelled(msg));
                }
                Err(gpu_err) => {
                    // Already cancelled? Return Failed with real error, don't dispatch fallback.
                    if control.is_cancelled() {
                        return Err(ExecuteUpscaleError::Failed(gpu_err.to_string()));
                    }
                    // GPU unavailable or execution failed: fall back to CPU with Lanczos3.
                    let fallback_reason = gpu_err.to_string();
                    if let Some(channel) = &on_event_clone {
                        let _ = channel.send(UpscaleProgress::Progress {
                            stage: "fallback".to_string(),
                            percent: last_percent.load(Ordering::Relaxed),
                            message: format!(
                                "GPU failed, falling back to CPU: {}",
                                fallback_reason
                            ),
                        });
                    }
                    if control.is_cancelled() {
                        return Err(ExecuteUpscaleError::Cancelled(
                            "job cancelled during upscale".to_string(),
                        ));
                    }
                    let mut cpu_job = job_clone.clone();
                    cpu_job.algorithm = Algorithm::Lanczos3;
                    let cpu = CpuUpscaler;
                    let img = cpu
                        .upscale_with_control(&cpu_job, &image, control.clone())
                        .map_err(|e| match e {
                            crate::upscaler::UpscaleError::Cancelled(msg) => {
                                ExecuteUpscaleError::Cancelled(msg)
                            }
                            other => ExecuteUpscaleError::Failed(format!(
                                "CPU fallback after GPU failure: {other}"
                            )),
                        })?;
                    (img, true, Some(fallback_reason))
                }
            },
            Backend::Cpu => {
                let cpu = CpuUpscaler;
                let img = cpu
                    .upscale_with_control(&job_clone, &image, control.clone())
                    .map_err(|e| match e {
                        crate::upscaler::UpscaleError::Cancelled(msg) => {
                            ExecuteUpscaleError::Cancelled(msg)
                        }
                        other => ExecuteUpscaleError::Failed(other.to_string()),
                    })?;
                (img, false, None)
            }
        };

        // Check for cancellation after upscale via the shared token.
        if cancel.load(Ordering::Relaxed) {
            return Err(ExecuteUpscaleError::Cancelled(
                "job cancelled during upscale".to_string(),
            ));
        }

        if let Some(channel) = &on_event_clone {
            let _ = channel.send(UpscaleProgress::Progress {
                stage: "saving".to_string(),
                percent: 90,
                message: "Saving output...".to_string(),
            });
        }

        let output_format = parse_image_format(&format_from_path(job_clone.output_path.as_path()))?;
        Converter::save_image_with_cancel(
            &upscaled,
            job_clone.output_path.as_path(),
            output_format,
            job_clone.jpeg_quality,
            overwrite,
            Some(gate.as_ref()),
        )
        .map_err(|e| ExecuteUpscaleError::from_crate_error(e))?;

        // Allow reading the output for export
        state_clone.allow_read(&job_clone.output_path.to_string_lossy());

        let (width, height) = upscaled.dimensions();

        // Generate preview of upscaled result (R3)
        let preview = image_to_base64_preview(&upscaled);

        Ok(UpscaleResultDto {
            job_id: job_id_clone,
            output_path: job_clone.output_path.to_string_lossy().to_string(),
            width,
            height,
            used_fallback,
            fallback_reason,
            preview: Some(preview),
        })
    })
    .await;

    // Handle join error
    let result = match result {
        Ok(r) => r,
        Err(e) => Err(ExecuteUpscaleError::Failed(format!(
            "upscale task panicked: {e}"
        ))),
    };

    match &result {
        Ok(dto) => {
            if let Some(channel) = on_event {
                let _ = channel.send(UpscaleProgress::Completed {
                    job_id: job_id.clone(),
                    result: dto.clone(),
                });
            }
        }
        Err(e) => {
            if let Some(channel) = on_event {
                let _ = channel.send(match e {
                    ExecuteUpscaleError::Cancelled(_) => UpscaleProgress::Cancelled {
                        job_id: job_id.clone(),
                    },
                    ExecuteUpscaleError::Failed(error) => UpscaleProgress::Failed {
                        job_id: job_id.clone(),
                        error: error.clone(),
                    },
                });
            }
        }
    }
    result
}

/// Upscale an image according to `job`, reporting progress via `Channel`.
///
/// GPU backend is attempted first when requested; on any GPU error the job is
/// retried on the CPU with Lanczos3 and `used_fallback` is set to `true`.
/// Progress events are sent via the provided Channel.
#[tauri::command]
pub async fn upscale(
    job: UpscaleJobDto,
    on_event: Channel<UpscaleProgress>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<UpscaleResultDto, String> {
    let backend = parse_backend(&job.backend)?;
    let algorithm = parse_algorithm(&job.algorithm)?;

    // Validate dimensions against limits (R5)
    validate_dimensions(job.target_width, job.target_height)?;

    let input_path = PathBuf::from(&job.input_path);
    let output_path = PathBuf::from(&job.output_path);
    let job_id = next_job_id();
    let overwrite = job.overwrite;
    // Register the job and obtain its shared cancel token before starting.
    let gate = state
        .start_job(&job_id)
        .ok_or("another upscale job is running".to_string())?;
    let _job_guard = JobGuard {
        state: &state,
        job_id: job_id.clone(),
    };

    execute_upscale(
        job_id,
        job,
        input_path,
        output_path,
        backend,
        algorithm,
        Some(&on_event),
        gate,
        &state,
        &app,
        overwrite,
        false,
    )
    .await
    .map_err(|e| e.to_string_error())
}

/// Cancel an in-flight upscale job.
#[tauri::command]
pub async fn cancel_job(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let gate = state.job_gate(&id)?;
    tokio::task::spawn_blocking(move || gate.request_cancel())
        .await
        .map_err(|err| err.to_string())?
}

/// Generate a default output path next to the input file with `_upscaled` suffix.
fn generate_default_output_path(input_path: &str) -> PathBuf {
    let path = PathBuf::from(input_path);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!("{stem}_upscaled.{ext}"))
}

/// Batch upscale: process multiple images sequentially, reporting progress via `Channel`.
/// Cancellation is honored at item boundaries; an in-progress item finishes before stopping.
#[tauri::command]
pub async fn batch_upscale(
    dto: BatchUpscaleDto,
    on_event: Channel<BatchProgress>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<BatchUpscaleResultDto, String> {
    validate_dimensions(dto.target_width, dto.target_height)?;

    let batch_id = next_job_id();
    let total = dto.items.len();

    if total == 0 {
        return Err("no items to process".to_string());
    }

    // Check read permission for all input paths up front
    for item in &dto.items {
        state.check_read(&item.input_path)?;
    }

    // Register the batch as one parent job and reuse its cancel token for
    // every child item — no independent child registry IDs.
    let gate = state
        .start_job(&batch_id)
        .ok_or("another job is running".to_string())?;
    let cancel = gate.cancel_token();
    let _job_guard = JobGuard {
        state: &state,
        job_id: batch_id.clone(),
    };

    let _ = on_event.send(BatchProgress::Started {
        batch_id: batch_id.clone(),
        total,
    });

    let mut successes = 0usize;
    let mut failures = 0usize;

    for (index, item) in dto.items.iter().enumerate() {
        // Honor cancellation at item boundaries via the shared token.
        if cancel.load(Ordering::Relaxed) {
            let _ = on_event.send(BatchProgress::Cancelled {
                batch_id: batch_id.clone(),
            });
            return Ok(BatchUpscaleResultDto { batch_id, total });
        }

        let current = item.input_path.clone();
        let done = index + 1;

        let _ = on_event.send(BatchProgress::Progress {
            batch_id: batch_id.clone(),
            done: index,
            total,
            current: current.clone(),
        });

        // Check token just AFTER sending Progress, before decode
        if cancel.load(Ordering::Relaxed) {
            let _ = on_event.send(BatchProgress::Cancelled {
                batch_id: batch_id.clone(),
            });
            return Ok(BatchUpscaleResultDto { batch_id, total });
        }

        let output_path = match &item.output_path {
            Some(p) => PathBuf::from(p),
            None => generate_default_output_path(&item.input_path),
        };

        let backend = match parse_backend(&dto.backend) {
            Ok(b) => b,
            Err(e) => {
                failures += 1;
                let _ = on_event.send(BatchProgress::ItemFailed {
                    batch_id: batch_id.clone(),
                    done,
                    total,
                    current: current.clone(),
                    error: e,
                });
                continue;
            }
        };
        let algorithm = match parse_algorithm(&dto.algorithm) {
            Ok(a) => a,
            Err(e) => {
                failures += 1;
                let _ = on_event.send(BatchProgress::ItemFailed {
                    batch_id: batch_id.clone(),
                    done,
                    total,
                    current: current.clone(),
                    error: e,
                });
                continue;
            }
        };

        let job_dto = UpscaleJobDto {
            input_path: item.input_path.clone(),
            output_path: output_path.to_string_lossy().to_string(),
            backend: dto.backend.clone(),
            algorithm: dto.algorithm.clone(),
            target_width: dto.target_width,
            target_height: dto.target_height,
            jpeg_quality: dto.quality,
            overwrite: false, // batch: no silent overwrite; frontend issues incremented names
        };

        let item_job_id = format!("{batch_id}-item-{index}");
        match execute_upscale(
            item_job_id,
            job_dto,
            PathBuf::from(&item.input_path),
            output_path.clone(),
            backend,
            algorithm,
            None, // batch mode: no per-item upscale channel
            gate.clone(),
            &state,
            &app,
            false, // batch: no silent overwrite
            dto.keep_aspect_ratio,
        )
        .await
        {
            Ok(result) => {
                successes += 1;
                let _ = on_event.send(BatchProgress::ItemCompleted {
                    batch_id: batch_id.clone(),
                    done,
                    total,
                    current: current.clone(),
                    output_path: result.output_path.clone(),
                });
            }
            Err(crate::commands::ExecuteUpscaleError::Cancelled(_)) => {
                let _ = on_event.send(BatchProgress::Cancelled {
                    batch_id: batch_id.clone(),
                });
                return Ok(BatchUpscaleResultDto { batch_id, total });
            }
            Err(crate::commands::ExecuteUpscaleError::Failed(error)) => {
                failures += 1;
                let _ = on_event.send(BatchProgress::ItemFailed {
                    batch_id: batch_id.clone(),
                    done,
                    total,
                    current: current.clone(),
                    error,
                });
            }
        }
    }

    // Guard handles finish_job; no explicit state.finish_job needed.

    if cancel.load(Ordering::Relaxed) && successes + failures < total {
        let _ = on_event.send(BatchProgress::Cancelled {
            batch_id: batch_id.clone(),
        });
    } else {
        let _ = on_event.send(BatchProgress::Completed {
            batch_id: batch_id.clone(),
            total,
            successes,
            failures,
        });
    }

    Ok(BatchUpscaleResultDto { batch_id, total })
}

/// Export (re-encode) an already-upscaled image to the user-chosen path.
#[tauri::command]
pub fn export_image(
    path: String,
    output_path: String,
    format: String,
    quality: u8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.check_read(&path)?;
    state.check_write(&output_path)?;
    // Same-file safeguard: never export a file onto itself (incl. aliases/hardlinks).
    if same_file(
        PathBuf::from(&path).as_path(),
        PathBuf::from(&output_path).as_path(),
    ) {
        return Err("output path is the same file as the input".to_string());
    }
    let format = parse_image_format(&format)?;
    let image = Converter::load_image(PathBuf::from(&path).as_path()).map_err(|e| e.to_string())?;
    Converter::save_image(
        &image,
        PathBuf::from(&output_path).as_path(),
        format,
        Some(quality),
        true, // user chose the save destination explicitly via save dialog
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Convert an image to a different format/quality and save to a new path.
#[tauri::command]
pub fn convert_image(
    input_path: String,
    output_path: String,
    format: String,
    quality: u8,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.check_read(&input_path)?;
    state.check_write(&output_path)?;
    // Same-file safeguard: never convert a file onto itself (incl. aliases/hardlinks).
    if same_file(
        PathBuf::from(&input_path).as_path(),
        PathBuf::from(&output_path).as_path(),
    ) {
        return Err("output path is the same file as the input".to_string());
    }
    let format = parse_image_format(&format)?;
    let image =
        Converter::load_image(PathBuf::from(&input_path).as_path()).map_err(|e| e.to_string())?;
    Converter::save_image(
        &image,
        PathBuf::from(&output_path).as_path(),
        format,
        Some(quality),
        true, // user chose the save destination explicitly via save dialog
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Separate grants: read authorization does not imply write authorization.
    #[test]
    fn test_app_state_separate_grants() {
        let state = AppState::new();

        // Read grant only permits reading.
        state.allow_read("/test/path.png");
        assert!(state.check_read("/test/path.png").is_ok());
        assert!(state.check_write("/test/path.png").is_err()); // read-only, not writable

        // Explicit save-file grant permits writing exactly that file.
        state.allow_write_file("/test/save.png");
        assert!(state.check_write("/test/save.png").is_ok());
        assert!(state.check_write("/test/save_other.png").is_err()); // not the same file

        // Folder grant is NOT checked by check_write; it is only usable via
        // is_derived_output_allowed for derived output path validation.
        state.allow_write_folder("/test/outdir");
        // check_write rejects paths under a folder without an exact grant.
        assert!(state.check_write("/test/outdir/result.png").is_err());
        assert!(state.check_write("/test/outdir/sub/result.png").is_err());

        // However, is_derived_output_allowed still accepts folder-based derivation.
        let input = PathBuf::from("/other/folder/image.png");
        let output = PathBuf::from("/test/outdir/result.png");
        state.allow_read(input.to_str().unwrap());
        assert!(
            is_derived_output_allowed(&state, &input, &output),
            "output under a selected output folder should be allowed via is_derived_output_allowed"
        );

        // Read and write sets are independent.
        assert!(state.check_read("/test/save.png").is_err()); // write-only path can't be read
    }

    /// Write-only paths must not become recent (add_recent_file requires check_read).
    #[test]
    fn test_write_only_path_not_recentable() {
        let state = AppState::new();
        state.allow_write_file("/test/save.png");
        state.allow_write_folder("/test/outdir");
        // check_write now only accepts exact save-file grants, not folder grants.
        assert!(state.check_write("/test/save.png").is_ok()); // exact save grant allows write
        assert!(state.check_write("/test/outdir/result.png").is_err()); // under folder, not exact grant
        assert!(state.check_read("/test/save.png").is_err());
        assert!(state.check_read("/test/outdir/result.png").is_err());
    }

    /// Test job tracking
    #[test]
    fn test_app_state_job_tracking() {
        let state = AppState::new();
        assert!(state.start_job("job-1").is_some());
        assert!(state.start_job("job-1").is_none()); // duplicate
        assert!(state.is_running("job-1"));
        // Cancel sets the flag but keeps the job registered until finish_job.
        assert!(state.cancel_job("job-1").is_ok());
        assert!(state.is_running("job-1")); // still registered after cancel
        assert!(state.cancel_job("job-1").is_ok()); // cancel again is fine (flag already set)
        state.finish_job("job-1");
        assert!(!state.is_running("job-1")); // removed at actual exit
        assert!(state.cancel_job("job-1").is_err()); // not found after finish
    }

    /// Derived output: same folder as input is allowed.
    #[test]
    fn test_derived_output_path_same_folder_allowed() {
        let state = AppState::new();
        let input = PathBuf::from("/test/folder/image.png");
        let output = PathBuf::from("/test/folder/image_upscaled.png");
        state.allow_read(input.to_str().unwrap());
        assert!(
            is_derived_output_allowed(&state, &input, &output),
            "same-folder output should be allowed"
        );
    }

    /// Derived output: explicitly selected output folder is allowed.
    #[test]
    fn test_derived_output_path_selected_folder_allowed() {
        let state = AppState::new();
        let input = PathBuf::from("/other/folder/image.png");
        let output = PathBuf::from("/test/outdir/image_upscaled.png");
        state.allow_read(input.to_str().unwrap());
        state.allow_write_folder("/test/outdir");
        assert!(
            is_derived_output_allowed(&state, &input, &output),
            "output under a selected output folder should be allowed"
        );
    }

    /// Derived output: parent traversal is rejected.
    #[test]
    fn test_derived_output_path_parent_traversal_rejected() {
        let state = AppState::new();
        let input = PathBuf::from("/test/folder/image.png");
        let output = PathBuf::from("/test/folder/../outside.png");
        state.allow_read(input.to_str().unwrap());
        assert!(
            !is_derived_output_allowed(&state, &input, &output),
            "parent-traversal output should be rejected"
        );
    }

    /// Derived output: Windows ADS (`:` in name) is rejected.
    #[test]
    fn test_derived_output_path_ads_rejected() {
        let state = AppState::new();
        let input = PathBuf::from("/test/folder/image.png");
        let output = PathBuf::from("/test/folder/image.png:ads");
        state.allow_read(input.to_str().unwrap());
        assert!(
            !is_derived_output_allowed(&state, &input, &output),
            "ADS output name should be rejected"
        );
    }

    /// Derived output: reserved Windows device name is rejected.
    #[test]
    fn test_derived_output_path_reserved_name_rejected() {
        let state = AppState::new();
        let input = PathBuf::from("/test/folder/image.png");
        let output = PathBuf::from("/test/folder/con.txt");
        state.allow_read(input.to_str().unwrap());
        assert!(
            !is_derived_output_allowed(&state, &input, &output),
            "reserved name output should be rejected"
        );
    }

    /// Derived output: an arbitrary path outside same folder and selected
    /// folders is rejected.
    #[test]
    fn test_derived_output_path_other_disk_rejected() {
        let state = AppState::new();
        let input = PathBuf::from("C:/input/image.png");
        let output = PathBuf::from("D:/output/image_upscaled.png");
        state.allow_read(input.to_str().unwrap());
        assert!(
            !is_derived_output_allowed(&state, &input, &output),
            "other-disk output should be rejected"
        );
    }

    /// R1: Windows reserved device names are rejected as output filenames.
    #[test]
    fn test_windows_reserved_name_rejected() {
        assert!(is_windows_reserved_name("CON"));
        assert!(is_windows_reserved_name("con.txt"));
        assert!(is_windows_reserved_name("COM1"));
        assert!(is_windows_reserved_name("LPT9"));
        assert!(is_windows_reserved_name("NUL"));
        assert!(!is_windows_reserved_name("image.png"));
        assert!(!is_windows_reserved_name("document.txt"));
        assert!(!is_windows_reserved_name("auxiliary.data"));
    }

    /// R1: same_file returns true for identical paths.
    #[test]
    fn test_same_file_identical_paths() {
        let p = PathBuf::from("/tmp/cookiesincraft/123/image.png");
        assert!(same_file(&p, &p));
    }

    /// R1: same_file returns false for different files in the same folder.
    #[test]
    fn test_same_file_different_files() {
        let input = PathBuf::from("/tmp/cookiesincraft/123/input.png");
        let output = PathBuf::from("/tmp/cookiesincraft/123/input_upscaled.png");
        assert!(!same_file(&input, &output));
    }

    /// R1: same_file returns true when paths canonicalize to the same file.
    #[test]
    fn test_same_file_same_canonical_path() {
        let input = PathBuf::from("/tmp/cookiesincraft/123/image.png");
        let output = PathBuf::from("/tmp/cookiesincraft/123/./image.png");
        assert!(same_file(&input, &output));
    }

    /// R1: same_file returns true for real hard links (Windows hard_link included).
    #[test]
    fn test_same_file_hard_link() {
        let dir = std::env::temp_dir().join(format!(
            "cookiesincraft_hardlink_test_{}_{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let original = dir.join("original.png");
        let link = dir.join("link.png");
        std::fs::write(&original, b"hardlink-test-data").unwrap();

        // Create a real hard link (std::fs::hard_link works on Windows too).
        // Must succeed; no silent success if creation fails.
        std::fs::hard_link(&original, &link)
            .expect("hard link creation must succeed in test environment");

        assert!(
            same_file(&original, &link),
            "hard links should be the same file"
        );
        assert!(
            same_file(&link, &original),
            "hard links should be the same file (symmetric)"
        );
        // Different file in same folder is not the same file.
        let other = dir.join("other.png");
        std::fs::write(&other, b"other").unwrap();
        assert!(!same_file(&original, &other));
    }

    /// R5: validate_dimensions accepts 4096x4096 (exactly at limit).
    #[test]
    fn test_validate_dimensions_at_limit() {
        assert!(validate_dimensions(4096, 4096).is_ok());
    }

    /// R5: validate_dimensions rejects 4097 in either axis.
    #[test]
    fn test_validate_dimensions_over_limit() {
        assert!(validate_dimensions(4097, 1024).is_err());
        assert!(validate_dimensions(1024, 4097).is_err());
    }

    /// R5: validate_dimensions rejects zero dimensions.
    #[test]
    fn test_validate_dimensions_zero() {
        assert!(validate_dimensions(0, 1024).is_err());
        assert!(validate_dimensions(1024, 0).is_err());
    }

    /// R5: validate_dimensions rejects buffer over 64 MB (8192x8192).
    #[test]
    fn test_validate_dimensions_buffer_over_limit() {
        assert!(validate_dimensions(8192, 8192).is_err());
    }
}
