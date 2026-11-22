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
//! We still copy source pixels into a 16-byte-aligned buffer defensively, as
//! required, since the crate's SIMD source loads read from that buffer.

use std::alloc::{alloc, Layout};
use std::num::NonZeroU32;

use fast_image_resize::{CpuExtensions, FilterType, Image, PixelType, ResizeAlg, Resizer};
use image::GenericImageView;

use crate::upscaler::{Algorithm, Backend, UpscaleError, UpscaleJob, Upscaler};

/// CPU-based upscaler using `fast_image_resize`.
pub struct CpuUpscaler;

impl Upscaler for CpuUpscaler {
    fn upscale(
        &self,
        job: &UpscaleJob,
        image: &image::DynamicImage,
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

        let algorithm = match job.algorithm {
            Algorithm::Nearest => ResizeAlg::Nearest,
            Algorithm::Bilinear => ResizeAlg::Convolution(FilterType::Bilinear),
            Algorithm::CatmullRom => ResizeAlg::Convolution(FilterType::CatmullRom),
            Algorithm::Lanczos3 => ResizeAlg::Convolution(FilterType::Lanczos3),
            other => {
                return Err(UpscaleError::UnsupportedAlgorithm(format!(
                    "CPU upscaler does not support algorithm {other:?}"
                )));
            }
        };

        let src_rgba = image.to_rgba8();
        let src_width = NonZeroU32::new(src_width).ok_or_else(|| {
            UpscaleError::Unknown("source image width is zero".to_string())
        })?;
        let src_height = NonZeroU32::new(src_height).ok_or_else(|| {
            UpscaleError::Unknown("source image height is zero".to_string())
        })?;
        let dst_width = NonZeroU32::new(job.target_width).ok_or_else(|| {
            UpscaleError::Unknown("target width is zero".to_string())
        })?;
        let dst_height = NonZeroU32::new(job.target_height).ok_or_else(|| {
            UpscaleError::Unknown("target height is zero".to_string())
        })?;

        // Copy source pixels into a 16-byte-aligned buffer.
        //
        // `image::DynamicImage::to_rgba8()` clones into a heap buffer whose
        // allocation is not guaranteed to be 16-byte aligned.
        // `fast_image_resize` performs SIMD loads from the source buffer, so
        // a misaligned pointer dereference can panic. We own the aligned
        // buffer here and keep it alive for the whole resize (`Image` owns it
        // and `view()` borrows it).
        let src_bytes = src_rgba.as_raw();
        let size = src_bytes.len();
        let layout = Layout::from_size_align(size, 16).expect("valid buffer layout");
        let aligned_ptr = unsafe { alloc(layout) };
        if aligned_ptr.is_null() {
            return Err(UpscaleError::Unknown(
                "failed to allocate aligned source buffer".to_string(),
            ));
        }
        unsafe { std::ptr::copy_nonoverlapping(src_bytes.as_ptr(), aligned_ptr, size) };
        let aligned_buf = unsafe { Vec::from_raw_parts(aligned_ptr, size, size) };

        // Owned source image over the RGBA pixels.
        let src_image = Image::from_vec_u8(
            src_width,
            src_height,
            aligned_buf,
            PixelType::U8x4,
        )
        .map_err(|e| UpscaleError::Unknown(format!("creating source image: {e:?}")))?;

        // Destination image owning its pixel buffer.
        let mut dst_image = Image::new(dst_width, dst_height, src_image.pixel_type());
        let mut dst_view = dst_image.view_mut();

        let mut resizer = Resizer::new(algorithm);
        // Force the scalar Native path: fast_image_resize's AVX2/SSE4
        // convolution paths read the internally computed kernel coefficients
        // with aligned i32 loads, which panics ("misaligned pointer
        // dereference") when the Lanczos3 window size is odd. This does not
        // change the algorithm mapping or the public API.
        unsafe {
            resizer.set_cpu_extensions(CpuExtensions::None);
        }
        resizer
            .resize(&src_image.view(), &mut dst_view)
            .map_err(|e| UpscaleError::Unknown(format!("resize failed: {e:?}")))?;

        let dst_buffer = dst_image.buffer().to_vec();
        let dst_rgba = image::ImageBuffer::from_raw(job.target_width, job.target_height, dst_buffer)
            .ok_or_else(|| {
                UpscaleError::Unknown("resized image has invalid dimensions".to_string())
            })?;

        Ok(image::DynamicImage::ImageRgba8(dst_rgba))
    }

    fn supports_backend(&self, backend: Backend) -> bool {
        matches!(backend, Backend::Cpu)
    }

    fn fallback_available(&self) -> bool {
        // CPU is the fallback endpoint; it does not fall back further.
        false
    }
}
