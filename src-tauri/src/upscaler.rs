//! Core domain types and the `Upscaler` trait for CookieSincraft.
//!
//! This module defines the shared domain model used by upscaling jobs:
//! backends, algorithms, job status, the job and result structs, the error
//! type, and the `Upscaler` trait. No concrete upscaler implementations live
//! here; those are added in later tasks.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use image::DynamicImage;

/// Compute backend used to run an upscaling job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// Run on the CPU.
    Cpu,
    /// Run on the GPU.
    Gpu,
}

/// Upscaling algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Nearest-neighbor upscaling.
    Nearest,
    /// Bilinear interpolation.
    Bilinear,
    /// Catmull-Rom interpolation.
    CatmullRom,
    /// Lanczos3 windowed sinc interpolation.
    Lanczos3,
    /// FSR 1.0 (FidelityFX Super Resolution).
    Fsr1_0,
    /// Anime4K upscaling.
    Anime4k,
    /// NIS (NVIDIA Image Scaling).
    Nis,
}

/// Lifecycle status of an upscaling job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    /// Job is waiting to be picked up.
    Queued,
    /// Job is currently being processed.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed.
    Failed,
    /// Job completed using a fallback backend/algorithm.
    FallbackTriggered,
}

/// An upscaling job to be executed.
#[derive(Debug, Clone)]
pub struct UpscaleJob {
    /// Unique identifier for the job.
    pub id: String,
    /// Path to the source image.
    pub input_path: PathBuf,
    /// Path where the upscaled image should be written.
    pub output_path: PathBuf,
    /// Backend to run the job on.
    pub backend: Backend,
    /// Algorithm to use for upscaling.
    pub algorithm: Algorithm,
    /// Target output width in pixels.
    pub target_width: u32,
    /// Target output height in pixels.
    pub target_height: u32,
    /// Optional JPEG quality (1..=100) when the output is JPEG.
    pub jpeg_quality: Option<u8>,
    /// Current lifecycle status of the job.
    pub status: JobStatus,
}

/// Result of an executed upscaling job.
#[derive(Debug, Clone)]
pub struct UpscaleResult {
    /// Identifier of the job this result belongs to.
    pub job_id: String,
    /// Path to the written output image.
    pub output_path: PathBuf,
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Whether a fallback backend/algorithm was used.
    pub used_fallback: bool,
}

/// Error returned by upscaling operations.
#[derive(Debug)]
pub enum UpscaleError {
    /// An I/O operation failed (read/write filesystem).
    Io(String),
    /// Decoding the source image failed.
    Decode(String),
    /// Encoding the output image failed.
    Encode(String),
    /// The requested backend is not available on this system.
    BackendUnavailable(String),
    /// The requested algorithm is not supported by the backend.
    UnsupportedAlgorithm(String),
    /// The fallback backend/algorithm also failed.
    FallbackFailed(String),
    /// The job was cancelled via the shared `UpscaleControl` token before
    /// completing any work, or partway through.
    Cancelled(String),
    /// An unspecified error.
    Unknown(String),
}

impl std::fmt::Display for UpscaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpscaleError::Io(msg) => write!(f, "I/O error: {msg}"),
            UpscaleError::Decode(msg) => write!(f, "decode error: {msg}"),
            UpscaleError::Encode(msg) => write!(f, "encode error: {msg}"),
            UpscaleError::BackendUnavailable(msg) => {
                write!(f, "backend unavailable: {msg}")
            }
            UpscaleError::UnsupportedAlgorithm(msg) => {
                write!(f, "unsupported algorithm: {msg}")
            }
            UpscaleError::FallbackFailed(msg) => write!(f, "fallback failed: {msg}"),
            UpscaleError::Cancelled(msg) => write!(f, "cancelled: {msg}"),
            UpscaleError::Unknown(msg) => write!(f, "upscale error: {msg}"),
        }
    }
}

/// Shared cancellation token for an upscaling job.
///
/// An `Arc<AtomicBool>` lets the caller (commands.rs / frontend) request
/// cancellation from another thread, while the upscaler checks it at
/// coarse-grained checkpoints without altering sampling coordinates.
///
/// # Progress callback
///
/// `on_progress` receives `(completed, total)` where both are actual units of
/// work already finished (source rows consumed / destination rows written,
/// depending on the pass). It is called at most once per 10% step, with a
/// minimum 100 ms gap between reports, except that 100% is always reported
/// on completion. Implementers should not use timers to fake progress.
#[derive(Clone)]
pub struct UpscaleControl {
    /// Shared cancel flag. Set to `true` by the caller to request cancellation.
    pub cancel: Arc<AtomicBool>,
    /// Optional progress callback: `(completed, total)` in actual work units.
    pub on_progress: Option<Arc<dyn Fn(usize, usize) + Send + Sync>>,
}

impl UpscaleControl {
    /// Create a control with cancellation disabled and no progress reporting.
    pub fn none() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            on_progress: None,
        }
    }

    /// Return `true` if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Report progress if enough time has passed since the last report (or
    /// this is 100%). Returns `true` if a report was sent.
    pub fn report_progress(
        &self,
        completed: usize,
        total: usize,
        last_report: &mut Option<std::time::Instant>,
    ) -> bool {
        let done = completed >= total;
        let should_report = done
            || last_report
                .map(|t| t.elapsed() >= std::time::Duration::from_millis(100))
                .unwrap_or(true);
        if should_report {
            if let Some(cb) = &self.on_progress {
                cb(completed, total);
            }
            *last_report = Some(std::time::Instant::now());
            true
        } else {
            false
        }
    }
}

impl Default for UpscaleControl {
    fn default() -> Self {
        Self::none()
    }
}

impl std::error::Error for UpscaleError {}

/// Upscaling backend abstraction.
///
/// Implementations handle a specific backend (CPU or GPU) and expose whether
/// they support a given backend and whether a fallback is available.
pub trait Upscaler: Send + Sync {
    /// Upscale `image` according to `job`, returning the upscaled image.
    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &DynamicImage,
    ) -> Result<DynamicImage, UpscaleError>;

    /// Whether this upscaler supports the given backend.
    fn supports_backend(&self, backend: Backend) -> bool;

    /// Whether this upscaler has a fallback available (e.g. a CPU fallback for
    /// a GPU upscaler).
    fn fallback_available(&self) -> bool;

    /// Upscale `image` according to `job`, honouring the shared cancellation
    /// token and reporting actual completed/total progress via `control`.
    ///
    /// The default implementation delegates to [`upscale`] with a disabled
    /// control, keeping the existing API stable. Override this in concrete
    /// upscalers to add mid-job cancellation checkpoints and progress reports
    /// without changing sampling coordinates.
    fn upscale_with_control(
        &self,
        job: &UpscaleJob,
        image: &DynamicImage,
        control: UpscaleControl,
    ) -> Result<DynamicImage, UpscaleError> {
        let _ = control;
        self.upscale(job, image)
    }
}
