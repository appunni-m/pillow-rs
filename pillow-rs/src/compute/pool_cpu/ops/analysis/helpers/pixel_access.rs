use crate::error::PilError;
use crate::image::FormattedPixelValue;
use crate::raster::{DynamicImage, GenericImageView, RgbaImage};

/// Reads one pixel from its exact stored variant without an intermediate mode.
pub(crate) fn formatted_pixel(
    image: &DynamicImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
    // TODO(pillow-rs#3ptb): move this variant dispatch into exact-mode Default strategies.
    match image {
        DynamicImage::One(image)
        | DynamicImage::ImageLuma8(image)
        | DynamicImage::ImagePalette8(image) => {
            Ok(FormattedPixelValue::Scalar(image.get_pixel(x, y).0[0]))
        }
        DynamicImage::ImageLumaA8(image)
        | DynamicImage::ImageLaPremultiplied8(image)
        | DynamicImage::ImagePaletteAlpha8(image) => {
            let [luma, alpha] = image.get_pixel(x, y).0;
            Ok(FormattedPixelValue::Components(vec![luma, alpha]))
        }
        DynamicImage::ImageRgb8(image)
        | DynamicImage::ImageYCbCr8(image)
        | DynamicImage::ImageHsv8(image) => {
            Ok(FormattedPixelValue::Components(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageLab8(image) => {
            let [luma, a, b] = image.get_pixel(x, y).0;
            Ok(FormattedPixelValue::Components(vec![
                luma,
                a.wrapping_sub(128),
                b.wrapping_sub(128),
            ]))
        }
        DynamicImage::ImageRgba8(image)
        | DynamicImage::ImageRgbPremultiplied8(image)
        | DynamicImage::ImageRgbx8(image)
        | DynamicImage::ImageCmyk8(image) => {
            Ok(FormattedPixelValue::Components(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageI32(image) => {
            let sample = scalar_bytes(image, x, y)?;
            Ok(FormattedPixelValue::Integer(i32::from_le_bytes(sample)))
        }
        DynamicImage::ImageF32(image) => {
            let sample = scalar_bytes(image, x, y)?;
            Ok(FormattedPixelValue::Float(f64::from(f32::from_le_bytes(sample))))
        }
        DynamicImage::ImageI16(image)
        | DynamicImage::ImageI16L(image)
        | DynamicImage::ImageI16B(image)
        | DynamicImage::ImageI16N(image) => {
            let index = pixel_index(image.width(), x, y)?;
            let sample = image.as_raw().get(index).copied().ok_or_else(|| {
                PilError::InternalError("I;16 pixel index out of bounds".into())
            })?;
            Ok(FormattedPixelValue::Integer(i32::from(sample)))
        }
        // NO_PILLOW_SUPPORT: internal high-depth modes expose native samples.
        DynamicImage::ImageLumaA16(image) => {
            Ok(FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageRgb16(image) => {
            Ok(FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageRgba16(image) => {
            Ok(FormattedPixelValue::U16Components(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageRgb32F(image) => {
            Ok(FormattedPixelValue::FloatComponents(image.get_pixel(x, y).0.to_vec()))
        }
        DynamicImage::ImageRgba32F(image) => {
            Ok(FormattedPixelValue::FloatComponents(image.get_pixel(x, y).0.to_vec()))
        }
    }
}

/// Reads one pixel using the Rust/JavaScript accessor's fixed RGBA8 shape.
// NO_PILLOW_SUPPORT: Pillow's `getpixel` preserves each mode's scalar/tuple shape.
pub(crate) fn rgba_pixel(image: &DynamicImage, x: u32, y: u32) -> [u8; 4] {
    image.get_pixel(x, y).0
}

/// Reads a direct RGB tuple without allocating the formatted component vector.
pub(crate) fn rgb_pixel(image: &DynamicImage, x: u32, y: u32) -> Option<[u8; 3]> {
    let DynamicImage::ImageRgb8(rgb) = image else {
        return None;
    };
    Some(rgb.get_pixel(x, y).0)
}

/// Returns the number of host value bytes produced by one formatted pixel.
pub(crate) fn formatted_pixel_bytes(value: &FormattedPixelValue) -> usize {
    match value {
        FormattedPixelValue::Scalar(_) => 1,
        FormattedPixelValue::Integer(_) => std::mem::size_of::<i32>(),
        FormattedPixelValue::Float(_) => std::mem::size_of::<f64>(),
        FormattedPixelValue::Components(values) => values.len(),
        FormattedPixelValue::U16Components(values) => {
            values.len().saturating_mul(std::mem::size_of::<u16>())
        }
        FormattedPixelValue::FloatComponents(values) => {
            values.len().saturating_mul(std::mem::size_of::<f32>())
        }
        FormattedPixelValue::InlineComponents { bands, .. } => usize::from(*bands),
    }
}

fn pixel_index(width: u32, x: u32, y: u32) -> Result<usize, PilError> {
    (y as usize)
        .checked_mul(width as usize)
        .and_then(|row| row.checked_add(x as usize))
        .ok_or_else(|| PilError::InternalError("pixel index overflow".into()))
}

fn scalar_bytes(image: &RgbaImage, x: u32, y: u32) -> Result<[u8; 4], PilError> {
    let offset = pixel_index(image.width(), x, y)?
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
    let end = offset
        .checked_add(4)
        .ok_or_else(|| PilError::InternalError("scalar pixel offset overflow".into()))?;
    let sample = image.as_raw().get(offset..end).ok_or_else(|| {
        PilError::InternalError("scalar pixel index out of bounds".into())
    })?;
    Ok([sample[0], sample[1], sample[2], sample[3]])
}
