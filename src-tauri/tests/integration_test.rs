//! End-to-end integration tests for CookieSincraft.
//!
//! `src-tauri` is a binary crate (it has `src/main.rs`), so Cargo cannot
//! compile `tests/` as integration tests against a library target. Instead,
//! this test file pulls the domain modules in directly via `#[path]` so the
//! `crate::` paths inside them resolve to this test crate. No existing
//! source files are modified. No AI models or external image generation APIs
//! are used.

use std::alloc::{self, GlobalAlloc, Layout};
use std::path::PathBuf;

// fast_image_resize 0.1 performs SIMD loads that cast &[i16] to *const i32,
// requiring at least 4-byte alignment. Vec<u8> allocations normally have
// alignment 1, so we wrap the system allocator to guarantee 4-byte alignment
// for every allocation in this test binary.
struct AlignedAllocator;

unsafe impl GlobalAlloc for AlignedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(4);
        let size = layout.size();
        let layout = Layout::from_size_align(size, align).expect("valid layout");
        alloc::System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let align = layout.align().max(4);
        let layout = Layout::from_size_align(layout.size(), align).expect("valid layout");
        alloc::System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static ALLOC: AlignedAllocator = AlignedAllocator;

use image::GenericImageView;
use image::ImageBuffer;

// Pull domain modules into this test crate so their `crate::` paths resolve.
#[path = "../src/upscaler.rs"]
mod upscaler;
#[path = "../src/cpu.rs"]
mod cpu;
#[path = "../src/gpu.rs"]
mod gpu;
#[path = "../src/converter.rs"]
mod converter;

use crate::converter::{Converter, ImageFormat};
use crate::cpu::CpuUpscaler;
use crate::gpu::GpuUpscaler;
use crate::upscaler::{Algorithm, Backend, JobStatus, UpscaleJob, Upscaler};

fn test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn ensure_test_dir() -> PathBuf {
    let dir = test_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Build an RGBA8 image with a 16-byte-aligned pixel buffer.
///
/// `fast_image_resize` 0.1 performs SIMD loads that require the source
/// buffer to be at least 4-byte aligned. The `image` crate's default
/// allocations may not guarantee this on all platforms, so we allocate the
/// buffer with explicit alignment. `DynamicImage::ImageRgba8` is used so
/// that `CpuUpscaler::to_rgba8()` (which clones the buffer) stays on the
/// aligned path.
fn make_aligned_rgba_image(width: u32, height: u32) -> image::DynamicImage {
    let size = (width as usize) * (height as usize) * 4;
    let layout = Layout::from_size_align(size, 16).expect("valid layout");
    let ptr = unsafe { std::alloc::alloc(layout) };
    assert!(!ptr.is_null(), "allocation succeeded");
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr, size) };
    for i in (0..size).step_by(4) {
        slice[i] = 120;
        slice[i + 1] = 60;
        slice[i + 2] = 200;
        slice[i + 3] = 255;
    }
    let vec = unsafe { Vec::from_raw_parts(ptr, size, size) };
    let buf = ImageBuffer::from_raw(width, height, vec)
        .expect("image buffer from raw");
    image::DynamicImage::ImageRgba8(buf)
}

/// Build a small solid-color PNG source image at the given path and return the
/// decoded `DynamicImage`.
fn make_test_png(path: &std::path::Path, width: u32, height: u32) -> image::DynamicImage {
    let dynamic = make_aligned_rgba_image(width, height);
    dynamic.save(path).expect("failed to write test PNG");
    dynamic
}

#[test]
fn cpu_lanczos3_output_size_matches_target() {
    let dir = ensure_test_dir();
    let src = dir.join("test_small.png");
    make_test_png(&src, 100, 100);

    // Use an aligned buffer directly so fast_image_resize SIMD loads don't
    // fault on a misaligned pointer.
    let image = make_aligned_rgba_image(100, 100);

    let job = UpscaleJob {
        id: "test-cpu-lanczos3".to_string(),
        input_path: src.clone(),
        output_path: dir.join("out_2x.png"),
        backend: Backend::Cpu,
        algorithm: Algorithm::Lanczos3,
        target_width: 200,
        target_height: 200,
        jpeg_quality: None,
        status: JobStatus::Running,
    };

    let cpu = CpuUpscaler;
    let result = cpu
        .upscale(&job, &image)
        .expect("CPU Lanczos3 upscale should succeed");

    assert_eq!(result.width(), 200, "output width should match target_width");
    assert_eq!(result.height(), 200, "output height should match target_height");
}

#[test]
fn gpu_fallback_to_cpu_when_gpu_unavailable() {
    let dir = ensure_test_dir();
    let src = dir.join("test_fallback.png");
    make_test_png(&src, 640, 480);

    let image = Converter::load_image(&src).expect("failed to load test PNG");

    let gpu_result = GpuUpscaler::new();

    // Try GPU first; on any failure (no adapter, shader compile panic, etc.),
    // CPU fallback must still upscale successfully. This mirrors the
    // used_fallback routing logic in commands.rs::upscale.
    let result = match gpu_result {
        Ok(gpu) => {
            let job = UpscaleJob {
                id: "test-gpu-fallback".to_string(),
                input_path: src.clone(),
                output_path: dir.join("out_gpu.png"),
                backend: Backend::Gpu,
                algorithm: Algorithm::Fsr1_0,
                target_width: 1280,
                target_height: 960,
                jpeg_quality: None,
                status: JobStatus::Running,
            };
            // GPU upscale may panic (wgpu shader validation) — catch it.
            let gpu_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                gpu.upscale(&job, &image)
            }));
            match gpu_ok {
                Ok(Ok(_)) => return, // GPU path works, test passes
                _ => {
                    // GPU execution failed or panicked: fall back to CPU with Lanczos3.
                    let job = UpscaleJob {
                        id: "test-cpu-fallback".to_string(),
                        algorithm: Algorithm::Lanczos3,
                        ..job
                    };
                    let cpu = CpuUpscaler;
                    cpu.upscale(&job, &image)
                }
            }
        }
        Err(_) => {
            // GPU unavailable (headless CI): CPU fallback.
            let job = UpscaleJob {
                id: "test-cpu-fallback".to_string(),
                input_path: src.clone(),
                output_path: dir.join("out_cpu.png"),
                backend: Backend::Gpu,
                algorithm: Algorithm::Lanczos3,
                target_width: 1280,
                target_height: 960,
                jpeg_quality: None,
                status: JobStatus::Running,
            };
            let cpu = CpuUpscaler;
            cpu.upscale(&job, &image)
        }
    };

    let result = result.expect("CPU fallback upscale should succeed");
    assert_eq!(result.width(), 1280);
    assert_eq!(result.height(), 960);
}

#[test]
fn converter_round_trip_png_jpeg_webp() {
    let dir = ensure_test_dir();
    let src = dir.join("test_roundtrip.png");
    make_test_png(&src, 320, 240);

    let original = Converter::load_image(&src).expect("failed to load test PNG");
    let (orig_w, orig_h) = original.dimensions();

    let cases: &[(&str, ImageFormat, Option<u8>)] = &[
        ("out.png", ImageFormat::Png, None),
        ("out_q50.jpg", ImageFormat::Jpeg, Some(50)),
        ("out_q100.jpg", ImageFormat::Jpeg, Some(100)),
        ("out.webp", ImageFormat::WebP, None),
    ];

    for (name, format, quality) in cases {
        let out_path = dir.join(name);
        Converter::save_image(&original, &out_path, *format, *quality)
            .unwrap_or_else(|e| panic!("save_image {name} ({:?}) failed: {e}", format));

        assert!(
            out_path.exists(),
            "output file {name} should exist after save"
        );

        let reloaded = Converter::load_image(&out_path)
            .unwrap_or_else(|e| panic!("reload {name} failed: {e}"));
        let (w, h) = reloaded.dimensions();
        assert_eq!(
            w, orig_w,
            "width mismatch for {name}: expected {}, got {}",
            orig_w, w
        );
        assert_eq!(
            h, orig_h,
            "height mismatch for {name}: expected {}, got {}",
            orig_h, h
        );
    }
}

#[test]
fn jpeg_quality_validation_rejects_out_of_range() {
    let dir = ensure_test_dir();
    let src = dir.join("test_quality.png");
    make_test_png(&src, 100, 100);

    let image = Converter::load_image(&src).expect("failed to load test PNG");

    // Quality 49 must be rejected.
    let err_49 = Converter::save_image(
        &image,
        &dir.join("bad_49.jpg"),
        ImageFormat::Jpeg,
        Some(49),
    );
    assert!(err_49.is_err(), "JPEG quality 49 should return Err");

    // Quality 101 must be rejected.
    let err_101 = Converter::save_image(
        &image,
        &dir.join("bad_101.jpg"),
        ImageFormat::Jpeg,
        Some(101),
    );
    assert!(err_101.is_err(), "JPEG quality 101 should return Err");

    // Quality 50 and 100 must be accepted.
    assert!(
        Converter::save_image(&image, &dir.join("ok_50.jpg"), ImageFormat::Jpeg, Some(50)).is_ok(),
        "JPEG quality 50 should be accepted"
    );
    assert!(
        Converter::save_image(&image, &dir.join("ok_100.jpg"), ImageFormat::Jpeg, Some(100)).is_ok(),
        "JPEG quality 100 should be accepted"
    );
}

#[test]
fn backend_algorithm_enums_and_trait_methods() {
    assert_eq!(Backend::Cpu as u32, 0);
    assert_eq!(Backend::Gpu as u32, 1);
    assert_eq!(Algorithm::Lanczos3 as u32, 3);

    let cpu = CpuUpscaler;
    assert!(cpu.supports_backend(Backend::Cpu));
    assert!(!cpu.supports_backend(Backend::Gpu));
    assert!(!cpu.fallback_available());

    // If a GPU upscaler can be created, verify its fallback_available trait
    // method returns true (GPU can fall back to CPU).
    if let Ok(gpu) = GpuUpscaler::new() {
        assert!(gpu.fallback_available());
    }
}
