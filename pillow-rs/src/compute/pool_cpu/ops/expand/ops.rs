//! CPU Expand mode strategies over native raster storage.

use crate::error::PilError;
use crate::image::Mode;
use crate::raster::DynamicImage;

#[path = "kernels/mod.rs"]
mod kernels;
use kernels::{expand_f32, expand_native_bytes, expand_u16, expanded_dimensions};

#[derive(Clone, Copy)]
enum ExpandStrategy {
    NativeBytes(usize),
    NativeU16(usize),
    NativeF32(usize),
}

/// Declare the native sample layout used by every logical mode.
const fn mode_strategy(mode: Mode) -> ExpandStrategy {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => ExpandStrategy::NativeBytes(1),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            ExpandStrategy::NativeBytes(2)
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            ExpandStrategy::NativeBytes(3)
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => ExpandStrategy::NativeBytes(4),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            ExpandStrategy::NativeU16(1)
        }
        // NO_PILLOW_SUPPORT: Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F
        // modes use native samples and promote the supplied byte fill values
        // to their sample type. These are core extensions, not Pillow modes.
        Mode::ImageLumaA16 => ExpandStrategy::NativeU16(2),
        Mode::ImageRgb16 => ExpandStrategy::NativeU16(3),
        Mode::ImageRgba16 => ExpandStrategy::NativeU16(4),
        Mode::ImageRgb32F => ExpandStrategy::NativeF32(3),
        Mode::ImageRgba32F => ExpandStrategy::NativeF32(4),
    }
}

fn storage_matches(img: &DynamicImage, strategy: ExpandStrategy) -> bool {
    match (strategy, img) {
        (ExpandStrategy::NativeBytes(1), DynamicImage::ImageLuma8(_))
        | (ExpandStrategy::NativeBytes(2), DynamicImage::ImageLumaA8(_))
        | (ExpandStrategy::NativeBytes(3), DynamicImage::ImageRgb8(_))
        | (ExpandStrategy::NativeBytes(4), DynamicImage::ImageRgba8(_))
        | (ExpandStrategy::NativeU16(1), DynamicImage::ImageLuma16(_))
        | (ExpandStrategy::NativeU16(2), DynamicImage::ImageLumaA16(_))
        | (ExpandStrategy::NativeU16(3), DynamicImage::ImageRgb16(_))
        | (ExpandStrategy::NativeU16(4), DynamicImage::ImageRgba16(_))
        | (ExpandStrategy::NativeF32(3), DynamicImage::ImageRgb32F(_))
        | (ExpandStrategy::NativeF32(4), DynamicImage::ImageRgba32F(_)) => true,
        _ => false,
    }
}

/// Compatibility helper retained for the existing strategy tests.
#[cfg(test)]
fn op_expand(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode = match explicit_mode {
        Some(name) => Mode::from_pillow_name(name)
            .ok_or_else(|| PilError::ValueError(format!("unsupported Pillow mode: {name}")))?,
        None => Mode::for_image(img, None),
    };
    op_expand_with_mode(img, border, fill, mode)
}

/// Execute Expand after the wrapper has selected its logical mode.
pub(crate) fn op_expand_with_mode(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let strategy = mode_strategy(mode);
    if !storage_matches(img, strategy) {
        return Err(PilError::InternalError(format!(
            "Expand mode {mode:?} does not match its native storage"
        )));
    }
    let (new_width, new_height) = expanded_dimensions(img, border)?;
    match strategy {
        ExpandStrategy::NativeBytes(channels) => {
            expand_native_bytes(img, border, fill, channels, new_width, new_height)
        }
        ExpandStrategy::NativeU16(channels) => {
            expand_u16(img, border, fill, channels, new_width, new_height)
        }
        ExpandStrategy::NativeF32(channels) => {
            expand_f32(img, border, fill, channels, new_width, new_height)
        }
    }
}

/// Return the physical byte count for the pre-existing native-mode tests.
#[cfg(test)]
fn native_expand_channels(img: &DynamicImage, explicit_mode: Option<&str>) -> Option<usize> {
    let mode = match explicit_mode {
        Some(name) => Mode::from_pillow_name(name)?,
        None => Mode::for_image(img, None),
    };
    let strategy = mode_strategy(mode);
    if !storage_matches(img, strategy) {
        return None;
    }
    match strategy {
        ExpandStrategy::NativeBytes(channels) => Some(channels),
        ExpandStrategy::NativeU16(_) | ExpandStrategy::NativeF32(_) => None,
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
