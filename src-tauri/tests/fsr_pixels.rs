//! Real-GPU (DX12) pixel regressions for the FSR 1.0 port.
//!
//! These tests run the actual EASU + RCAS compute pipelines through
//! `GpuUpscaler::upscale` and read the final rgba8unorm output. There is no
//! CPU fallback on this path: a missing adapter or pipeline fails the test
//! instead of skipping, because skipping is what let the corrupted EASU tap
//! addressing (alpha 64..65 instead of 64..255 at 960x540 -> 1920x1080) pass.
//!
//! `src-tauri` is a binary crate, so the domain modules are pulled in with
//! `#[path]`, exactly like `tests/integration_test.rs`.

use std::path::PathBuf;

use image::GenericImageView;

#[path = "../src/cpu.rs"]
mod cpu;
#[path = "../src/gpu.rs"]
mod gpu;
#[path = "../src/upscaler.rs"]
mod upscaler;

use crate::gpu::GpuUpscaler;
use crate::upscaler::{Algorithm, Backend, JobStatus, UpscaleJob, Upscaler};

fn gpu() -> GpuUpscaler {
    match GpuUpscaler::new() {
        Ok(gpu) => gpu,
        Err(e) => panic!(
            "FSR pixel tests require a real DX12 adapter and must not skip or \
             fall back to CPU: {e}"
        ),
    }
}

/// FSR job with dummy paths: `execute_fsr1_0` only uses `image` and the target
/// size, so nothing is read from or written to disk.
fn fsr_job(name: &str, target_w: u32, target_h: u32) -> UpscaleJob {
    UpscaleJob {
        id: name.to_string(),
        input_path: PathBuf::from(name),
        output_path: PathBuf::from(name),
        backend: Backend::Gpu,
        algorithm: Algorithm::Fsr1_0,
        target_width: target_w,
        target_height: target_h,
        jpeg_quality: None,
        status: JobStatus::Running,
    }
}

/// 960x540 -> 1920x1080: the measured failure case. Alpha must ramp 64..255
/// across the output row (the buggy EASU gather base produced 64..65), and the
/// right/bottom edges must carry the source's right/bottom colour.
#[test]
fn fsr_easu_alpha_ramp_and_right_bottom_edges() {
    let gpu = gpu();
    let (sw, sh) = (960u32, 540u32);
    let (dw, dh) = (1920u32, 1080u32);

    // R ramps with x, G ramps with y, B is constant, alpha ramps 64..=255.
    let source = image::RgbaImage::from_fn(sw, sh, |x, y| {
        let r = ((x * 255) / (sw - 1)) as u8;
        let g = ((y * 255) / (sh - 1)) as u8;
        let a = 64 + ((x * 191) / (sw - 1)) as u8;
        image::Rgba([r, g, 128, a])
    });
    let source = image::DynamicImage::ImageRgba8(source);

    let out = gpu
        .upscale(&fsr_job("fsr_pixels_ramp", dw, dh), &source)
        .unwrap_or_else(|e| panic!("FSR upscale failed: {e}"));
    assert_eq!(out.dimensions(), (dw, dh), "FSR output size");
    let rgba = out.to_rgba8();

    // --- alpha ramp -------------------------------------------------------
    let mid_y = dh / 2;
    let a_left = rgba.get_pixel(0, mid_y)[3];
    let a_right = rgba.get_pixel(dw - 1, mid_y)[3];
    assert_eq!(a_left, 64, "left edge alpha must be the source minimum");
    assert!(
        a_right >= 254,
        "right edge alpha {a_right}, expected ~255 (corrupted gather base stops near 64)"
    );

    let row: Vec<u8> = (0..dw).map(|x| rgba.get_pixel(x, mid_y)[3]).collect();
    assert!(
        row.windows(2).all(|w| w[1] >= w[0]),
        "alpha ramp must be non-decreasing left to right"
    );
    let span = a_right as i32 - a_left as i32;
    assert!(span >= 180, "alpha span {span}, expected ~191");
    let a_mid = row[(dw / 2) as usize];
    assert!(
        (145..=175).contains(&a_mid),
        "mid-row alpha {a_mid}, expected ~159 (half of the 64..255 ramp)"
    );

    // --- right/bottom colour ---------------------------------------------
    // Tolerance 12 covers RCAS sharpening on a smooth ramp.
    let check = |x: u32, y: u32, want: [u8; 3], what: &str| {
        let p = rgba.get_pixel(x, y).0;
        for c in 0..3 {
            let d = (p[c] as i32 - want[c] as i32).abs();
            assert!(
                d <= 12,
                "{what} at ({x},{y}) = {p:?}, expected ~{want:?} (channel {c} off by {d})"
            );
        }
    };
    check(0, 0, [0, 0, 128], "top-left");
    check(dw - 1, dh / 2, [255, 128, 128], "right edge");
    check(dw / 2, dh - 1, [128, 255, 128], "bottom edge");
    check(dw - 1, dh - 1, [255, 255, 128], "bottom-right corner");
}

/// A constant image must survive EASU + RCAS unchanged (including alpha), which
/// also pins the RCAS signed denominator: a positive-only clamp changes the
/// limiter sign and shifts flat colours.
#[test]
fn fsr_easu_rcas_constant_image_is_stable() {
    let gpu = gpu();
    let src_px = [90u8, 140, 200, 77];
    let source =
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 48, image::Rgba(src_px)));

    let out = gpu
        .upscale(&fsr_job("fsr_pixels_const", 128, 96), &source)
        .unwrap_or_else(|e| panic!("FSR upscale failed: {e}"));
    assert_eq!(out.dimensions(), (128, 96), "FSR output size");
    let rgba = out.to_rgba8();

    for (x, y, p) in rgba.enumerate_pixels() {
        let q = p.0;
        for c in 0..4 {
            let d = (q[c] as i32 - src_px[c] as i32).abs();
            assert!(
                d <= 1,
                "constant image changed at ({x},{y}): got {q:?}, expected {src_px:?}"
            );
        }
    }
}
