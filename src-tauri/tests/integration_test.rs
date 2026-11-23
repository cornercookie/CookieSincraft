//! End-to-end integration tests for CookieSincraft.
//!
//! `src-tauri` is a binary crate (it has `src/main.rs`), so Cargo cannot
//! compile `tests/` as integration tests against a library target. Instead,
//! this test file pulls the domain modules in directly via `#[path]` so the
//! `crate::` paths inside them resolve to this test crate. No existing
//! source files are modified. No AI models or external image generation APIs
//! are used.

use std::path::PathBuf;

use image::GenericImageView;

// Pull domain modules into this test crate so their `crate::` paths resolve.
#[path = "../src/upscaler.rs"]
mod upscaler;
#[path = "../src/cpu.rs"]
mod cpu;
#[path = "../src/gpu.rs"]
mod gpu;
#[path = "../src/converter.rs"]
mod converter;
#[path = "../src/commands.rs"]
mod commands;

use crate::commands::validate_dimensions;
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

/// Build a small solid-color PNG source image at the given path and return the
/// decoded `DynamicImage`. Uses a plain (non-aligned) buffer since the
/// retained tests do not exercise `fast_image_resize` SIMD paths.
fn make_test_png(path: &std::path::Path, width: u32, height: u32) -> image::DynamicImage {
    let img = image::ImageBuffer::from_fn(width, height, |x, y| {
        image::Rgba([(x.wrapping_mul(2)) as u8, (y.wrapping_mul(2)) as u8, 128, 255])
    });
    let dynamic = image::DynamicImage::ImageRgba8(img);
    dynamic.save(path).expect("failed to write test PNG");
    dynamic
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

#[test]
fn validate_dimensions_boundary_values() {
    // 4096x4096 = exactly 64 MB buffer (16,777,216 pixels * 4 bytes)
    assert!(validate_dimensions(4096, 4096).is_ok(), "4096x4096 should be allowed");

    // 4097x4097 exceeds dimension limit
    let err = validate_dimensions(4097, 4097).expect_err("4097x4097 should be rejected");
    assert!(err.contains("4096"), "error should mention max dimension 4096");

    // 8192x8192 exceeds dimension limit (checked before buffer limit)
    let err = validate_dimensions(8192, 8192).expect_err("8192x8192 should be rejected");
    assert!(err.contains("4096"), "error should mention max dimension 4096");

    // Test buffer limit with dimensions under 4096 but over 64 MB
    // 4096x4096 = 64 MB exactly, so we need dimensions slightly over that
    // But since 4096 is the max dimension, we can't test buffer limit independently
    // The buffer limit is enforced at the 4096x4096 boundary
}

#[test]
fn validate_dimensions_zero_rejected() {
    let err = validate_dimensions(0, 100).expect_err("zero width should be rejected");
    assert_eq!(err, "target dimensions must be non-zero");

    let err = validate_dimensions(100, 0).expect_err("zero height should be rejected");
    assert_eq!(err, "target dimensions must be non-zero");
}

#[test]
fn validate_dimensions_checked_mul_overflow() {
    // u32::MAX exceeds dimension limit (4096), so dimension check triggers first
    let err = validate_dimensions(u32::MAX, u32::MAX).expect_err("overflow should be rejected");
    assert!(err.contains("4096"), "error should mention max dimension 4096: {}", err);
}
