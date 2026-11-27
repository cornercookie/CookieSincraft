//! CPU upscaler backed by `fast_image_resize`.
//!
//! Provides `CpuUpscaler`, a CPU implementation of the `Upscaler` trait that
//! resizes images to the requested target dimensions using nearest-neighbor,
//! bilinear, Catmull-Rom, or Lanczos3 filtering.
//!
//! `fast_image_resize`'s AVX2/SSE4 convolution paths read the internally
//! computed kernel coefficients with aligned i32 loads. Those coefficients are
//! not 4-byte aligned when the Lanczos3 window size is odd (e.g. 7 for
//! Lanczos3), so the SIMD paths panic with "misaligned pointer dereference"
//! even when the source buffer is 16-byte aligned. `CpuUpscaler` therefore
//! forces the scalar `Native` path (`CpuExtensions::None`), which reads
//! coefficients element-by-element and is alignment-agnostic. This does not
//! change the algorithm mapping (`Algorithm` -> `ResizeAlg`) or the public API.
//!
//! The controlled pipeline mirrors `fast_image_resize` 0.7's two-pass
//! convolution (horizontal, then vertical). It processes horizontal strips of
//! source rows with the full X extent, then vertical strips of destination
//! columns with the full Y extent. Every strip uses the same scalar resizer as
//! the monolithic path, so output is bit-exact while cancellation and progress
//! can be checked between strips. Buffers are ordinary `Vec`s (no custom
//! allocator), matching allocation and deallocation.

use std::borrow::Cow;
use std::num::NonZeroU32;

use fast_image_resize::{CpuExtensions, FilterType, Image, PixelType, ResizeAlg, Resizer};
use image::GenericImageView;

use crate::upscaler::{Algorithm, Backend, UpscaleControl, UpscaleError, UpscaleJob, Upscaler};

/// Maximum size of an internal resize buffer (64 MiB).
const INTERNAL_BUFFER_LIMIT: usize = 64 * 1024 * 1024;

/// Source rows handled per horizontal band.
const BAND_ROWS: u32 = 64;

/// Destination columns handled per vertical band.
const BAND_COLS: u32 = 64;

/// CPU-based upscaler using `fast_image_resize`.
pub struct CpuUpscaler;

impl Upscaler for CpuUpscaler {
    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
    ) -> Result<image::DynamicImage, UpscaleError> {
        self.upscale_with_control(job, image, UpscaleControl::none())
    }

    fn upscale_with_control(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
        control: UpscaleControl,
    ) -> Result<image::DynamicImage, UpscaleError> {
        let (src_width, src_height) = image.dimensions();
        if src_width == 0 || src_height == 0 {
            return Err(UpscaleError::Unknown("source image is empty".to_string()));
        }
        if job.target_width == 0 || job.target_height == 0 {
            return Err(UpscaleError::Unknown(
                "target dimensions must be non-zero".to_string(),
            ));
        }

        let src_rgba = image.to_rgba8();

        if job.algorithm == Algorithm::Nearest {
            return nearest_with_control(&src_rgba, job.target_width, job.target_height, &control);
        }

        let algorithm = match job.algorithm {
            Algorithm::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
            Algorithm::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
            Algorithm::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "CPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        convolution_with_control(
            &src_rgba,
            job.target_width,
            job.target_height,
            algorithm,
            &control,
        )
    }

    fn supports_backend(&self, backend: Backend) -> bool {
        matches!(backend, Backend::Cpu)
    }

    fn fallback_available(&self) -> bool {
        // CPU is the fallback endpoint; it does not fall back further.
        false
    }
}

fn check_cancel(control: &UpscaleControl) -> Result<(), UpscaleError> {
    if control.is_cancelled() {
        Err(UpscaleError::Cancelled("CPU resize cancelled".to_string()))
    } else {
        Ok(())
    }
}

fn report_and_check(
    control: &UpscaleControl,
    completed: usize,
    total: usize,
    last_report: &mut Option<std::time::Instant>,
) -> Result<(), UpscaleError> {
    // 100% is reported only after the finished image is built; bands report
    // their actual completed work up to (but not including) the final unit.
    if completed < total {
        control.report_progress(completed, total, last_report);
    }
    // A callback may request cancellation; honour it before starting the next
    // unit of work.
    check_cancel(control)
}

fn to_nonzero(value: u32) -> Result<NonZeroU32, UpscaleError> {
    NonZeroU32::new(value)
        .ok_or_else(|| UpscaleError::Unknown("image dimension is zero".to_string()))
}

fn nearest_with_control(
    src: &image::RgbaImage,
    dst_width: u32,
    dst_height: u32,
    control: &UpscaleControl,
) -> Result<image::DynamicImage, UpscaleError> {
    let (src_width, src_height) = (src.width(), src.height());
    let src_bytes = src.as_raw();
    let row_bytes = src_width as usize * 4;

    let x_scale = src_width as f64 / dst_width as f64;
    let y_scale = src_height as f64 / dst_height as f64;
    let x_in_start = x_scale * 0.5;
    let y_in_start = y_scale * 0.5;

    // Same table construction as fast_image_resize 0.7's nearest resampler.
    let x_in_tab: Vec<usize> = (0..dst_width)
        .map(|x| ((x_in_start + x_scale * x as f64) as usize).min(src_width as usize))
        .collect();

    let total = dst_width as usize * dst_height as usize;
    let mut dst = vec![0u8; total * 4];
    let mut last_report = None;
    let mut completed = 0usize;
    // fast_image_resize accumulates the source position by repeated addition,
    // not multiplication; mirror that for identical sampling.
    let mut src_y = y_in_start;

    for y in 0..dst_height {
        if y % BAND_ROWS == 0 {
            check_cancel(control)?;
        }
        let src_row = (src_y as usize).min(src_height as usize - 1);
        let src_offset = src_row * row_bytes;
        let dst_offset = y as usize * dst_width as usize * 4;
        for (x, &src_x) in x_in_tab.iter().enumerate() {
            let s = src_offset + src_x * 4;
            let d = dst_offset + x * 4;
            dst[d..d + 4].copy_from_slice(&src_bytes[s..s + 4]);
        }
        src_y += y_scale;
        completed += dst_width as usize;

        if (y + 1) % BAND_ROWS == 0 {
            report_and_check(control, completed, total, &mut last_report)?;
        }
    }

    if dst_height % BAND_ROWS != 0 {
        check_cancel(control)?;
    }

    let out = image::ImageBuffer::from_raw(dst_width, dst_height, dst)
        .ok_or_else(|| UpscaleError::Unknown("resized image has invalid dimensions".to_string()))?;
    // 100% is reported only after the product is complete.
    control.report_progress(total, total, &mut last_report);
    Ok(image::DynamicImage::ImageRgba8(out))
}

fn convolution_with_control(
    src: &image::RgbaImage,
    dst_width: u32,
    dst_height: u32,
    algorithm: ResizeAlg,
    control: &UpscaleControl,
) -> Result<image::DynamicImage, UpscaleError> {
    let (src_width, src_height) = (src.width(), src.height());
    let src_bytes = src.as_raw();
    let need_horizontal = dst_width != src_width;
    let need_vertical = dst_height != src_height;

    let horizontal_work = if need_horizontal {
        src_width as usize * src_height as usize
    } else {
        0
    };
    let vertical_work = if need_vertical {
        dst_width as usize * dst_height as usize
    } else {
        0
    };
    let total = horizontal_work + vertical_work;

    if !need_horizontal && !need_vertical {
        control.report_progress(0, 0, &mut None);
        let out = image::ImageBuffer::from_raw(src_width, src_height, src_bytes.to_vec())
            .ok_or_else(|| {
                UpscaleError::Unknown("resized image has invalid dimensions".to_string())
            })?;
        return Ok(image::DynamicImage::ImageRgba8(out));
    }

    let src_width_nz = to_nonzero(src_width)?;
    let src_height_nz = to_nonzero(src_height)?;
    let dst_width_nz = to_nonzero(dst_width)?;

    let mut last_report = None;
    let mut completed = 0usize;

    let mut resizer = Resizer::new(algorithm);
    // Force the scalar Native path: fast_image_resize's AVX2/SSE4
    // convolution paths read the internally computed kernel coefficients
    // with aligned i32 loads, which panics ("misaligned pointer
    // dereference") when the Lanczos3 window size is odd. This does not
    // change the algorithm mapping or the public API.
    unsafe {
        resizer.set_cpu_extensions(CpuExtensions::None);
    }

    // Horizontal pass: source-row strips with the full X extent. The result is
    // the intermediate image at destination width and source height.
    let intermediate_bytes = (dst_width as usize)
        .checked_mul(src_height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| UpscaleError::Unknown("internal resize buffer size overflow".to_string()))?;
    if need_horizontal && intermediate_bytes > INTERNAL_BUFFER_LIMIT {
        return Err(UpscaleError::Unknown(format!(
            "internal resize buffer of {intermediate_bytes} bytes exceeds the 64 MiB limit"
        )));
    }

    let intermediate: Cow<'_, [u8]> = if need_horizontal {
        let mut intermediate = vec![0u8; intermediate_bytes];
        let mut y = 0u32;
        while y < src_height {
            check_cancel(control)?;
            let band_rows = BAND_ROWS.min(src_height - y);
            let band_rows_nz = to_nonzero(band_rows)?;
            let in_start = y as usize * src_width as usize * 4;
            let in_len = band_rows as usize * src_width as usize * 4;
            let band = src_bytes[in_start..in_start + in_len].to_vec();

            let src_band = Image::from_vec_u8(src_width_nz, band_rows_nz, band, PixelType::U8x4)
                .map_err(|e| UpscaleError::Unknown(format!("creating source strip: {e:?}")))?;
            let mut dst_band = Image::new(dst_width_nz, band_rows_nz, PixelType::U8x4);
            resizer
                .resize(&src_band.view(), &mut dst_band.view_mut())
                .map_err(|e| UpscaleError::Unknown(format!("resize failed: {e:?}")))?;

            let out_start = y as usize * dst_width as usize * 4;
            let out_len = band_rows as usize * dst_width as usize * 4;
            intermediate[out_start..out_start + out_len].copy_from_slice(dst_band.buffer());

            y += band_rows;
            completed += src_width as usize * band_rows as usize;
            report_and_check(control, completed, total, &mut last_report)?;
        }
        Cow::Owned(intermediate)
    } else {
        Cow::Borrowed(src_bytes)
    };

    if !need_vertical {
        let out = image::ImageBuffer::from_raw(dst_width, dst_height, intermediate.into_owned())
            .ok_or_else(|| {
                UpscaleError::Unknown("resized image has invalid dimensions".to_string())
            })?;
        control.report_progress(total, total, &mut last_report);
        return Ok(image::DynamicImage::ImageRgba8(out));
    }

    let dst_height_nz = to_nonzero(dst_height)?;
    let src_height_usize = src_height as usize;
    let dst_width_usize = dst_width as usize;
    let mut dst = vec![0u8; dst_width_usize * dst_height as usize * 4];

    // Vertical pass: destination-column strips with the full Y extent.
    let mut x = 0u32;
    while x < dst_width {
        check_cancel(control)?;
        let strip_cols = BAND_COLS.min(dst_width - x);
        let strip_cols_nz = to_nonzero(strip_cols)?;
        let strip_row_bytes = strip_cols as usize * 4;

        let mut strip = Vec::with_capacity(strip_row_bytes * src_height_usize);
        for row in 0..src_height_usize {
            let start = row * dst_width_usize * 4 + x as usize * 4;
            strip.extend_from_slice(&intermediate[start..start + strip_row_bytes]);
        }

        let src_strip = Image::from_vec_u8(strip_cols_nz, src_height_nz, strip, PixelType::U8x4)
            .map_err(|e| UpscaleError::Unknown(format!("creating source strip: {e:?}")))?;
        let mut dst_strip = Image::new(strip_cols_nz, dst_height_nz, PixelType::U8x4);
        resizer
            .resize(&src_strip.view(), &mut dst_strip.view_mut())
            .map_err(|e| UpscaleError::Unknown(format!("resize failed: {e:?}")))?;

        for row in 0..dst_height as usize {
            let src_start = row * strip_row_bytes;
            let dst_start = row * dst_width_usize * 4 + x as usize * 4;
            dst[dst_start..dst_start + strip_row_bytes]
                .copy_from_slice(&dst_strip.buffer()[src_start..src_start + strip_row_bytes]);
        }

        x += strip_cols;
        completed += strip_cols as usize * dst_height as usize;
        report_and_check(control, completed, total, &mut last_report)?;
    }

    let out = image::ImageBuffer::from_raw(dst_width, dst_height, dst)
        .ok_or_else(|| UpscaleError::Unknown("resized image has invalid dimensions".to_string()))?;
    // Report 100% only after the product is complete.
    control.report_progress(total, total, &mut last_report);
    Ok(image::DynamicImage::ImageRgba8(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    fn make_job(algorithm: Algorithm, width: u32, height: u32) -> UpscaleJob {
        UpscaleJob {
            id: "test".to_string(),
            input_path: PathBuf::new(),
            output_path: PathBuf::new(),
            backend: Backend::Cpu,
            algorithm,
            target_width: width,
            target_height: height,
            jpeg_quality: None,
            status: crate::upscaler::JobStatus::Queued,
        }
    }

    fn make_image(width: u32, height: u32) -> image::DynamicImage {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                data.push((x.wrapping_mul(37).wrapping_add(y * 11) % 256) as u8);
                data.push((x.wrapping_mul(5).wrapping_add(y * 61) % 256) as u8);
                data.push((x.wrapping_mul(97).wrapping_add(y * 3) % 256) as u8);
                data.push(255);
            }
        }
        image::DynamicImage::ImageRgba8(image::ImageBuffer::from_raw(width, height, data).unwrap())
    }

    /// Original monolithic fast_image_resize path used as the reference.
    fn reference_upscale(job: &UpscaleJob, image: &image::DynamicImage) -> image::DynamicImage {
        let (src_width, src_height) = image.dimensions();
        let algorithm = match job.algorithm {
            Algorithm::Nearest => ResizeAlg::Nearest,
            Algorithm::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
            Algorithm::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
            Algorithm::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
            other => panic!("unsupported {other:?}"),
        };
        let src_rgba = image.to_rgba8();
        let src = Image::from_vec_u8(
            NonZeroU32::new(src_width).unwrap(),
            NonZeroU32::new(src_height).unwrap(),
            src_rgba.into_raw(),
            PixelType::U8x4,
        )
        .unwrap();
        let mut dst = Image::new(
            NonZeroU32::new(job.target_width).unwrap(),
            NonZeroU32::new(job.target_height).unwrap(),
            PixelType::U8x4,
        );
        let mut resizer = Resizer::new(algorithm);
        unsafe { resizer.set_cpu_extensions(CpuExtensions::None) };
        resizer.resize(&src.view(), &mut dst.view_mut()).unwrap();
        let out = image::ImageBuffer::from_raw(
            job.target_width,
            job.target_height,
            dst.buffer().to_vec(),
        )
        .unwrap();
        image::DynamicImage::ImageRgba8(out)
    }

    #[test]
    fn cpu_resize_bit_exact_all_filters_and_odd_dimensions() {
        let cases = [
            (37u32, 29u32, 101u32, 83u32),
            (101, 83, 37, 29),
            (64, 48, 65, 47),
            (65, 47, 64, 48),
            (9, 13, 3, 4),
            (3, 4, 9, 13),
            (1, 1, 7, 5),
        ];
        for algorithm in [
            Algorithm::Nearest,
            Algorithm::Bilinear,
            Algorithm::CatmullRom,
            Algorithm::Lanczos3,
        ] {
            for (sw, sh, dw, dh) in cases {
                let job = make_job(algorithm, dw, dh);
                let image = make_image(sw, sh);
                let actual = CpuUpscaler
                    .upscale(&job, &image)
                    .unwrap_or_else(|e| panic!("{algorithm:?} {sw}x{sh}->{dw}x{dh}: {e}"));
                let expected = reference_upscale(&job, &image);
                assert_eq!(
                    actual.to_rgba8().as_raw(),
                    expected.to_rgba8().as_raw(),
                    "{algorithm:?} {sw}x{sh}->{dw}x{dh} not bit-exact"
                );
            }
        }
    }

    #[test]
    fn cpu_resize_cancels_deterministically_mid_work() {
        let job = make_job(Algorithm::Lanczos3, 400, 400);
        let image = make_image(200, 200);
        let cancel = Arc::new(AtomicBool::new(false));
        let reports = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let cancel_cb = cancel.clone();
        let reports_cb = reports.clone();
        let control = UpscaleControl {
            cancel: cancel.clone(),
            on_progress: Some(Arc::new(move |completed, total| {
                reports_cb.lock().unwrap().push((completed, total));
                cancel_cb.store(true, Ordering::Relaxed);
            })),
        };

        let result = CpuUpscaler.upscale_with_control(&job, &image, control);
        assert!(matches!(result, Err(UpscaleError::Cancelled(_))));
        let reports = reports.lock().unwrap();
        assert!(
            !reports.is_empty(),
            "progress must fire before cancellation"
        );
        assert!(reports[0].0 < reports[0].1, "cancel must happen mid-work");
    }

    #[test]
    fn cpu_resize_progress_is_monotonic_and_ends_at_total() {
        let job = make_job(Algorithm::Lanczos3, 301, 241);
        let image = make_image(150, 120);
        let reports = Arc::new(Mutex::new(Vec::<(usize, usize)>::new()));
        let reports_cb = reports.clone();
        let control = UpscaleControl {
            cancel: Arc::new(AtomicBool::new(false)),
            on_progress: Some(Arc::new(move |completed, total| {
                reports_cb.lock().unwrap().push((completed, total));
            })),
        };

        CpuUpscaler
            .upscale_with_control(&job, &image, control)
            .unwrap();
        let reports = reports.lock().unwrap();
        let total = 150 * 120 + 301 * 241;
        let mut previous = 0;
        for &(completed, reported_total) in reports.iter() {
            assert_eq!(reported_total, total);
            assert!(completed >= previous);
            previous = completed;
        }
        assert_eq!(*reports.last().unwrap(), (total, total));
    }

    #[test]
    fn cpu_resize_rejects_internal_buffer_over_64_mib() {
        let job = make_job(Algorithm::Lanczos3, 16385, 1024);
        let image = make_image(1024, 1024);
        let result = CpuUpscaler.upscale(&job, &image);
        match result {
            Err(UpscaleError::Unknown(message)) => {
                assert!(message.contains("64 MiB"), "unexpected message: {message}");
            }
            other => panic!("expected buffer-limit error, got {other:?}"),
        }
    }
}
