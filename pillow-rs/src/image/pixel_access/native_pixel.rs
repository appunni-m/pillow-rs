use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};
use crate::FormattedPixelValue;

/// Reads one pixel from its exact stored variant without an intermediate mode.
pub(super) fn formatted_pixel(
    image: &DynamicImage,
    x: u32,
    y: u32,
) -> Result<FormattedPixelValue, PilError> {
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
        // TODO(pillow-rs#3ptb): cover typed CoreOnly GetPixel values in parity.
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

