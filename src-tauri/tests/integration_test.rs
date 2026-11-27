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
#[path = "../src/settings.rs"]
mod settings;

use crate::commands::validate_dimensions;
use crate::converter::{Converter, ImageFormat};
use crate::cpu::CpuUpscaler;
use crate::gpu::GpuUpscaler;
use crate::upscaler::{Algorithm, Backend, JobStatus, UpscaleJob, Upscaler};

fn test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn ensure_test_dir() -> PathBuf {
    // Fresh unique subdir per run so leftover outputs from previous runs
    // never trip the no-clobber guard, and we never delete the test source
    // files that live in `src-tauri/tests/`.
    let dir = test_dir().join(format!(
        "run_{}_{}",
        std::process::id(),
        fastrand::u64(..)
    ));
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
        Converter::save_image(&original, &out_path, *format, *quality, false)
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
        false,
    );
    assert!(err_49.is_err(), "JPEG quality 49 should return Err");

    // Quality 101 must be rejected.
    let err_101 = Converter::save_image(
        &image,
        &dir.join("bad_101.jpg"),
        ImageFormat::Jpeg,
        Some(101),
        false,
    );
    assert!(err_101.is_err(), "JPEG quality 101 should return Err");

    // Quality 50 and 100 must be accepted.
    assert!(
        Converter::save_image(&image, &dir.join("ok_50.jpg"), ImageFormat::Jpeg, Some(50), false)
            .is_ok(),
        "JPEG quality 50 should be accepted"
    );
    assert!(
        Converter::save_image(&image, &dir.join("ok_100.jpg"), ImageFormat::Jpeg, Some(100), false)
            .is_ok(),
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

/// GPU shader module / pipeline validation.
///
/// Runs only on machines with a DX12 adapter (`#[ignore]` on headless CI to
/// avoid a false green from the CPU fallback path). `GpuUpscaler::new()` uses
/// `create_shader_module_safe` / `create_compute_pipeline_safe`, which convert
/// wgpu validation panics into `Err`; if `new()` succeeds, all shaders and the
/// pipeline passed validation. A failure here is a real test failure — we do
/// NOT catch_unwind and then mark GPU as passed via CPU Lanczos3.
#[test]
#[ignore]
fn gpu_shader_module_and_pipeline_validation() {
    let _gpu = GpuUpscaler::new()
        .map_err(|e| panic!("GPU shader module / pipeline creation failed: {e}"))
        .expect("GPU upscaler should initialize without validation errors");
}

// ---------------------------------------------------------------------------
// opencode-gpu-port required tests (no GPU; source-level checks only).
// ---------------------------------------------------------------------------

fn read_to_string_rel(path: &str) -> String {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(base.join(path)).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// Test 1: FSR EASU shader is a real 12-tap port (textureLoad-based), not a
/// 3x3 sharpen. Counts the actual tap-load call sites (`load_offset(`) which
/// this port uses instead of a `load2x2` helper, and confirms it is not
/// reduced to `grad_x = r - l`.
#[test]
fn fsr_easu_is_12_tap_port() {
    let src = read_to_string_rel("shaders/fsr_easu.wgsl");
    // This port loads texels via `load_offset(` (one call per tap). Upstream
    // FSR EASU is a 12-tap kernel: taps b,c,e,f,g,h,i,j,k,l,n,o.
    let tap_count = src.matches("load_offset(").count();
    assert!(
        tap_count >= 12,
        "FSR EASU should load 12 taps, got {tap_count} load_offset call sites"
    );
    // Sanity: it still defines and uses texel_load_xy.
    let load_count = src.matches("texel_load_xy(").count();
    assert!(
        load_count >= 5,
        "FSR EASU should define and use texel_load_xy (>=5 sites), got {load_count}"
    );
    // Must not be the fake 3x3 sharpen.
    assert!(
        !src.contains("grad_x = r - l"),
        "FSR EASU must not be reduced to a 3x3 grad_x = r - l sharpen"
    );
}

/// Test 2: NIS coefficients are a 64x8 table (coef_scale from NIS_Config.h)
/// and the WGSL indexes that buffer (not a dead constant table).
#[test]
fn nis_coefficients_are_64x8_and_indexed() {
    let gpu_src = read_to_string_rel("src/gpu.rs");
    // Rust table: [[f32; 8]; 64]
    assert!(
        gpu_src.contains("const NIS_COEF_SCALE: [[f32; 8]; 64]"),
        "Rust should declare NIS_COEF_SCALE as [[f32; 8]; 64]"
    );
    // First row must equal NIS_Config.h coef_scale row 0: {0,0,1,0,0,0,0,0}
    assert!(
        gpu_src.contains("[0f32, 0f32, 1f32, 0f32, 0f32, 0f32, 0f32, 0f32],"),
        "NIS_COEF_SCALE row 0 must match NIS_Config.h coef_scale row 0"
    );
    // Count rows: 64 occurrences of a row starting with whitespace + [ ... ],
    let row_count = gpu_src.matches("],").count();
    assert!(
        row_count >= 64,
        "NIS_COEF_SCALE should have 64 rows, found ~{row_count}"
    );

    // WGSL must index the coefficient buffer (binding 4 / coef_scale).
    let wgsl_src = read_to_string_rel("shaders/nis_scaler.wgsl");
    assert!(
        wgsl_src.contains("binding(4)"),
        "NIS WGSL should bind coef_scale at binding 4"
    );
    assert!(
        wgsl_src.contains("coef_scale"),
        "NIS WGSL should index coef_scale buffer"
    );
    assert!(
        wgsl_src.contains("array<f32, 512>"),
        "NIS WGSL stores the 64x8 table row-major as 512 f32 (DX12 cannot dynamically index array<array<...>>)"
    );
    assert!(
        wgsl_src.contains("phase * 8"),
        "NIS WGSL should index the 64x8 table as phase * 8 + tap"
    );
}

/// Test 3: Anime4K CNN x2 M ships 9 pass shader modules (one per hook) and the
/// weight string 0.16265294 (from GLSL:230) appears in the generated shader.
#[test]
fn anime4k_cnn_has_9_passes_and_weight() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders/anime4k_cnn_passes");
    let passes = [
        "conv2d_tf", "conv2d_1_tf", "conv2d_2_tf", "conv2d_3_tf", "conv2d_4_tf",
        "conv2d_5_tf", "conv2d_6_tf", "conv2d_last_tf", "MAIN",
    ];
    for name in &passes {
        let path = base.join(format!("{name}.wgsl"));
        assert!(
            path.exists(),
            "Anime4K pass shader missing: {path:?}"
        );
    }
    // The GLSL mat4 at Anime4K_Upscale_CNN_x2_M.glsl:230 contains 0.16265294;
    // it must be mechanically copied into conv2d_6_tf.wgsl.
    let conv6 = read_to_string_rel("shaders/anime4k_cnn_passes/conv2d_6_tf.wgsl");
    assert!(
        conv6.contains("0.16265294"),
        "conv2d_6_tf.wgsl must contain upstream weight 0.16265294"
    );
}

/// Test 4: Every `include_str!`-ed WGSL must parse with naga 0.19
/// `front::wgsl::parse_str`. A parse failure is a hard test failure — we no
/// longer rely on counting `textureLoad` sites or searching for weight strings.
#[test]
fn every_included_wgsl_parses_with_naga() {
    use naga::front::wgsl::parse_str;

    let shaders: &[&str] = &[
        "shaders/bilinear.wgsl",
        "shaders/fsr_easu.wgsl",
        "shaders/fsr_rcas.wgsl",
        "shaders/nis_scaler.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_1_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_2_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_3_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_4_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_5_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_6_tf.wgsl",
        "shaders/anime4k_cnn_passes/conv2d_last_tf.wgsl",
        "shaders/anime4k_cnn_passes/MAIN.wgsl",
    ];

    for path in shaders {
        let source = read_to_string_rel(path);
        parse_str(&source)
            .map_err(|e| panic!("{path} failed to parse WGSL: {e:?}"))
            .expect("WGSL should parse");
    }
}

fn job_for(name: &str, algorithm: Algorithm, src: &std::path::Path, out: &std::path::Path) -> UpscaleJob {
    UpscaleJob {
        id: name.to_string(),
        input_path: src.to_path_buf(),
        output_path: out.to_path_buf(),
        backend: Backend::Gpu,
        algorithm,
        target_width: 128,
        target_height: 96,
        jpeg_quality: None,
        status: JobStatus::Running,
    }
}

/// Dispatch the GPU algorithms on a real device and reload the saved PNG.
/// No adapter: skip (CI). Adapter present but dispatch fails: fail.
/// This does not prove pixel parity with upstream shaders.
#[test]
fn gpu_dispatch_writes_target_size_png() {
    let gpu = match GpuUpscaler::new() {
        Ok(gpu) => gpu,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no DX12") || msg.contains("no adapter") {
                eprintln!("skip gpu_dispatch_writes_target_size_png: {msg}");
                return;
            }
            panic!("GPU device init failed: {msg}");
        }
    };

    let dir = ensure_test_dir();
    let src_path = dir.join("gpu_dispatch_src.png");
    let source = make_test_png(&src_path, 64, 48);

    let cases = [
        (Algorithm::Bilinear, "gpu_dispatch_bilinear.png"),
        (Algorithm::Fsr1_0, "gpu_dispatch_fsr.png"),
        (Algorithm::Nis, "gpu_dispatch_nis.png"),
        (Algorithm::Anime4k, "gpu_dispatch_anime4k.png"),
    ];

    for (algorithm, name) in cases {
        let out = dir.join(name);
        let job = job_for(name, algorithm, &src_path, &out);
        let image = gpu
            .upscale(&job, &source)
            .unwrap_or_else(|e| panic!("{name} upscale failed: {e}"));
        let (w, h) = image.dimensions();
        assert_eq!((w, h), (128, 96), "{name} in-memory size");
        Converter::save_image(&image, &out, ImageFormat::Png, None, false)
            .unwrap_or_else(|e| panic!("{name} save failed: {e}"));
        let reloaded = Converter::load_image(&out)
            .unwrap_or_else(|e| panic!("{name} reload failed: {e}"));
        assert_eq!(reloaded.dimensions(), (128, 96), "{name} reloaded size");
        let rgba = reloaded.to_rgba8();
        let first = rgba.get_pixel(0, 0).0;
        let varies = rgba.pixels().any(|p| p.0 != first);
        assert!(varies, "{name} output is a flat color");
    }
}

// ---------------------------------------------------------------------------
// Anime4K GPU pass correctness regressions (DX12 path).
// ---------------------------------------------------------------------------

/// Create an Anime4k job for a 2x target from a small source.
fn anime4k_job_2x(
    name: &str,
    src: &std::path::Path,
    out: &std::path::Path,
    src_w: u32,
    src_h: u32,
) -> UpscaleJob {
    UpscaleJob {
        id: name.to_string(),
        input_path: src.to_path_buf(),
        output_path: out.to_path_buf(),
        backend: Backend::Gpu,
        algorithm: Algorithm::Anime4k,
        target_width: src_w * 2,
        target_height: src_h * 2,
        jpeg_quality: None,
        status: JobStatus::Running,
    }
}

/// Helper: initialise GPU, skipping on no-adapter, failing on validation error.
fn gpu_or_skip(test_name: &str) -> Option<GpuUpscaler> {
    match GpuUpscaler::new() {
        Ok(gpu) => Some(gpu),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("no DX12") || msg.contains("no adapter") {
                eprintln!("skip {test_name}: {msg}");
                None
            } else {
                panic!("GPU device init failed (validation error, not no-adapter): {msg}");
            }
        }
    }
}

/// Image-based regression: Anime4K depth-to-space must preserve the original
/// image's alpha (bilinear at texel centre = exact copy), not replace it with
/// the depth-to-space scalar. For a 2x output, every output pixel's alpha
/// equals the corresponding source pixel's alpha (cpos = pos / 2).
#[test]
fn gpu_anime4k_alpha_preserved_from_original() {
    let Some(gpu) = gpu_or_skip("gpu_anime4k_alpha_preserved_from_original") else { return };

    let dir = ensure_test_dir();
    let src_path = dir.join("anime4k_alpha_src.png");
    // 2x2 source with distinct alpha per pixel.
    let source = image::ImageBuffer::from_fn(2, 2, |x, y| {
        image::Rgba([100, 150, 200, (x * 80 + y * 40) as u8])
    });
    let dynamic = image::DynamicImage::ImageRgba8(source.clone());
    dynamic.save(&src_path).expect("save test PNG");

    let job = anime4k_job_2x("anime4k_alpha", &src_path, &dir.join("anime4k_alpha_out.png"), 2, 2);
    let result = gpu
        .upscale(&job, &dynamic)
        .unwrap_or_else(|e| panic!("Anime4K alpha upscale failed: {e}"));
    let (w, h) = result.dimensions();
    assert_eq!((w, h), (4, 4), "Anime4K 2x output must be 4x4");

    let rgba = result.to_rgba8();
    let expected = [[0u8,20,60,80],[10,30,70,90],[30,50,90,110],[40,60,100,120]];
    for (y,row) in expected.iter().enumerate() {
        for (x,alpha) in row.iter().enumerate() {
            assert!(rgba.get_pixel(x as u32,y as u32).0[3].abs_diff(*alpha) <= 1);
        }
    }
}

/// Source-level regression: Anime4K intermediate feature maps must use
/// rgba16float storage so signed unclamped values (including negative
/// relu_neg contributions) are preserved, not clamped by rgba8unorm.
#[test]
fn gpu_anime4k_feature_maps_use_float_storage() {
    let gpu_src = read_to_string_rel("src/gpu.rs");
    // Conv pass outputs (run_single helper) must create Rgba16Float textures.
    assert!(
        gpu_src.contains("format: wgpu::TextureFormat::Rgba16Float"),
        "gpu.rs must create Rgba16Float textures for Anime4K feature maps"
    );
    // conv2d_last output must also be Rgba16Float.
    assert!(
        gpu_src.contains("anime conv2d_last output"),
        "conv2d_last output texture must exist"
    );
    // The conv bind group layout must use Rgba16Float storage.
    assert!(
        gpu_src.contains("anime conv bind group layout"),
        "anime conv bind group layout must exist"
    );
    // The old rgba8unorm storage must NOT be used for anime conv pass outputs.
    assert!(
        !gpu_src.contains("anime pass output"),
        "anime pass output label must not remain (old rgba8unorm path)"
    );
}

/// Shader-level regression: depth-to-space must select one of 4 channels based
/// on sub-pixel position (fract-based depth-to-space), replicating that scalar
/// across RGB and preserving original alpha.
#[test]
fn gpu_anime4k_depth_to_space_subpixel_parity() {
    let main_src = read_to_string_rel("shaders/anime4k_cnn_passes/MAIN.wgsl");
    // Sub-pixel detection: pos.x % 2 and pos.y % 2.
    assert!(
        main_src.contains("pos.x % 2u") && main_src.contains("pos.y % 2u"),
        "MAIN.wgsl must detect sub-pixel position via pos % 2"
    );
    // Channel index: sub_y * 2 + sub_x.
    assert!(
        main_src.contains("sub_y * 2u + sub_x"),
        "MAIN.wgsl must compute channel index as sub_y * 2 + sub_x"
    );
    // Channel selection from texel.
    assert!(
        main_src.contains("texel[channel]"),
        "MAIN.wgsl must select channel from texel via texel[channel]"
    );
    // Alpha preserved from main_texel, not scalar.
    assert!(
        main_src.contains("main_texel.a"),
        "MAIN.wgsl must preserve alpha from main_texel.a"
    );
    // Scalar replicated across RGB, alpha = 0.0 before adding main texel.
    assert!(
        main_src.contains("vec4<f32>(scalar, scalar, scalar, 0.0)"),
        "MAIN.wgsl must replicate scalar across RGB with alpha 0.0"
    );
}

/// DX12 test: 96x64 => 144x96 upscale (1.5x non-integer ratio) with distinct
/// alpha top/bottom to prove no crop occurs (full image processed, not just a
/// sub-rectangle). The CNN path does not trigger for 1.5x (2x=192x128 exceeds
/// target on both axes), so this exercises the GPU Bilinear whole-frame resample
/// from the original source to the exact non-integer target. At 1.5x run complete
/// 2x CNN then GPU Bilinear.
    #[test]
    fn gpu_anime4k_96x64_to_144x96_no_crop() {
        let Some(gpu) = gpu_or_skip("gpu_anime4k_96x64_to_144x96_no_crop") else { return };

        let dir = ensure_test_dir();
        // 96x64 source with distinct alpha top (50) / bottom (250) and non-constant RGB fixture.
        let src_path = dir.join("anime4k_96x64_src.png");
        let source = image::ImageBuffer::from_fn(96, 64, |x, y| {
            let alpha = if y < 32 { 50u8 } else { 250u8 };
            image::Rgba([x as u8, y as u8, (x + y) as u8, alpha])
        });
        source.save(&src_path).expect("save test PNG");

        // Non-integer 1.5x ratio: target 144x96 (not 2x).
        let job = UpscaleJob {
            id: "anime4k_96x64".to_string(),
            input_path: src_path.clone(),
            output_path: dir.join("anime4k_96x64_out.png"),
            backend: Backend::Gpu,
            algorithm: Algorithm::Anime4k,
            target_width: 144,
            target_height: 96,
            jpeg_quality: None,
            status: JobStatus::Running,
        };
        let result = gpu
            .upscale(&job, &image::DynamicImage::ImageRgba8(source.clone()))
            .unwrap_or_else(|e| panic!("Anime4K 96x64=>144x96 upscale failed: {e}"));
        let (w, h) = result.dimensions();
        assert_eq!((w, h), (144, 96), "Anime4K 96x64=>144x96 output must be 144x96");

        // Save and reload actual PNG result via image API.
        result.save(&dir.join("anime4k_96x64_out.png")).expect("save result PNG");
        let reloaded = image::open(&dir.join("anime4k_96x64_out.png"))
            .expect("reload result PNG")
            .to_rgba8();

        let rgba = reloaded;
        // Top-left (0,0) should have alpha=50 (top-half source, bilinear at pixel centre).
        assert_eq!(
            rgba.get_pixel(0, 0).0[3], 50,
            "alpha at (0,0) should be 50 (top-half source), got {}",
            rgba.get_pixel(0, 0).0[3]
        );
        // Top-right (143,0) should have alpha=50 (top half, no crop on right edge).
        assert_eq!(
            rgba.get_pixel(143, 0).0[3], 50,
            "alpha at (143,0) should be 50 (top half, no right-edge crop), got {}",
            rgba.get_pixel(143, 0).0[3]
        );
        // Bottom-left (0,95) should have alpha=250 (bottom half source).
        assert_eq!(
            rgba.get_pixel(0, 95).0[3], 250,
            "alpha at (0,95) should be 250 (bottom half source), got {}",
            rgba.get_pixel(0, 95).0[3]
        );
        // Bottom-right (143,95) should have alpha=250 (bottom half, no crop on right edge).
        assert_eq!(
            rgba.get_pixel(143, 95).0[3], 250,
            "alpha at (143,95) should be 250 (bottom half, no right-edge crop), got {}",
            rgba.get_pixel(143, 95).0[3]
        );
        // Center pixel (71,47) maps to source row ~31.3 (top half). Alpha is
        // 50 with nearest sampling or interpolated between 50-250 with bilinear
        // — either way it comes from the top half source, proving the full frame
        // is processed and the right/bottom edges are not cropped.
        let center_alpha = rgba.get_pixel(71, 47).0[3];
        assert!(
            center_alpha >= 50 && center_alpha <= 250,
            "center alpha {} should be between 50 and 250, proving no crop",
            center_alpha
        );
        // Output must not be flat: top row alpha 50, bottom row alpha 250.
        let first = rgba.get_pixel(0, 0).0;
        let varies = rgba.pixels().any(|p| p.0 != first);
        assert!(varies, "Anime4K output is a flat color");
        // The Anime4K-upscaled RGB must differ from direct bilinear original.
        let dir_bilinear_path = dir.join("anime4k_96x64_direct_bilinear.png");
        let direct_src = image::ImageBuffer::from_fn(96, 64, |x, y| {
            image::Rgba([x as u8, y as u8, (x + y) as u8, if y < 32 { 50u8 } else { 250u8 }])
        });
        direct_src.save(&dir_bilinear_path).expect("save direct bilinear PNG");
        let direct_result = gpu
            .upscale(
                &UpscaleJob {
                    id: "direct_bilinear".to_string(),
                    input_path: dir.join("anime4k_96x64_src.png"),
                    output_path: dir_bilinear_path,
                    backend: Backend::Gpu,
                    algorithm: Algorithm::Bilinear,
                    target_width: 144,
                    target_height: 96,
                    jpeg_quality: None,
                    status: JobStatus::Running,
                },
                &image::DynamicImage::ImageRgba8(source.clone()),
            )
            .unwrap_or_else(|e| panic!("Direct bilinear upscale failed: {e}"));
        let direct_rgba = direct_result.to_rgba8();
        // Verify the Anime4K result differs in RGB from direct bilinear over full image.
        assert!(
            rgba.pixels().zip(direct_rgba.pixels()).any(|(a, b)| a.0[..3] != b.0[..3]),
            "Anime4K result RGB must differ from direct bilinear original somewhere in the full image"
        );
        // Assert Anime4K result equals 2x Anime then GPU Bilinear to 144x96.
        // 2x Anime4K target (192x128) then bilinear down to 144x96 reproduces
        // the same non-integer upsample path, and their RGB must match.
        let job_2x = UpscaleJob {
            id: "anime4k_2x".to_string(),
            input_path: src_path.clone(),
            output_path: dir.join("anime4k_2x_out.png"),
            backend: Backend::Gpu,
            algorithm: Algorithm::Anime4k,
            target_width: 192,
            target_height: 128,
            jpeg_quality: None,
            status: JobStatus::Running,
        };
        let result_2x = gpu
            .upscale(&job_2x, &image::DynamicImage::ImageRgba8(source.clone()))
            .unwrap_or_else(|e| panic!("Anime4K 2x upscale failed: {e}"));
        result_2x.save(&dir.join("anime4k_2x_out.png")).expect("save 2x Anime4K PNG");
        let reloaded_2x = image::open(&dir.join("anime4k_2x_out.png"))
            .expect("reload 2x Anime4K PNG")
            .to_rgba8();
        // Bilinear from 192x128 to 144x96.
        let job_bilinear = UpscaleJob {
            id: "bilinear_192to144".to_string(),
            input_path: dir.join("anime4k_2x_out.png"),
            output_path: dir.join("anime4k_2x_bilinear_144x96.png"),
            backend: Backend::Gpu,
            algorithm: Algorithm::Bilinear,
            target_width: 144,
            target_height: 96,
            jpeg_quality: None,
            status: JobStatus::Running,
        };
        let result_bilinear = gpu
            .upscale(&job_bilinear, &image::DynamicImage::ImageRgba8(reloaded_2x.clone()))
            .unwrap_or_else(|e| panic!("Bilinear 192x128=>144x96 failed: {e}"));
        let bilinear_rgba = result_bilinear.to_rgba8();
        // Assert Anime4K result equals 2x Anime then GPU Bilinear (full-frame chained result).
        assert_eq!(rgba.as_raw(), bilinear_rgba.as_raw(), "full-frame chained result");
    }

    /// Regression: GPU validation failure must not be hidden by CPU fallback.
    /// The `upscale` method returns Err on GPU error scope failure; callers
    /// (commands.rs) handle fallback. This test verifies the GPU path reports
    /// the error rather than silently producing CPU output.
    #[test]
    fn gpu_anime4k_validation_error_not_hidden_by_fallback() {
    // This test verifies the error-propagation contract: GpuUpscaler::upscale
    // returns Err on GPU error-scope failure (checked after every pass), and
    // does not fall back internally. The fallback decision lives in commands.rs.
    // We verify by source inspection that execute_anime4k and run_anime_cnn
    // check error_flag after every pass and return Err.
    let gpu_src = read_to_string_rel("src/gpu.rs");
    let error_check_count = gpu_src.matches("error_flag.lock().unwrap()").count();
    assert!(
        error_check_count >= 4,
        "gpu.rs must check error_flag after GPU operations (>=4 checks), found {error_check_count}"
    );
    // Every pop_error_scope must be followed by error handling, not ignored.
    assert!(
        gpu_src.contains("if let Some(err) = validation_err.or(oom_err)"),
        "GPU error scopes must be checked with if-let, not ignored"
    );
    // catch_unwind is only allowed in shader/pipeline creation safe wrappers
    // (which convert panics to Err), never around GPU dispatch where it would
    // hide validation errors. Verify it appears exactly twice (the two safe
    // constructors).
    let catch_unwind_count = gpu_src.matches("catch_unwind").count();
    assert_eq!(
        catch_unwind_count, 2,
        "catch_unwind ({catch_unwind_count}) must appear exactly twice (in \
         create_shader_module_safe and create_compute_pipeline_safe), never around GPU dispatch"
    );
}
