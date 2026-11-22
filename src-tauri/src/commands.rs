//! Tauri command layer for CookieSincraft.
//!
//! Bridges frontend IPC (`load_image`, `upscale`, `cancel_job`,
//! `export_image`) to the domain modules (`Converter`, `Upscaler`,
//! `CpuUpscaler`, `GpuUpscaler`). All command functions return
//! `Result<T, String>` so errors are surfaced to the frontend without
//! panicking.

use std::collections::HashSet;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};

use base64::Engine;
use image::GenericImageView;
use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, Manager, Runtime, State};

use crate::converter::{Converter, ImageFormat};
use crate::cpu::CpuUpscaler;
use crate::gpu::GpuUpscaler;
use crate::upscaler::{Algorithm, Backend, JobStatus, UpscaleJob, Upscaler};
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
}

/// Progress event sent via Channel during upscale.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum UpscaleProgress {
    Started { job_id: String },
    Progress { stage: String, percent: u8, message: String },
    Completed { job_id: String, result: UpscaleResultDto },
    Cancelled { job_id: String },
    Failed { job_id: String, error: String },
}

// ---------------------------------------------------------------------------
// App State (path allowlist + job tracking)
// ---------------------------------------------------------------------------

/// Tracks paths allowed by file dialogs and running jobs for cancellation.
pub struct AppState {
    /// Paths selected via file dialogs (read + write allowed)
    allowed_paths: Mutex<HashSet<String>>,
    /// Running jobs: job_id -> cancel flag
    running_jobs: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            allowed_paths: Mutex::new(HashSet::new()),
            running_jobs: Mutex::new(HashSet::new()),
        }
    }

    pub fn allow_read(&self, path: &str) {
        self.allowed_paths.lock().unwrap().insert(path.to_string());
    }

    pub fn allow_write(&self, path: &str) {
        self.allowed_paths.lock().unwrap().insert(path.to_string());
    }

    pub fn check_read(&self, path: &str) -> Result<(), String> {
        let paths = self.allowed_paths.lock().unwrap();
        if paths.contains(path) {
            Ok(())
        } else {
            Err(format!("access denied: path '{}' was not selected through a file dialog", path))
        }
    }

    pub fn check_write(&self, path: &str) -> Result<(), String> {
        let paths = self.allowed_paths.lock().unwrap();
        if paths.contains(path) {
            Ok(())
        } else {
            Err(format!("access denied: path '{}' was not selected through a file dialog", path))
        }
    }

    pub fn start_job(&self, job_id: &str) -> bool {
        let mut jobs = self.running_jobs.lock().unwrap();
        if jobs.contains(job_id) {
            false
        } else {
            jobs.insert(job_id.to_string());
            true
        }
    }

    pub fn cancel_job(&self, job_id: &str) -> Result<(), String> {
        let mut jobs = self.running_jobs.lock().unwrap();
        if jobs.remove(job_id) {
            Ok(())
        } else {
            Err("job not found".to_string())
        }
    }

    pub fn is_running(&self, job_id: &str) -> bool {
        self.running_jobs.lock().unwrap().contains(job_id)
    }

    pub fn finish_job(&self, job_id: &str) {
        self.running_jobs.lock().unwrap().remove(job_id);
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

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load an image and return its metadata plus a base64-encoded PNG preview.
#[tauri::command]
pub fn load_image(
    path: String,
    state: State<'_, AppState>,
) -> Result<ImageMeta, String> {
    state.check_read(&path)?;
    let image = Converter::load_image(PathBuf::from(&path).as_path())
        .map_err(|e| e.to_string())?;
    let (width, height) = image.dimensions();
    let format = format_from_path(std::path::Path::new(&path));
    let preview = image_to_base64_preview(&image);
    Ok(ImageMeta {
        width,
        height,
        format,
        preview,
    })
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
            &["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "bmp", "ico", "pnm", "pbm", "pgm", "ppm", "pam"],
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
    eprintln!("select_save_path called: default_name={}, filters={:?}", default_name, filters);
    let mut dialog = FileDialog::new().set_file_name(&default_name);
    for filter in filters {
        if filter.len() >= 2 {
            let name = &filter[0];
            let exts: Vec<&str> = filter[1..].iter().map(|s| s.as_str()).collect();
            dialog = dialog.add_filter(name, &exts);
        }
    }
    let selected = dialog.save_file();
    eprintln!("select_save_path returned: {:?}", selected);
    if let Some(ref path) = selected {
        let path_str = path.to_string_lossy().to_string();
        state.allow_write(&path_str);
    }
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
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

    let input_path = PathBuf::from(&job.input_path);
    let output_path = PathBuf::from(&job.output_path);
    let job_id = next_job_id();
    let job = build_job(
        &job_id,
        &job,
        input_path.clone(),
        output_path.clone(),
        backend,
        algorithm,
    );

    // Check path permissions
    state.check_read(&job.input_path.to_string_lossy())?;
    state.check_write(&job.output_path.to_string_lossy())?;

    // Register job
    if !state.start_job(&job_id) {
        return Err("another upscale job is running".to_string());
    }
    send_progress(&on_event, UpscaleProgress::Started { job_id: job_id.clone() });

    let image = Converter::load_image(job.input_path.as_path()).map_err(|e| e.to_string())?;

    // Send loading progress
    send_progress(&on_event, UpscaleProgress::Progress {
        stage: "loading".to_string(),
        percent: 5,
        message: "Image loaded, starting upscale".to_string(),
    });

    // Clone channel for use in background task
    let on_event_clone = on_event.clone();

    // Run upscale in background thread to keep UI responsive
    let job_clone = job.clone();
    let job_id_clone = job_id.clone();
    let app_clone = app.clone();

    let result = tokio::task::spawn_blocking(move || {
        // Access state via AppHandle (which is 'static)
        let state_clone = app_clone.state::<AppState>().inner();
        
        // Check for cancellation before starting
        if state_clone.running_jobs.lock().unwrap().contains(&job_id_clone) == false {
            return Err("job cancelled before start".to_string());
        }

        send_progress(&on_event_clone, UpscaleProgress::Progress {
            stage: "upscaling".to_string(),
            percent: 10,
            message: "Upscaling...".to_string(),
        });

        let (upscaled, used_fallback, fallback_reason) = match backend {
            Backend::Gpu => match GpuUpscaler::new().and_then(|gpu| gpu.upscale(&job_clone, &image)) {
                Ok(img) => (img, false, None),
                Err(gpu_err) => {
                    // GPU unavailable or execution failed: fall back to CPU with Lanczos3.
                    let fallback_reason = gpu_err.to_string();
                    send_progress(&on_event_clone, UpscaleProgress::Progress {
                        stage: "fallback".to_string(),
                        percent: 50,
                        message: format!("GPU failed, falling back to CPU: {}", fallback_reason),
                    });
                    let mut cpu_job = job_clone.clone();
                    cpu_job.algorithm = Algorithm::Lanczos3;
                    let cpu = CpuUpscaler;
                    let img = cpu
                        .upscale(&cpu_job, &image)
                        .map_err(|e| format!("GPU upscaler failed: {gpu_err}; CPU fallback also failed: {e}"))?;
                    (img, true, Some(fallback_reason))
                }
            },
            Backend::Cpu => {
                let cpu = CpuUpscaler;
                let img = cpu.upscale(&job_clone, &image).map_err(|e| e.to_string())?;
                (img, false, None)
            }
        };

        // Check for cancellation after upscale
        if state_clone.running_jobs.lock().unwrap().contains(&job_id_clone) == false {
            return Err("job cancelled during upscale".to_string());
        }

        send_progress(&on_event_clone, UpscaleProgress::Progress {
            stage: "saving".to_string(),
            percent: 90,
            message: "Saving output...".to_string(),
        });

        let output_format = parse_image_format(&format_from_path(job_clone.output_path.as_path()))?;
        Converter::save_image(
            &upscaled,
            job_clone.output_path.as_path(),
            output_format,
            job_clone.jpeg_quality,
        )
        .map_err(|e| e.to_string())?;

        // Allow reading the output for export
        state_clone.allow_read(&job_clone.output_path.to_string_lossy());

        let (width, height) = upscaled.dimensions();

        Ok(UpscaleResultDto {
            job_id: job_id_clone,
            output_path: job_clone.output_path.to_string_lossy().to_string(),
            width,
            height,
            used_fallback,
            fallback_reason,
        })
    }).await;

    // Handle join error
    let result = match result {
        Ok(r) => r,
        Err(e) => Err(format!("upscale task panicked: {e}")),
    };

    // Finish job
    state.finish_job(&job_id);

    match result {
        Ok(dto) => {
            send_progress(&on_event, UpscaleProgress::Completed { job_id, result: dto.clone() });
            Ok(dto)
        }
        Err(e) => {
            send_progress(&on_event, UpscaleProgress::Failed { job_id, error: e.clone() });
            Err(e)
        }
    }
}

/// Cancel an in-flight upscale job.
#[tauri::command]
pub fn cancel_job(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.cancel_job(&id)
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
    let format = parse_image_format(&format)?;
    let image = Converter::load_image(PathBuf::from(&path).as_path())
        .map_err(|e| e.to_string())?;
    Converter::save_image(
        &image,
        PathBuf::from(&output_path).as_path(),
        format,
        Some(quality),
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
    let format = parse_image_format(&format)?;
    let image = Converter::load_image(PathBuf::from(&input_path).as_path())
        .map_err(|e| e.to_string())?;
    Converter::save_image(
        &image,
        PathBuf::from(&output_path).as_path(),
        format,
        Some(quality),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test path allowlist logic
    #[test]
    fn test_app_state_path_allowlist() {
        let state = AppState::new();
        state.allow_read("/test/path.png");
        assert!(state.check_read("/test/path.png").is_ok());
        assert!(state.check_read("/other/path.png").is_err());

        state.allow_write("/test/out.png");
        assert!(state.check_write("/test/out.png").is_ok());
        assert!(state.check_write("/other/out.png").is_err());
    }

    /// Test job tracking
    #[test]
    fn test_app_state_job_tracking() {
        let state = AppState::new();
        assert!(state.start_job("job-1"));
        assert!(!state.start_job("job-1")); // duplicate
        assert!(state.is_running("job-1"));
        assert!(state.cancel_job("job-1").is_ok());
        assert!(!state.is_running("job-1"));
        assert!(state.cancel_job("job-1").is_err()); // not found
    }
}