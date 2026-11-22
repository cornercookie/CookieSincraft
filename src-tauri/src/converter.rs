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
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use image::DynamicImage;
use image::ImageOutputFormat;

/// Default JPEG quality used when no quality is provided.
const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Maximum width/height of a single ICO entry.
const ICO_MAX_DIM: u32 = 256;

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
            ConverterError::Unknown(msg) => write!(f, "converter error: {msg}"),
        }
    }
}

impl std::error::Error for ConverterError {}

/// Image encode/decode and export.
///
/// Stateless utility holding no configuration; all parameters are passed per
/// call.
pub struct Converter;

impl Converter {
    /// Read an image from `path`, guessing its format from the file contents.
    ///
    /// Supports PNG, JPEG, WebP and AVIF (AVIF decoding requires the `image`
    /// crate's `avif` feature).
    pub fn load_image(path: &Path) -> Result<DynamicImage, ConverterError> {
        let file = File::open(path).map_err(|e| {
            ConverterError::Io(format!("failed to open image {path:?}: {e}"))
        })?;
        let reader = BufReader::new(file);
        let dynamic = image::io::Reader::new(reader)
            .with_guessed_format()
            .map_err(|e| {
                ConverterError::Decode(format!(
                    "could not determine image format of {path:?}: {e}"
                ))
            })?
            .decode()
            .map_err(|e| {
                ConverterError::Decode(format!("failed to decode image at {path:?}: {e}"))
            })?;
        Ok(dynamic)
    }

    /// Write `image` to `path` in `format`.
    ///
    /// For `ImageFormat::Jpeg`, `jpeg_quality` must be `None` (default 85) or
    /// a value in 50..=100; any other value returns an error. For all other
    /// formats `jpeg_quality` is ignored.
    pub fn save_image(
        image: &DynamicImage,
        path: &Path,
        format: ImageFormat,
        jpeg_quality: Option<u8>,
    ) -> Result<(), ConverterError> {
        let quality = Self::resolve_jpeg_quality(format, jpeg_quality)?;
        let prepared = Self::prepare_for_format(image, format);
        let image = prepared.as_ref();

        let file = File::create(path).map_err(|e| {
            ConverterError::Io(format!("failed to create output image {path:?}: {e}"))
        })?;
        let mut writer = BufWriter::new(file);

        let output_format = match format {
            ImageFormat::Png => ImageOutputFormat::Png,
            ImageFormat::Jpeg => ImageOutputFormat::Jpeg(quality),
            ImageFormat::WebP => ImageOutputFormat::WebP,
            ImageFormat::Gif => ImageOutputFormat::Gif,
            ImageFormat::Tiff => ImageOutputFormat::Tiff,
            ImageFormat::Bmp => ImageOutputFormat::Bmp,
            ImageFormat::Ico => ImageOutputFormat::Ico,
            ImageFormat::Pnm => ImageOutputFormat::Pnm(image::codecs::pnm::PnmSubtype::Pixmap(image::codecs::pnm::SampleEncoding::Binary)),
        };

        image
            .write_to(&mut writer, output_format)
            .map_err(|e| ConverterError::Encode(format!("failed to encode image to {path:?}: {e}")))?;
        Ok(())
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
    fn resolve_jpeg_quality(format: ImageFormat, jpeg_quality: Option<u8>) -> Result<u8, ConverterError> {
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
            image::Rgba([(x.wrapping_mul(2)) as u8, (y.wrapping_mul(2)) as u8, 128, 255])
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

    #[test]
    fn roundtrip_all_supported_formats() {
        let tmp = std::env::temp_dir().join("cookiesincraft_convert_test");
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
            Converter::save_image(&img, &out, *fmt, Some(85))
                .unwrap_or_else(|e| panic!("{ext} encode failed: {e}"));
            assert!(out.exists(), "{ext} output file missing");

            // Round-trip: the output must be decodable again.
            let reloaded = Converter::load_image(&out)
                .unwrap_or_else(|e| panic!("{ext} reload failed: {e}"));
            assert!(reloaded.width() > 0 && reloaded.height() > 0, "{ext} reload empty");
        }
    }

    #[test]
    fn pnm_roundtrip_with_rgb_source() {
        // PNM (PPM) does not support alpha channels, so use an RGB source.
        let tmp = std::env::temp_dir().join("cookiesincraft_pnm_test");
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgb_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");
        let out = tmp.join("out.pnm");
        Converter::save_image(&img, &out, ImageFormat::Pnm, None)
            .expect("pnm encode failed");
        assert!(out.exists());
        let reloaded = Converter::load_image(&out).expect("pnm reload failed");
        assert!(reloaded.width() > 0 && reloaded.height() > 0);
    }

    #[test]
    fn ico_large_source_is_downscaled_to_fit() {
        // ICO entries are limited to 256x256; real photos are much larger.
        let tmp = std::env::temp_dir().join("cookiesincraft_ico_large_test");
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let img = DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(
            1024,
            512,
            image::Rgba([10, 20, 30, 255]),
        ));
        let out = tmp.join("out.ico");
        Converter::save_image(&img, &out, ImageFormat::Ico, None).expect("ico encode failed");
        let reloaded = Converter::load_image(&out).expect("ico reload failed");
        assert!(reloaded.width() <= 256 && reloaded.height() <= 256);
        assert_eq!(reloaded.width(), 256);
        assert_eq!(reloaded.height(), 128);
    }

    #[test]
    fn pnm_accepts_rgba_source() {
        // Typical PNG inputs carry alpha; PPM must still be writable.
        let tmp = std::env::temp_dir().join("cookiesincraft_pnm_rgba_test");
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");
        let out = tmp.join("out.ppm");
        Converter::save_image(&img, &out, ImageFormat::Pnm, None).expect("pnm encode failed");
        let reloaded = Converter::load_image(&out).expect("pnm reload failed");
        assert_eq!((reloaded.width(), reloaded.height()), (100, 100));
    }

    #[test]
    fn invalid_quality_is_rejected() {
        let tmp = std::env::temp_dir().join("cookiesincraft_quality_test");
        std::fs::create_dir_all(&tmp).expect("temp dir");
        let src = tmp.join("src.png");
        make_rgba_test_image(&src);
        let img = Converter::load_image(&src).expect("load source");

        let out = tmp.join("out.jpg");
        let err = Converter::save_image(&img, &out, ImageFormat::Jpeg, Some(49))
            .unwrap_err();
        assert!(matches!(err, ConverterError::InvalidQuality(_)), "unexpected error: {err:?}");

        // Non-JPEG formats ignore quality entirely (no error for out-of-range).
        let out2 = tmp.join("out.png");
        Converter::save_image(&img, &out2, ImageFormat::Png, Some(49))
            .expect("png should ignore quality");
    }
}
