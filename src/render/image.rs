// Copyright (c) DeltaForks Labs
// Licensed under the MIT License OR Apache 2.0.

use std::fmt;

use skia_safe::Image;

use super::image_headers::encoded_image_dimensions;

pub(crate) const MAX_IMAGE_DECODE_WIDTH: u32 = 8192;
pub(crate) const MAX_IMAGE_DECODE_HEIGHT: u32 = 8192;
pub(crate) const MAX_IMAGE_DECODE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_ENCODED_IMAGE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageDecodeLimits {
    pub max_encoded_bytes: usize,
    pub max_width: u32,
    pub max_height: u32,
    pub max_alloc_bytes: u64,
}

impl Default for ImageDecodeLimits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: MAX_ENCODED_IMAGE_BYTES,
            max_width: MAX_IMAGE_DECODE_WIDTH,
            max_height: MAX_IMAGE_DECODE_HEIGHT,
            max_alloc_bytes: MAX_IMAGE_DECODE_ALLOC_BYTES,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RutterDecodedImage {
    pub image: Image,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImageDecodeError {
    EncodedBytesExceeded {
        encoded_bytes: usize,
        max_encoded_bytes: usize,
    },
    InvalidData {
        expected: &'static str,
    },
    DimensionsExceeded {
        width: u32,
        height: u32,
        max_width: u32,
        max_height: u32,
    },
    AllocationExceeded {
        width: u32,
        height: u32,
        alloc_bytes: u64,
        max_alloc_bytes: u64,
    },
}

impl fmt::Display for ImageDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EncodedBytesExceeded {
                encoded_bytes,
                max_encoded_bytes,
            } => write_encoded_bytes_limit_error(f, *encoded_bytes, *max_encoded_bytes),
            Self::InvalidData { expected } => write_invalid_image_data(f, expected),
            Self::DimensionsExceeded {
                width,
                height,
                max_width,
                max_height,
            } => write_dimension_limit_error(f, *width, *height, *max_width, *max_height),
            Self::AllocationExceeded {
                width,
                height,
                alloc_bytes,
                max_alloc_bytes,
            } => write_allocation_limit_error(f, *width, *height, *alloc_bytes, *max_alloc_bytes),
        }
    }
}

impl std::error::Error for ImageDecodeError {}

pub(crate) fn decode_rutter_image(data: &[u8]) -> Result<RutterDecodedImage, ImageDecodeError> {
    decode_rutter_image_with_limits(data, ImageDecodeLimits::default())
}

pub(crate) fn decode_rutter_image_with_limits(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<RutterDecodedImage, ImageDecodeError> {
    validate_encoded_bytes(data, limits)?;
    validate_known_encoded_dimensions(data, limits)?;
    decode_rutter_image_impl(data, limits)
}

#[cfg(not(feature = "image-rs-decoder"))]
fn decode_rutter_image_impl(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<RutterDecodedImage, ImageDecodeError> {
    use skia_safe::Data;

    let image = Image::from_encoded(Data::new_copy(data)).ok_or(ImageDecodeError::InvalidData {
        expected: "Skia-supported encoded image data",
    })?;
    validate_decoded_dimensions(image.width(), image.height(), limits)?;
    Ok(RutterDecodedImage {
        width: image.width(),
        height: image.height(),
        image,
    })
}

#[cfg(feature = "image-rs-decoder")]
fn decode_rutter_image_impl(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<RutterDecodedImage, ImageDecodeError> {
    let (width, height) = read_image_crate_dimensions(data, limits)?;
    validate_decoded_dimensions(width as i32, height as i32, limits)?;
    let dynamic = decode_image_crate_data(data, limits)?;
    let rgba = dynamic.to_rgba8();
    let (width, height) = (rgba.width() as i32, rgba.height() as i32);
    skia_image_from_rgba(width, height, rgba.into_raw())
}

#[cfg(feature = "image-rs-decoder")]
fn read_image_crate_dimensions(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<(u32, u32), ImageDecodeError> {
    image_crate_reader(data, limits)?
        .into_dimensions()
        .map_err(|_| image_crate_invalid_data())
}

#[cfg(feature = "image-rs-decoder")]
fn decode_image_crate_data(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<::image::DynamicImage, ImageDecodeError> {
    image_crate_reader(data, limits)?
        .decode()
        .map_err(|_| image_crate_invalid_data())
}

#[cfg(feature = "image-rs-decoder")]
fn image_crate_reader<'a>(
    data: &'a [u8],
    limits: ImageDecodeLimits,
) -> Result<::image::ImageReader<std::io::Cursor<&'a [u8]>>, ImageDecodeError> {
    let mut reader = ::image::ImageReader::new(std::io::Cursor::new(data));
    reader.limits(image_crate_dimension_limits(limits));
    reader
        .with_guessed_format()
        .map_err(|_| image_crate_invalid_data())
}

#[cfg(feature = "image-rs-decoder")]
fn image_crate_dimension_limits(limits: ImageDecodeLimits) -> ::image::Limits {
    let mut image_limits = ::image::Limits::default();
    image_limits.max_image_width = Some(limits.max_width);
    image_limits.max_image_height = Some(limits.max_height);
    image_limits.max_alloc = Some(limits.max_alloc_bytes);
    image_limits
}

#[cfg(feature = "image-rs-decoder")]
fn skia_image_from_rgba(
    width: i32,
    height: i32,
    raw: Vec<u8>,
) -> Result<RutterDecodedImage, ImageDecodeError> {
    let mut bitmap = rgba_bitmap(width, height)?;
    copy_rgba_pixels(&mut bitmap, &raw)?;
    let image = raster_image_from_bitmap(&bitmap)?;
    Ok(RutterDecodedImage {
        image,
        width,
        height,
    })
}

#[cfg(feature = "image-rs-decoder")]
fn rgba_bitmap(width: i32, height: i32) -> Result<skia_safe::Bitmap, ImageDecodeError> {
    use skia_safe::{AlphaType, Bitmap, ColorType, ImageInfo};

    let mut bitmap = Bitmap::new();
    if !bitmap.set_info(
        &ImageInfo::new(
            (width, height),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        ),
        None,
    ) {
        return Err(ImageDecodeError::InvalidData {
            expected: "Skia bitmap-compatible RGBA image data",
        });
    }
    bitmap.alloc_pixels();
    Ok(bitmap)
}

#[cfg(feature = "image-rs-decoder")]
fn copy_rgba_pixels(bitmap: &mut skia_safe::Bitmap, raw: &[u8]) -> Result<(), ImageDecodeError> {
    let pixels = bitmap.pixels();
    if pixels.is_null() {
        return Err(ImageDecodeError::InvalidData {
            expected: "allocated Skia bitmap pixels",
        });
    }
    unsafe {
        std::ptr::copy_nonoverlapping(raw.as_ptr(), pixels as *mut u8, raw.len());
    }
    Ok(())
}

#[cfg(feature = "image-rs-decoder")]
fn raster_image_from_bitmap(bitmap: &skia_safe::Bitmap) -> Result<Image, ImageDecodeError> {
    skia_safe::images::raster_from_bitmap(bitmap).ok_or(ImageDecodeError::InvalidData {
        expected: "Skia raster image from decoded bitmap",
    })
}

#[cfg(feature = "image-rs-decoder")]
fn image_crate_invalid_data() -> ImageDecodeError {
    ImageDecodeError::InvalidData {
        expected: "image-rs-supported encoded image data",
    }
}

fn validate_decoded_dimensions(
    width: i32,
    height: i32,
    limits: ImageDecodeLimits,
) -> Result<(), ImageDecodeError> {
    let (width, height) = validate_positive_dimensions(width, height)?;
    validate_dimension_bounds(width, height, limits)?;
    validate_allocation_budget(width, height, limits)
}

fn validate_encoded_bytes(data: &[u8], limits: ImageDecodeLimits) -> Result<(), ImageDecodeError> {
    if data.len() > limits.max_encoded_bytes {
        return Err(ImageDecodeError::EncodedBytesExceeded {
            encoded_bytes: data.len(),
            max_encoded_bytes: limits.max_encoded_bytes,
        });
    }
    Ok(())
}

fn validate_known_encoded_dimensions(
    data: &[u8],
    limits: ImageDecodeLimits,
) -> Result<(), ImageDecodeError> {
    let Some((width, height)) = encoded_image_dimensions(data) else {
        return Ok(());
    };
    validate_dimension_bounds(width, height, limits)?;
    validate_allocation_budget(width, height, limits)
}

fn validate_positive_dimensions(width: i32, height: i32) -> Result<(u32, u32), ImageDecodeError> {
    if width <= 0 || height <= 0 {
        return Err(ImageDecodeError::InvalidData {
            expected: "image with positive dimensions",
        });
    }
    Ok((width as u32, height as u32))
}

fn validate_dimension_bounds(
    width: u32,
    height: u32,
    limits: ImageDecodeLimits,
) -> Result<(), ImageDecodeError> {
    if width > limits.max_width || height > limits.max_height {
        return Err(ImageDecodeError::DimensionsExceeded {
            width,
            height,
            max_width: limits.max_width,
            max_height: limits.max_height,
        });
    }
    Ok(())
}

fn validate_allocation_budget(
    width: u32,
    height: u32,
    limits: ImageDecodeLimits,
) -> Result<(), ImageDecodeError> {
    let alloc_bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    if alloc_bytes > limits.max_alloc_bytes {
        return Err(ImageDecodeError::AllocationExceeded {
            width,
            height,
            alloc_bytes,
            max_alloc_bytes: limits.max_alloc_bytes,
        });
    }
    Ok(())
}

fn write_invalid_image_data(f: &mut fmt::Formatter<'_>, expected: &str) -> fmt::Result {
    write!(
        f,
        "invalid image data: offending value `encoded bytes`, expected {expected}"
    )
}

fn write_encoded_bytes_limit_error(
    f: &mut fmt::Formatter<'_>,
    encoded_bytes: usize,
    max_encoded_bytes: usize,
) -> fmt::Result {
    write!(
        f,
        "encoded image bytes exceed limit: offending value `{encoded_bytes}` bytes, expected <= {max_encoded_bytes} bytes"
    )
}

fn write_dimension_limit_error(
    f: &mut fmt::Formatter<'_>,
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> fmt::Result {
    write!(
        f,
        "image dimensions exceed limits: offending value `{width}x{height}`, expected width <= {max_width} and height <= {max_height}"
    )
}

fn write_allocation_limit_error(
    f: &mut fmt::Formatter<'_>,
    width: u32,
    height: u32,
    alloc_bytes: u64,
    max_alloc_bytes: u64,
) -> fmt::Result {
    write!(
        f,
        "image allocation exceeds limit: offending value `{width}x{height}` requires {alloc_bytes} bytes, expected <= {max_alloc_bytes} bytes"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ImageDecodeError, ImageDecodeLimits, decode_rutter_image, decode_rutter_image_with_limits,
    };

    #[cfg(not(feature = "image-rs-decoder"))]
    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x08,
            0x99, 0x63, 0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99,
            0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ]
    }

    #[cfg(feature = "image-rs-decoder")]
    fn tiny_png() -> Vec<u8> {
        use ::image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
        use std::io::Cursor;

        let image = RgbaImage::from_pixel(1, 1, Rgba([0x12, 0x34, 0x56, 0xff]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn decode_rutter_image_accepts_small_png_with_default_limits() {
        let image = decode_rutter_image(&tiny_png()).unwrap();
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
    }

    #[test]
    fn decode_rutter_image_rejects_small_png_when_alloc_budget_is_too_low() {
        let result = decode_rutter_image_with_limits(
            &tiny_png(),
            ImageDecodeLimits {
                max_encoded_bytes: 1024,
                max_width: 16,
                max_height: 16,
                max_alloc_bytes: 1,
            },
        );
        assert!(matches!(
            result,
            Err(ImageDecodeError::AllocationExceeded { .. })
        ));
    }

    #[test]
    fn decode_rutter_image_rejects_invalid_data() {
        let result = decode_rutter_image(b"not an image");
        assert!(matches!(result, Err(ImageDecodeError::InvalidData { .. })));
    }

    #[test]
    fn decode_rutter_image_rejects_encoded_input_before_copying() {
        let result = decode_rutter_image_with_limits(
            &[0; 8],
            ImageDecodeLimits {
                max_encoded_bytes: 7,
                max_width: 16,
                max_height: 16,
                max_alloc_bytes: 1024,
            },
        );

        assert!(matches!(
            result,
            Err(ImageDecodeError::EncodedBytesExceeded { .. })
        ));
    }

    #[test]
    fn decode_rutter_image_rejects_png_dimensions_before_skia_decode() {
        let result = decode_rutter_image_with_limits(
            &tiny_png(),
            ImageDecodeLimits {
                max_encoded_bytes: 1024,
                max_width: 0,
                max_height: 16,
                max_alloc_bytes: 1024,
            },
        );

        assert!(matches!(
            result,
            Err(ImageDecodeError::DimensionsExceeded { .. })
        ));
    }
}
