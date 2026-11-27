//! Image encode/decode and export for CookieSincraft.
//!
//! Provides [`Converter`], a stateless utility for loading and saving images
//! in PNG, JPEG, WebP and AVIF formats using the `image` crate.
//!
//! # Format support
//!
//! PNG, JPEG and WebP are available with the `image` crate's default
//! features. AVIF requires the `image` crate's `avif` feature
//! (`image = { version = "0.24", features = ["avif"] }`); with the default
//! features AVIF load/save operations return a [`ConverterError`] explaining
//! that the feature is missing.
//!
//! # JPEG quality
//!
//! JPEG quality is validated to the inclusive range 50..=100. Values outside
//! that range produce an error; for non-JPEG formats the quality is ignored.

use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Cursor, ErrorKind, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use image::io::{Limits, Reader};
use image::{DynamicImage, ImageDecoder, ImageOutputFormat};

/// Default JPEG quality used when no quality is provided.
const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Maximum width/height of a single ICO entry.
const ICO_MAX_DIM: u32 = 256;

/// Maximum source image width or height accepted before pixel allocation.
const MAX_IMAGE_DIM: u32 = 4096;

/// Maximum decoded pixel buffer accepted before allocation (64 MiB).
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;

/// Supported image file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// Portable Network Graphics (lossless).
    Png,
    /// Joint Photographic Experts Group (lossy, quality 50..=100).
    Jpeg,
    /// WebP (lossless via the `image` crate's default feature set).
    WebP,
    /// Graphics Interchange Format.
    Gif,
    /// Tagged Image File Format (requires the `image` crate's `tiff` feature).
    Tiff,
    /// Windows Bitmap.
    Bmp,
    /// Microsoft Icon.
    Ico,
    /// Portable Any Map (PNM: PBM, PGM, PPM, PAM).
    Pnm,
}

/// Error returned by converter operations.
#[derive(Debug)]
pub enum ConverterError {
    /// An I/O operation failed (read/write filesystem).
    Io(String),
    /// Decoding the source image failed.
    Decode(String),
    /// Encoding the output image failed.
    Encode(String),
    /// The requested JPEG quality is outside the allowed range 50..=100.
    InvalidQuality(String),
    /// The job was cancelled before publication; the staged file is removed
    /// and the previous target is left untouched.
    Cancelled(String),
    /// An unspecified error.
    Unknown(String),
}

impl std::fmt::Display for ConverterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConverterError::Io(msg) => write!(f, "I/O error: {msg}"),
            ConverterError::Decode(msg) => write!(f, "decode error: {msg}"),
            ConverterError::Encode(msg) => write!(f, "encode error: {msg}"),
            ConverterError::InvalidQuality(msg) => write!(f, "invalid quality: {msg}"),
            ConverterError::Cancelled(msg) => write!(f, "cancelled: {msg}"),
            ConverterError::Unknown(msg) => write!(f, "converter error: {msg}"),
        }
    }
}

impl std::error::Error for ConverterError {}

/// Publication gate that combines a cancel flag with a mutex-locked
/// section to coordinate cancel-safe atomic publication.
///
/// The `cancel` flag is an `Arc<AtomicBool>` that can be observed inside the
/// locked section. `request_cancel` acquires the mutex (poison → Io error)
/// and then atomically sets the flag, so the check-and-set is deterministic.
/// The gate ensures the flag is set atomically under the lock, so check-and-set
/// is deterministic and the staged file is published or cleaned up consistently.
pub(crate) struct PublicationGate {
    cancel: Arc<AtomicBool>,
    lock: Mutex<()>,
}

impl PublicationGate {
    /// Create a new gate with the cancel flag cleared.
    pub(crate) fn new() -> Self {
        PublicationGate {
            cancel: Arc::new(AtomicBool::new(false)),
            lock: Mutex::new(()),
        }
    }

    /// Return the `Arc<AtomicBool>` that can be checked for cancellation.
    pub(crate) fn cancel_token(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Acquire the mutex (poison maps to `Err`) and set the cancel flag to `true`.
    /// Returns `Ok(())` on success, or `Err` if the mutex was poisoned.
    pub(crate) fn request_cancel(&self) -> Result<(), String> {
        let _guard = self
            .lock
            .lock()
            .map_err(|e| format!("lock poisoned: {e}"))?;
        self.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }
}

/// Image encode/decode and export.
///
/// Stateless utility holding no configuration; all parameters are passed per
/// call.
pub struct Converter;

impl Converter {
    /// Read an image from `path`, guessing its format from the file contents.
    ///
    /// Supports the formats enabled in the `image` crate (PNG, JPEG, WebP,
    /// GIF, TIFF, BMP, ICO, PNM, HDR and other default codecs).
    ///
    /// Decoding is bounded: the same held file handle is used to read the
    /// header, check dimensions and pixel format, and then decode. Images
    /// wider or taller than 4096, or whose decoded pixel buffer would exceed
    /// 64 MiB, are rejected before the full pixel allocation.
    pub fn load_image(path: &Path) -> Result<DynamicImage, ConverterError> {
        let file = File::open(path)
            .map_err(|e| ConverterError::Io(format!("failed to open image {path:?}: {e}")))?;
        Self::decode_reader(BufReader::new(file), &format!("{path:?}"))
    }

    /// Read an image from an in-memory byte slice using the same bounded
    /// decode path as [`Converter::load_image`].
    ///
    /// The caller is responsible for bounding the input bytes themselves;
    /// this function bounds the decoded dimensions and pixel buffer.
    pub fn load_image_from_bytes(bytes: &[u8]) -> Result<DynamicImage, ConverterError> {
        Self::decode_reader(Cursor::new(bytes), "<memory>")
    }

    /// Guess the format and decode from one already-open reader.
    ///
    /// The reader is never reopened by path: format guessing seeks back to the
    /// start, then the selected decoder consumes the same handle.
    fn decode_reader<R>(reader: R, source: &str) -> Result<DynamicImage, ConverterError>
    where
        R: BufRead + Seek,
    {
        let reader = Reader::new(reader).with_guessed_format().map_err(|e| {
            ConverterError::Decode(format!("could not determine image format of {source}: {e}"))
        })?;
        let format = reader.format().ok_or_else(|| {
            ConverterError::Decode(format!("could not determine image format of {source}"))
        })?;
        Self::decode_format(reader.into_inner(), format, source)
    }

    /// Dispatch to the bounded decoder for a guessed format.
    fn decode_format<R>(
        reader: R,
        format: image::ImageFormat,
        source: &str,
    ) -> Result<DynamicImage, ConverterError>
    where
        R: BufRead + Seek,
    {
        use image::codecs;

        let decoded: Result<DynamicImage, String> = match format {
            image::ImageFormat::Png => {
                codecs::png::PngDecoder::with_limits(reader, Self::decode_limits())
                    .map_err(|e| e.to_string())
                    .and_then(Self::decode_bounded)
            }
            image::ImageFormat::Jpeg => codecs::jpeg::JpegDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::WebP => Self::decode_webp(reader),
            image::ImageFormat::Gif => codecs::gif::GifDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Tiff => codecs::tiff::TiffDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Bmp => codecs::bmp::BmpDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Ico => codecs::ico::IcoDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Pnm => codecs::pnm::PnmDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Hdr => codecs::hdr::HdrAdapter::new(BufReader::new(reader))
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Dds => codecs::dds::DdsDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Farbfeld => codecs::farbfeld::FarbfeldDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::OpenExr => codecs::openexr::OpenExrDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Qoi => codecs::qoi::QoiDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            image::ImageFormat::Tga => codecs::tga::TgaDecoder::new(reader)
                .map_err(|e| e.to_string())
                .and_then(Self::decode_bounded),
            other => Err(format!("unsupported image format {other:?}")),
        };

        decoded.map_err(|msg| {
            ConverterError::Decode(format!("failed to decode image at {source}: {msg}"))
        })
    }

    /// Decode a WebP image from a buffered reader, with dimension and size bounds.
    fn decode_webp<R: BufRead + Seek>(reader: R) -> Result<DynamicImage, String> {
        let mut decoder = image_webp::WebPDecoder::new(reader).map_err(|e| e.to_string())?;
        let (width, height) = decoder.dimensions();
        // Reject zero dimensions
        if width == 0 || height == 0 {
            return Err(format!("WebP dimensions are zero: {}x{}", width, height));
        }
        // Reject exceeding MAX_IMAGE_DIM
        if width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM {
            return Err(format!(
                "WebP dimensions {}x{} exceed maximum allowed {}x{}",
                width, height, MAX_IMAGE_DIM, MAX_IMAGE_DIM
            ));
        }
        let n = decoder
            .output_buffer_size()
            .ok_or_else(|| "WebP output size overflow".to_string())?;
        if n as u64 > MAX_DECODED_BYTES {
            return Err(format!(
                "WebP decoded size {} exceeds maximum allowed {}",
                n, MAX_DECODED_BYTES
            ));
        }
        decoder.set_memory_limit(MAX_DECODED_BYTES as usize);
        if decoder.is_animated() && !decoder.has_alpha() {
            if let Some([b, g, r, a]) = decoder.background_color_hint() {
                // BGRA on-disk → RGBA for set_background_color
                decoder
                    .set_background_color([r, g, b, a])
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut buf = Vec::new();
        buf.try_reserve_exact(n).map_err(|e| e.to_string())?;
        buf.resize(n, 0);
        decoder.read_image(&mut buf).map_err(|e| e.to_string())?;
        let img = if decoder.has_alpha() {
            image::RgbaImage::from_raw(width, height, buf).map(DynamicImage::ImageRgba8)
        } else {
            image::RgbImage::from_raw(width, height, buf).map(DynamicImage::ImageRgb8)
        };
        img.ok_or_else(|| "invalid WebP pixel buffer".to_string())
    }

    /// Decoder limits applied before any full pixel buffer is allocated.
    fn decode_limits() -> Limits {
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_DIM);
        limits.max_image_height = Some(MAX_IMAGE_DIM);
        limits.max_alloc = Some(MAX_DECODED_BYTES);
        limits
    }

    /// Check dimensions and decoded size, then apply codec limits and decode.
    ///
    /// The size check uses the decoder's output color type (bytes per pixel),
    /// so formats such as RGBA16 or RGB32F are rejected even when their width
    /// and height fit the 4096 limit.
    fn decode_bounded<'a, D>(mut decoder: D) -> Result<DynamicImage, String>
    where
        D: ImageDecoder<'a>,
    {
        let (width, height) = decoder.dimensions();
        if width == 0 || height == 0 {
            return Err("image dimensions must be non-zero".to_string());
        }
        if width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM {
            return Err(format!(
                "image dimensions {width}x{height} exceed the {MAX_IMAGE_DIM}x{MAX_IMAGE_DIM} limit"
            ));
        }

        let bytes_per_pixel = u64::from(decoder.color_type().bytes_per_pixel());
        let decoded_bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
            .ok_or_else(|| "decoded image size calculation overflowed".to_string())?;
        if decoded_bytes > MAX_DECODED_BYTES {
            return Err(format!(
                "decoded image buffer {decoded_bytes} bytes exceeds the 64 MiB limit"
            ));
        }

        let mut limits = Self::decode_limits();
        limits
            .reserve(decoder.total_bytes())
            .map_err(|e| format!("decoded image buffer exceeds the 64 MiB limit: {e}"))?;
        decoder
            .set_limits(limits)
            .map_err(|e| format!("image decoder limits rejected the image: {e}"))?;
        DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())
    }

    /// Write `image` to `path` in `format`.
    ///
    /// For `ImageFormat::Jpeg`, `jpeg_quality` must be `None` (default 85) or
    /// a value in 50..=100; any other value returns an error. For all other
    /// formats `jpeg_quality` is ignored.
    ///
    /// # Cancellation / publication gate
    ///
    /// When `cancel` is `Some`, it is checked after the encode flush and
    /// before the atomic publication (rename / no-replace hard link). If
    /// cancellation has been requested, the staged file is removed, the
    /// previous target is left untouched, and a [`ConverterError::Cancelled`]
    /// is returned. The same `Arc<AtomicBool>` that `cancel_job` sets is the
    /// guard, so before/after commit outcome is deterministic.
    ///
    /// # No-clobber publication (R2)
    ///
    /// When `overwrite` is `false`, publication uses an atomic no-replace
    /// hard link, so `path` is never silently clobbered even if it appears
    /// after the caller checked. When `overwrite` is `true`, `path` is
    /// replaced atomically.
    ///
    /// The output is always staged to an exclusively created temporary file
    /// in the same directory as `path`. On cancellation or encode errors the
    /// previous target stays intact: either the old file remains or the
    /// complete new file appears, never a truncated target.
    pub fn save_image(
        image: &DynamicImage,
        path: &Path,
        format: ImageFormat,
        jpeg_quality: Option<u8>,
        overwrite: bool,
    ) -> Result<(), ConverterError> {
        Self::save_image_with_cancel(image, path, format, jpeg_quality, overwrite, None)
    }

    /// Write `image` to `path` in `format` with optional cancellation.
    ///
    /// Same contract as [`save_image`] but accepts a cancel flag. Pass `None`
    /// for no cancellation.
    pub fn save_image_with_cancel(
        image: &DynamicImage,
        path: &Path,
        format: ImageFormat,
        jpeg_quality: Option<u8>,
        overwrite: bool,
        cancel: Option<&PublicationGate>,
    ) -> Result<(), ConverterError> {
        let quality = Self::resolve_jpeg_quality(format, jpeg_quality)?;
        let prepared = Self::prepare_for_format(image, format);
        let image = prepared.as_ref();

        let output_format = match format {
            ImageFormat::Png => ImageOutputFormat::Png,
            ImageFormat::Jpeg => ImageOutputFormat::Jpeg(quality),
            ImageFormat::WebP => ImageOutputFormat::WebP,
            ImageFormat::Gif => ImageOutputFormat::Gif,
            ImageFormat::Tiff => ImageOutputFormat::Tiff,
            ImageFormat::Bmp => ImageOutputFormat::Bmp,
            ImageFormat::Ico => ImageOutputFormat::Ico,
            ImageFormat::Pnm => ImageOutputFormat::Pnm(image::codecs::pnm::PnmSubtype::Pixmap(
                image::codecs::pnm::SampleEncoding::Binary,
            )),
        };

        // Stage next to the target so publication is atomic on the same volume.
        let (staging, file) = Self::create_staging(path)?;

        let result = (|| -> Result<(), ConverterError> {
            let mut writer = BufWriter::new(file);

            image.write_to(&mut writer, output_format).map_err(|e| {
                ConverterError::Encode(format!("failed to encode image to {staging:?}: {e}"))
            })?;
            writer.flush().map_err(|e| {
                ConverterError::Io(format!("failed to flush staging file {staging:?}: {e}"))
            })?;
            drop(writer);

            // Publication gate: after encode flush, before atomic publication.
            // If cancellation has been requested, the staged file is removed
            // by outer cleanup, the previous target is left untouched, and a
            // ConverterError::Cancelled is returned.
            let _publication_guard: Option<std::sync::MutexGuard<'_, ()>> =
                if let Some(gate) = cancel {
                    let _guard = gate
                        .lock
                        .lock()
                        .map_err(|e| ConverterError::Io(format!("lock poisoned: {e}")))?;
                    if gate.cancel.load(Ordering::Relaxed) {
                        return Err(ConverterError::Cancelled(
                            "cancelled before publication".to_string(),
                        ));
                    }
                    Some(_guard)
                } else {
                    None
                };

            if overwrite {
                // Approved replacement: atomically swap in the staged file.
                // If this fails the previous target is left untouched.
                std::fs::rename(&staging, path).map_err(|e| {
                    ConverterError::Io(format!("failed to publish staged output to {path:?}: {e}"))
                })
            } else {
                // Atomic no-replace: fails if the target exists, even if it
                // appeared after the caller checked.
                Self::publish_no_clobber(&staging, path)
            }
        })();

        // Best-effort cleanup of the staging entry. The published target is a
        // separate hard link (or the rename already consumed the staging file),
        // so this never removes the completed output.
        let _ = std::fs::remove_file(&staging);
        result
    }

    /// Create a uniquely named staging file next to `path` using exclusive
    /// creation, so an existing or concurrently created file is never truncated.
    fn create_staging(path: &Path) -> Result<(PathBuf, File), ConverterError> {
        static NEXT_STAGING: AtomicU64 = AtomicU64::new(0);

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("output");

        for _ in 0..8 {
            let n = NEXT_STAGING.fetch_add(1, Ordering::Relaxed);
            let unique = fastrand::u64(..);
            let staging = parent.join(format!(
                ".{file_name}.cookiesincraft-{}-{n}-{unique}.part",
                std::process::id()
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staging)
            {
                Ok(file) => return Ok((staging, file)),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    return Err(ConverterError::Io(format!(
                        "failed to create staging file {staging:?}: {e}"
                    )))
                }
            }
        }

        Err(ConverterError::Io(format!(
            "failed to create a unique staging file for {path:?}"
        )))
    }

    /// Publish `staging` as `path` without replacing an existing file.
    ///
    /// `hard_link` is atomic and fails with `AlreadyExists` when `path` exists,
    /// unlike `rename`, which would clobber it. The caller removes `staging`
    /// afterwards, leaving the published hard link intact.
    fn publish_no_clobber(staging: &Path, path: &Path) -> Result<(), ConverterError> {
        match std::fs::hard_link(staging, path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(ConverterError::Io(format!(
                "output file already exists: {path:?}"
            ))),
            Err(e) => Err(ConverterError::Io(format!(
                "failed to publish staged output to {path:?}: {e}"
            ))),
        }
    }

    /// Adapt `image` to the constraints of the target encoder.
    ///
    /// - ICO entries are limited to 256x256, so larger images are downscaled
    ///   (aspect ratio preserved).
    /// - PNM is written as a binary Pixmap (PPM), which has no alpha channel,
    ///   so the image is flattened to RGB8.
    ///
    /// Returns a borrowed image when no adaptation is needed.
    fn prepare_for_format(image: &DynamicImage, format: ImageFormat) -> Cow<'_, DynamicImage> {
        match format {
            ImageFormat::Ico if image.width() > ICO_MAX_DIM || image.height() > ICO_MAX_DIM => {
                Cow::Owned(image.resize(
                    ICO_MAX_DIM,
                    ICO_MAX_DIM,
                    image::imageops::FilterType::Lanczos3,
                ))
            }
            ImageFormat::Pnm if !matches!(image, DynamicImage::ImageRgb8(_)) => {
                Cow::Owned(DynamicImage::ImageRgb8(image.to_rgb8()))
            }
            _ => Cow::Borrowed(image),
        }
    }

    /// Resolve and validate the JPEG quality for the requested format.
    ///
    /// For non-JPEG formats the quality is ignored and the default is
    /// returned. For JPEG the quality must be `None` or within 50..=100.
    fn resolve_jpeg_quality(
        format: ImageFormat,
        jpeg_quality: Option<u8>,
    ) -> Result<u8, ConverterError> {
        match format {
            ImageFormat::Jpeg => match jpeg_quality {
                Some(q) if (50..=100).contains(&q) => Ok(q),
                Some(q) => Err(ConverterError::InvalidQuality(format!(
                    "JPEG quality {q} is outside the allowed range 50..=100"
                ))),
                None => Ok(DEFAULT_JPEG_QUALITY),
            },
            // For non-JPEG formats the JPEG quality is ignored entirely.
            _ => Ok(DEFAULT_JPEG_QUALITY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_rgba_test_image(path: &Path) -> image::RgbaImage {
        let img = image::ImageBuffer::from_fn(100, 100, |x, y| {
            image::Rgba([
                (x.wrapping_mul(2)) as u8,
                (y.wrapping_mul(2)) as u8,
                128,
                255,
            ])
        });
        image::DynamicImage::ImageRgba8(img.clone())
            .save(path)
            .expect("failed to write test image");
        img
    }

    fn make_rgb_test_image(path: &Path) -> image::RgbImage {
        let img = image::ImageBuffer::from_fn(100, 100, |x, y| {
            image::Rgb([(x.wrapping_mul(2)) as u8, (y.wrapping_mul(2)) as u8, 128])
        });
        image::DynamicImage::ImageRgb8(img.clone())
            .save(path)
            .expect("failed to write test image");
        img
    }

    /// Return staging files left behind in `dir`.
    fn staging_files(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .expect("read temp dir")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.contains(".cookiesincraft-"))
                    .unwrap_or(false)
            })
            .collect()
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    /// Patch an encoded PNG's IHDR dimensions and repair its CRC.
    fn patch_png_dimensions(png: &mut [u8], width: u32, height: u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&png[12..16], b"IHDR");
        png[16..20].copy_from_slice(&width.to_be_bytes());
        png[20..24].copy_from_slice(&height.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
    }

    /// Patch an encoded JPEG's SOF dimensions without touching pixel data.
    fn patch_jpeg_dimensions(jpeg: &mut [u8], width: u32, height: u32) {
        let mut i = 2;
        while i + 9 < jpeg.len() {
            if jpeg[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = jpeg[i + 1];
            if matches!(marker, 0xC0..=0xC3) {
                jpeg[i + 5..i + 7].copy_from_slice(&(height as u16).to_be_bytes());
                jpeg[i + 7..i + 9].copy_from_slice(&(width as u16).to_be_bytes());
                return;
            }
            i += 2;
        }
        panic!("JPEG SOF marker not found");
    }

    #[test]
    fn roundtrip_all_supported_formats() {
        let tmp = std::env::temp_dir().join("cookiesincraft_convert_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);

        let cases: &[(&str, ImageFormat)] = &[
            ("png", ImageFormat::Png),
            ("jpg", ImageFormat::Jpeg),
            ("webp", ImageFormat::WebP),
            ("gif", ImageFormat::Gif),
            ("bmp", ImageFormat::Bmp),
            ("tiff", ImageFormat::Tiff),
            ("ico", ImageFormat::Ico),
        ];

        for (ext, fmt) in cases {
            let out = tmp.join(format!("out.{}", ext));
            let img = Converter::load_image(&src).expect("load source");
            Converter::save_image(&img, &out, *fmt, Some(85), false)
                .unwrap_or_else(|e| panic!("{ext} encode failed: {e}"));
            assert!(out.exists(), "{ext} output file missing");

            // Round-trip: the output must be decodable again.
            let reloaded =
                Converter::load_image(&out).unwrap_or_else(|e| panic!("{ext} reload failed: {e}"));
            assert!(
                reloaded.width() > 0 && reloaded.height() > 0,
                "{ext} reload empty"
            );
        }
    }

    #[test]
    fn pnm_roundtrip_with_rgb_source() {
        // PNM (PPM) does not support alpha channels, so use an RGB source.
        let tmp = std::env::temp_dir().join("cookiesincraft_pnm_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgb_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");
        let out = tmp.join("out.pnm");
        Converter::save_image(&img, &out, ImageFormat::Pnm, None, false)
            .expect("pnm encode failed");
        assert!(out.exists());
        let reloaded = Converter::load_image(&out).expect("pnm reload failed");
        assert!(reloaded.width() > 0 && reloaded.height() > 0);
    }

    #[test]
    fn ico_large_source_is_downscaled_to_fit() {
        // ICO entries are limited to 256x256; real photos are much larger.
        let tmp = std::env::temp_dir().join("cookiesincraft_ico_large_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            1024,
            512,
            image::Rgba([10, 20, 30, 255]),
        ));
        let out = tmp.join("out.ico");
        Converter::save_image(&img, &out, ImageFormat::Ico, None, false)
            .expect("ico encode failed");
        let reloaded = Converter::load_image(&out).expect("ico reload failed");
        assert!(reloaded.width() <= 256 && reloaded.height() <= 256);
        assert_eq!(reloaded.width(), 256);
        assert_eq!(reloaded.height(), 128);
    }

    #[test]
    fn pnm_accepts_rgba_source() {
        // Typical PNG inputs carry alpha; PPM must still be writable.
        let tmp = std::env::temp_dir().join("cookiesincraft_pnm_rgba_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");
        let out = tmp.join("out.ppm");
        Converter::save_image(&img, &out, ImageFormat::Pnm, None, false)
            .expect("pnm encode failed");
        let reloaded = Converter::load_image(&out).expect("pnm reload failed");
        assert_eq!((reloaded.width(), reloaded.height()), (100, 100));
    }

    #[test]
    fn invalid_quality_is_rejected() {
        let tmp = std::env::temp_dir().join("cookiesincraft_quality_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");

        let out = tmp.join("out.jpg");
        let err =
            Converter::save_image(&img, &out, ImageFormat::Jpeg, Some(49), false).unwrap_err();
        assert!(
            matches!(err, ConverterError::InvalidQuality(_)),
            "unexpected error: {err:?}"
        );

        // Non-JPEG formats ignore quality entirely (no error for out-of-range).
        let out2 = tmp.join("out.png");
        Converter::save_image(&img, &out2, ImageFormat::Png, Some(49), false)
            .expect("png should ignore quality");
    }

    #[test]
    fn save_image_no_clobber_protects_existing_file() {
        // An existing output must not be silently clobbered unless overwrite
        // is chosen; the target must be left byte-identical on refusal.
        let tmp = std::env::temp_dir().join("cookiesincraft_noclobber_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");

        let out = tmp.join("existing.png");
        std::fs::write(&out, b"SENTINEL-CONTENT").expect("write sentinel");

        // Refuse without overwrite: error and target untouched.
        let err = Converter::save_image(&img, &out, ImageFormat::Png, None, false).unwrap_err();
        assert!(
            matches!(err, ConverterError::Io(_)),
            "unexpected error: {err:?}"
        );
        let untouched = std::fs::read(&out).expect("read sentinel");
        assert_eq!(untouched, b"SENTINEL-CONTENT");

        // No leftover staging file.
        assert!(
            staging_files(&tmp).is_empty(),
            "staging file must be cleaned up"
        );

        // Overwrite replaces the target atomically.
        Converter::save_image(&img, &out, ImageFormat::Png, None, true)
            .expect("overwrite encode failed");
        let replaced = Converter::load_image(&out).expect("reload replaced");
        assert!(replaced.width() > 0 && replaced.height() > 0);
        assert!(
            staging_files(&tmp).is_empty(),
            "staging file cleaned up after publish"
        );
    }

    #[test]
    fn load_image_valid_corrupt_valid_batch_outcomes() {
        // Simulates a batch of [valid, corrupt, valid]: both valid items
        // decode and the corrupt item fails with Decode instead of panicking
        // or leaking state.
        let tmp = std::env::temp_dir().join("cookiesincraft_batch_decode_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let valid1 = tmp.join("a.png");
        let corrupt = tmp.join("b.png");
        let valid2 = tmp.join("c.png");
        make_rgba_test_image(&valid1);
        make_rgba_test_image(&valid2);
        std::fs::write(&corrupt, b"not-a-real-png-file----").expect("write corrupt");

        let img1 = Converter::load_image(&valid1).expect("valid item 1 must decode");
        assert!(img1.width() > 0 && img1.height() > 0);

        let err = Converter::load_image(&corrupt).unwrap_err();
        assert!(
            matches!(err, ConverterError::Decode(_)),
            "corrupt item must fail with Decode, got {err:?}"
        );

        let img2 = Converter::load_image(&valid2).expect("valid item 2 must decode");
        assert!(img2.width() > 0 && img2.height() > 0);
    }

    #[test]
    fn load_image_from_bytes_ordinary_input() {
        let tmp = std::env::temp_dir().join("cookiesincraft_bytes_load_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let bytes = std::fs::read(&src).expect("read source");

        let image = Converter::load_image_from_bytes(&bytes).expect("bytes decode");
        assert_eq!((image.width(), image.height()), (100, 100));
    }

    #[test]
    fn oversized_png_metadata_rejected_before_pixels() {
        let tmp = std::env::temp_dir().join("cookiesincraft_png_limit_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let small = tmp.join("small.png");
        make_rgba_test_image(&small);

        let mut bytes = std::fs::read(&small).expect("read png");
        patch_png_dimensions(&mut bytes, 5000, 5000);
        let huge = tmp.join("huge.png");
        std::fs::write(&huge, &bytes).expect("write huge png");

        let err = Converter::load_image(&huge).unwrap_err();
        let msg = err.to_string();
        assert!(
            matches!(&err, ConverterError::Decode(_)),
            "unexpected: {msg}"
        );
        assert!(
            msg.contains("limit") || msg.contains("dimension") || msg.contains("exceed"),
            "expected a pre-decode limit error, got: {msg}"
        );

        // The in-memory path shares the same precheck.
        let err = Converter::load_image_from_bytes(&bytes).unwrap_err();
        assert!(
            matches!(err, ConverterError::Decode(_)),
            "unexpected: {err}"
        );
    }

    #[test]
    fn oversized_png_rgba16_buffer_rejected_before_pixels() {
        let tmp = std::env::temp_dir().join("cookiesincraft_png_rgba16_limit_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let small = tmp.join("small.png");
        let image = DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            1,
            1,
            image::Rgba([1u16, 2, 3, 65535]),
        ));
        image.save(&small).expect("write rgba16 png");

        // 3000 x 3000 x 8 bytes = ~72 MB, over the 64 MiB decoded limit while
        // still inside the 4096 dimension limit.
        let mut bytes = std::fs::read(&small).expect("read png");
        patch_png_dimensions(&mut bytes, 3000, 3000);
        let huge = tmp.join("huge16.png");
        std::fs::write(&huge, &bytes).expect("write huge png");

        let err = Converter::load_image(&huge).unwrap_err();
        let msg = err.to_string();
        assert!(
            matches!(&err, ConverterError::Decode(_)),
            "unexpected: {msg}"
        );
        assert!(
            msg.contains("64 MiB"),
            "expected decoded-byte limit, got: {msg}"
        );
    }

    #[test]
    fn oversized_jpeg_metadata_rejected_before_pixels() {
        let tmp = std::env::temp_dir().join("cookiesincraft_jpeg_limit_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let small = tmp.join("small.jpg");
        let image = DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
            8,
            8,
            image::Rgb([10u8, 20, 30]),
        ));
        image.save(&small).expect("write jpeg");

        let mut bytes = std::fs::read(&small).expect("read jpeg");
        patch_jpeg_dimensions(&mut bytes, 5000, 5000);
        let huge = tmp.join("huge.jpg");
        std::fs::write(&huge, &bytes).expect("write huge jpeg");

        let err = Converter::load_image(&huge).unwrap_err();
        let msg = err.to_string();
        assert!(
            matches!(&err, ConverterError::Decode(_)),
            "unexpected: {msg}"
        );
        assert!(
            msg.contains("exceed"),
            "expected dimension limit, got: {msg}"
        );
    }

    #[test]
    fn publish_no_clobber_never_replaces_target() {
        let tmp = std::env::temp_dir().join("cookiesincraft_publish_noclobber_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let staging = tmp.join("staged.part");
        let target = tmp.join("target.png");
        std::fs::write(&staging, b"NEW-CONTENT").expect("write staging");

        // Target absent: the hard-link publication succeeds and leaves the
        // staging entry for the caller to clean up.
        Converter::publish_no_clobber(&staging, &target).expect("first publish");
        assert_eq!(std::fs::read(&target).expect("read target"), b"NEW-CONTENT");
        assert!(staging.exists(), "caller removes the staging link");

        // Target now exists: publication must fail without replacing it.
        let err = Converter::publish_no_clobber(&staging, &target).unwrap_err();
        assert!(matches!(err, ConverterError::Io(_)), "unexpected: {err}");
        assert!(
            err.to_string().contains("already exists"),
            "unexpected: {err}"
        );
        assert_eq!(std::fs::read(&target).expect("read target"), b"NEW-CONTENT");
    }

    #[test]
    fn save_image_error_preserves_existing_target() {
        let tmp = std::env::temp_dir().join("cookiesincraft_publish_error_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let image = Converter::load_image(&src).expect("load source");

        // A directory at the target path makes the final rename fail; the
        // pre-existing entry must survive and no staging file may remain.
        let target = tmp.join("out.png");
        std::fs::create_dir(&target).expect("create target directory");
        let err = Converter::save_image(&image, &target, ImageFormat::Png, None, true).unwrap_err();
        assert!(matches!(err, ConverterError::Io(_)), "unexpected: {err}");
        assert!(target.is_dir(), "existing target must not be destroyed");
        assert!(
            staging_files(&tmp).is_empty(),
            "staging file must be cleaned up"
        );
    }

    // Tests in Converter tests module, `use super::*` already in scope.

    #[test]
    fn test_webp_decode_oversize_fixture() {
        let bytes = b"RIFF\x12\0\0\0WEBPVP8L\x05\0\0\0\x2f\0\x10\0\0\0";
        let result = Converter::load_image_from_bytes(bytes);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("4097x1") && err.contains("exceed"),
            "error should mention 4097x1 and exceed, got: {err}"
        );
    }

    #[test]
    fn test_webp_decode_valid() {
        let rgba = [255u8, 128, 64, 32].repeat(6);
        let mut bytes = Vec::new();
        image_webp::WebPEncoder::new(&mut bytes)
            .encode(&rgba, 3, 2, image_webp::ColorType::Rgba8)
            .unwrap();
        let result = Converter::load_image_from_bytes(&bytes).unwrap();
        assert_eq!((result.width(), result.height()), (3, 2));
        let raw = result.to_rgba8().into_raw();
        assert_eq!(raw, rgba);
    }
}
