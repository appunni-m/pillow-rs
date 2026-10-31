//! Native sample arithmetic shared by Convert's exact target strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode, RgbImage, Rgba, RgbaImage};

#[inline]
pub(crate) fn premultiply_byte(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

#[inline]
pub(crate) fn unpremultiply_byte(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        value
    } else {
        ((u32::from(value) * 255 / u32::from(alpha)).min(255)) as u8
    }
}

pub(crate) fn scalar_i32(image: &DynamicImage) -> DynamicImage {
    if let Some((width, height, samples)) = native_luma16(image) {
        let output = RgbaImage::from_fn(width, height, |x, y| {
            Rgba(samples[(y * width + x) as usize].to_le_bytes())
        });
        return DynamicImage::ImageI32(output);
    }
    scalar_i32_rgb8(&image.to_rgb8())
}

pub(crate) fn scalar_i32_rgb8(rgb: &RgbImage) -> DynamicImage {
    let output = RgbaImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        let pixel = rgb.get_pixel(x, y);
        Rgba(scalar_i32_rgb(pixel[0], pixel[1], pixel[2]))
    });
    DynamicImage::ImageI32(output)
}

#[inline]
pub(crate) fn scalar_i32_rgb(red: u8, green: u8, blue: u8) -> [u8; 4] {
    let value = (19_595i32 * i32::from(red)
        + 38_470i32 * i32::from(green)
        + 7_471i32 * i32::from(blue)
        + 32_768)
        >> 16;
    value.to_le_bytes()
}

pub(crate) fn scalar_f32(image: &DynamicImage) -> DynamicImage {
    if let Some((width, height, samples)) = native_luma16(image) {
        let output = RgbaImage::from_fn(width, height, |x, y| {
            Rgba(f32::from(samples[(y * width + x) as usize]).to_le_bytes())
        });
        return DynamicImage::ImageF32(output);
    }
    scalar_f32_rgb8(&image.to_rgb8())
}

pub(crate) fn scalar_f32_rgb8(rgb: &RgbImage) -> DynamicImage {
    let output = RgbaImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        let pixel = rgb.get_pixel(x, y);
        Rgba(scalar_f32_rgb(pixel[0], pixel[1], pixel[2]))
    });
    DynamicImage::ImageF32(output)
}

#[inline]
pub(crate) fn scalar_f32_rgb(red: u8, green: u8, blue: u8) -> [u8; 4] {
    let sum = i32::from(red) * 299 + i32::from(green) * 587 + i32::from(blue) * 114;
    (sum as f32 / 1000.0_f32).to_le_bytes()
}

pub(crate) fn luma16(image: &DynamicImage, target: Mode) -> Result<DynamicImage, PilError> {
    let (width, height, samples) = match image {
        DynamicImage::One(source) => (
            source.width(),
            source.height(),
            source.pixels().map(|pixel| u16::from(if pixel[0] != 0 { 255 } else { pixel[0] }))
                .collect::<Vec<_>>(),
        ),
        DynamicImage::ImageLuma8(source) => (
            source.width(),
            source.height(),
            source.pixels().map(|pixel| u16::from(pixel[0])).collect::<Vec<_>>(),
        ),
        _ => {
            return Err(PilError::ValueError(
                "I;16 conversion requires L or binary input".into(),
            ));
        }
    };
    let output = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples)
        .ok_or_else(|| PilError::InternalError("I;16 conversion shape mismatch".into()))?;
    Ok(match target {
        Mode::ImageI16 => DynamicImage::ImageI16(output),
        Mode::ImageI16L => DynamicImage::ImageI16L(output),
        Mode::ImageI16B => DynamicImage::ImageI16B(output),
        Mode::ImageI16N => DynamicImage::ImageI16N(output),
        _ => return Err(PilError::InternalError("invalid I;16 Convert target".into())),
    })
}

fn native_luma16(image: &DynamicImage) -> Option<(u32, u32, &[u16])> {
    match image {
        DynamicImage::ImageI16(source)
        | DynamicImage::ImageI16L(source)
        | DynamicImage::ImageI16B(source)
        | DynamicImage::ImageI16N(source) => Some((source.width(), source.height(), source.as_raw())),
        _ => None,
    }
}
