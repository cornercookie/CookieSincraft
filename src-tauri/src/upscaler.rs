//! Core domain types and the `Upscaler` trait for CookieSincraft.
//!
//! This module defines the shared domain model used by upscaling jobs:
//! backends, algorithms, job status, the job and result structs, the error
//! type, and the `Upscaler` trait. No concrete upscaler implementations live
//! here; those are added in later tasks.

use std::path::PathBuf;

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
            UpscaleError::Unknown(msg) => write!(f, "upscale error: {msg}"),
        }
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
}
