//! Palette lookups shared by the native P and PA Convert strategies.

use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, LumaA, Mode};

use super::super::ops::{ConvertWrapperMetadata, PreparedConvertOutput};

#[inline]
pub(crate) fn color(index: u8, palette: PaletteInput<'_>) -> [u8; 3] {
    let start = usize::from(index) * 3;
    let Some(rgb) = palette.rgb else {
        return [0; 3];
    };
    [
        rgb.get(start).copied().unwrap_or(0),
        rgb.get(start + 1).copied().unwrap_or(0),
        rgb.get(start + 2).copied().unwrap_or(0),
    ]
}

#[inline]
pub(crate) fn alpha(index: u8, palette: PaletteInput<'_>) -> u8 {
    palette
        .alpha
        .get(usize::from(index))
        .copied()
        .unwrap_or(u8::MAX)
}

pub(crate) fn web_alpha_from_rgb_samples(
    width: u32,
    height: u32,
    rgb_samples: &[u8],
    dither: bool,
    alpha: impl FnMut(usize) -> u8,
) -> Result<PreparedConvertOutput, PilError> {
    web_palette_from_rgb_samples(
        width,
        height,
        rgb_samples,
        dither,
        Mode::ImagePaletteAlpha8,
        alpha,
        ConvertWrapperMetadata::ExifOnly,
    )
}

pub(crate) fn web_palette_from_pixels(
    width: u32,
    height: u32,
    pixels: impl Iterator<Item = [u8; 3]>,
    dither: bool,
    target: Mode,
    metadata: ConvertWrapperMetadata,
) -> Result<PreparedConvertOutput, PilError> {
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    let rgb_length = pixel_count
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    let mut rgb_samples = Vec::with_capacity(rgb_length);
    for pixel in pixels.take(pixel_count) {
        rgb_samples.extend_from_slice(&pixel);
    }
    if rgb_samples.len() != rgb_length {
        return Err(PilError::ValueError("palette conversion failed".into()));
    }
    web_palette_from_rgb_samples(
        width,
        height,
        &rgb_samples,
        dither,
        target,
        |_| u8::MAX,
        metadata,
    )
}

pub(crate) fn web_palette_from_rgb_samples(
    width: u32,
    height: u32,
    rgb_samples: &[u8],
    dither: bool,
    target: Mode,
    alpha: impl FnMut(usize) -> u8,
    metadata: ConvertWrapperMetadata,
) -> Result<PreparedConvertOutput, PilError> {
    let (indices, palette) = crate::compute::pool_cpu::helpers::quantization::web_palette_quantize(
        rgb_samples,
        width,
        height,
        dither,
    )?;
    match target {
        Mode::ImagePalette8 => {
            palette_index_output(width, height, indices, palette, None, metadata)
        }
        Mode::ImagePaletteAlpha8 => {
            palette_alpha_output(width, height, indices, palette, alpha, metadata)
        }
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

pub(crate) fn palette_index_output(
    width: u32,
    height: u32,
    indices: Vec<u8>,
    palette: Vec<u8>,
    palette_alpha: Option<Vec<u8>>,
    metadata: ConvertWrapperMetadata,
) -> Result<PreparedConvertOutput, PilError> {
    let image = GrayImage::from_raw(width, height, indices)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    Ok(PreparedConvertOutput {
        image: DynamicImage::ImagePalette8(image),
        palette: Some(palette),
        palette_alpha,
        wrapper_metadata: Some(metadata),
    })
}

pub(crate) fn palette_alpha_output(
    width: u32,
    height: u32,
    indices: Vec<u8>,
    palette: Vec<u8>,
    mut alpha: impl FnMut(usize) -> u8,
    metadata: ConvertWrapperMetadata,
) -> Result<PreparedConvertOutput, PilError> {
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("palette conversion failed".into()))?;
    if indices.len() != pixel_count {
        return Err(PilError::ValueError("palette conversion failed".into()));
    }
    let mut image = GrayAlphaImage::new(width, height);
    for (position, (destination, index)) in image.pixels_mut().zip(indices.into_iter()).enumerate()
    {
        *destination = LumaA([index, alpha(position)]);
    }
    Ok(PreparedConvertOutput {
        image: DynamicImage::ImagePaletteAlpha8(image),
        palette: Some(palette),
        palette_alpha: None,
        wrapper_metadata: Some(metadata),
    })
}
