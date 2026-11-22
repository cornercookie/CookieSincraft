//! Tauri command layer for CookieSincraft.
//!
//! Bridges frontend IPC (`load_image`, `upscale`, `cancel_job`,
//! `export_image`) to the domain modules (`Converter`, `Upscaler`,
//! `CpuUpscaler`, `GpuUpscaler`). All command functions return
//! `Result<T, String>` so errors are surfaced to the frontend without
//! panicking.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use image::GenericImageView;
use serde::{Deserialize, Serialize};

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
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Monotonic counter used to mint job ids without adding a uuid dependency.
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
        "fsr1_0" | "fsr-1-0" | "fsr1" => Ok(Algorithm::Fsr1_0),
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

/// Best-effort format hint derived from the file extension.
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

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Load an image and return its metadata (width, height, format hint) plus a
/// base64-encoded PNG preview for the frontend to display directly.
#[tauri::command]
pub fn load_image(path: String) -> Result<ImageMeta, String> {
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

/// Convert a `DynamicImage` to a base64-encoded PNG data URL, downscaled to
/// fit within 800px so the frontend preview stays fast.
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
#[tauri::command]
pub fn select_image() -> Result<Option<String>, String> {
    let selected = FileDialog::new()
        .add_filter(
            "Image",
            &["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "bmp", "ico", "pnm", "pbm", "pgm", "ppm", "pam"],
        )
        .pick_file();
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
}

/// Open a native save file dialog and return the selected path.
#[tauri::command]
pub fn select_save_path(default_name: String, filters: Vec<Vec<String>>) -> Result<Option<String>, String> {
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
    Ok(selected.map(|p| p.to_string_lossy().to_string()))
}

/// Upscale an image according to `job`.
///
/// GPU backend is attempted first when requested; on any GPU error the job is
/// retried on the CPU and `used_fallback` is set to `true`.
#[tauri::command]
pub fn upscale(job: UpscaleJobDto) -> Result<UpscaleResultDto, String> {
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

    let image = Converter::load_image(job.input_path.as_path()).map_err(|e| e.to_string())?;

    let (upscaled, used_fallback) = match backend {
        Backend::Gpu => match GpuUpscaler::new().and_then(|gpu| gpu.upscale(&job, &image)) {
            Ok(img) => (img, false),
            Err(gpu_err) => {
                // GPU unavailable or execution failed: fall back to CPU.
                let cpu = CpuUpscaler;
                let img = cpu
                    .upscale(&job, &image)
                    .map_err(|e| format!("GPU upscaler failed: {gpu_err}; CPU fallback also failed: {e}"))?;
                (img, true)
            }
        },
        Backend::Cpu => {
            let cpu = CpuUpscaler;
            let img = cpu.upscale(&job, &image).map_err(|e| e.to_string())?;
            (img, false)
        }
    };

    let output_format = parse_image_format(&format_from_path(job.output_path.as_path()))?;
    Converter::save_image(
        &upscaled,
        job.output_path.as_path(),
        output_format,
        job.jpeg_quality,
    )
    .map_err(|e| e.to_string())?;

    let (width, height) = upscaled.dimensions();

    Ok(UpscaleResultDto {
        job_id,
        output_path: job.output_path.to_string_lossy().to_string(),
        width,
        height,
        used_fallback,
    })
}

/// Cancel an in-flight job (stub: real cancellation not implemented yet).
#[tauri::command]
pub fn cancel_job(_id: String) -> Result<(), String> {
    // TODO: wire up job cancellation once a job queue exists.
    Ok(())
}

/// Export (re-encode) an already-upscaled image to the user-chosen path.
#[tauri::command]
pub fn export_image(path: String, output_path: String, format: String, quality: u8) -> Result<(), String> {
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
pub fn convert_image(input_path: String, output_path: String, format: String, quality: u8) -> Result<(), String> {
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

    /// Exercise the `convert_image` / `export_image` commands end-to-end
    /// with a realistic (large, RGBA) source, for every format the UI offers.
    #[test]
    fn convert_and_export_commands_write_every_ui_format() {
        let tmp = std::env::temp_dir().join("cookiesincraft_cmd_convert_test");
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        image::DynamicImage::ImageRgba8(image::ImageBuffer::from_fn(1200, 800, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 200])
        }))
        .save(&src)
        .expect("write source");
        let src = src.to_string_lossy().to_string();

        for ext in ["png", "jpeg", "webp", "gif", "tiff", "bmp", "ico", "pnm"] {
            let out = tmp.join(format!("converted.{ext}"));
            let out_s = out.to_string_lossy().to_string();
            convert_image(src.clone(), out_s.clone(), ext.to_string(), 85)
                .unwrap_or_else(|e| panic!("convert to {ext} failed: {e}"));
            let meta = load_image(out_s.clone())
                .unwrap_or_else(|e| panic!("reload {ext} failed: {e}"));
            assert!(meta.width > 0 && meta.height > 0, "{ext} empty");

            // Export must write to the chosen path and leave the source intact.
            let exported = tmp.join(format!("exported.{ext}"));
            export_image(out_s.clone(), exported.to_string_lossy().to_string(), ext.to_string(), 85)
                .unwrap_or_else(|e| panic!("export {ext} failed: {e}"));
            assert!(exported.exists(), "export {ext} did not write the chosen path");
        }
    }
}
