//! CPU Reduce mode strategy and execution orchestration.

use crate::error::PilError;
use crate::image::Mode;
use crate::raster::DynamicImage;

#[path = "kernels/mod.rs"]
mod kernels;

#[derive(Clone, Copy)]
struct ReduceAlphaLayout {
    premultiplied_alpha: bool,
    native_rgba_reduce: bool,
}

fn alpha_layout(mode: Mode) -> ReduceAlphaLayout {
    let premultiplied_alpha = matches!(mode, Mode::ImageLumaA8 | Mode::ImageRgba8);
    ReduceAlphaLayout {
        premultiplied_alpha,
        native_rgba_reduce: mode == Mode::ImageRgba8,
    }
}

fn untagged_alpha_layout(img: &DynamicImage) -> ReduceAlphaLayout {
    // TODO(pillow-rs#3ptb): Remove this storage-derived compatibility path
    // after every Reduce caller forwards the wrapper's cached Mode.
    ReduceAlphaLayout {
        premultiplied_alpha: matches!(
            img.color(),
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ),
        native_rgba_reduce: false,
    }
}

#[derive(Clone, Copy)]
enum ReduceStrategy {
    NativeBytes,
    NativeRgb,
    NativeI32,
    NativeF32,
    NativeI16,
    NativeU16Pixels,
    NativeF32Pixels,
}

/// Exhaustive mode support for CPU Reduce. Non-RGB byte modes use the same
/// native channel reducer; RGBA and LA select premultiplied-alpha handling,
/// while RGB also has a guarded packed kernel. Scalar and I;16 use typed reducers.
const fn mode_strategy(mode: Mode) -> ReduceStrategy {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ReduceStrategy::NativeBytes,
        Mode::ImageRgb8 => ReduceStrategy::NativeRgb,
        Mode::ImageI32 => ReduceStrategy::NativeI32,
        Mode::ImageF32 => ReduceStrategy::NativeF32,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            ReduceStrategy::NativeI16
        }
        // NO_PILLOW_SUPPORT: Core-only high-depth Reduce averages each native
        // channel independently, including alpha. Pillow has no equivalent
        // public mode identities for these carriers; no mode cast is used.
        Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16 => {
            ReduceStrategy::NativeU16Pixels
        }
        Mode::ImageRgb32F | Mode::ImageRgba32F => ReduceStrategy::NativeF32Pixels,
    }
}

fn validate_native_storage(img: &DynamicImage, mode: Mode) -> Result<(), PilError> {
    let matches_native_storage = match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
            matches!(img, DynamicImage::ImageLuma8(_))
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            matches!(img, DynamicImage::ImageLumaA8(_))
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            matches!(img, DynamicImage::ImageRgb8(_))
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => matches!(img, DynamicImage::ImageRgba8(_)),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            matches!(img, DynamicImage::ImageLuma16(_))
        }
        Mode::ImageLumaA16 => matches!(img, DynamicImage::ImageLumaA16(_)),
        Mode::ImageRgb16 => matches!(img, DynamicImage::ImageRgb16(_)),
        Mode::ImageRgba16 => matches!(img, DynamicImage::ImageRgba16(_)),
        Mode::ImageRgb32F => matches!(img, DynamicImage::ImageRgb32F(_)),
        Mode::ImageRgba32F => matches!(img, DynamicImage::ImageRgba32F(_)),
    };
    if matches_native_storage {
        Ok(())
    } else {
        Err(PilError::InternalError(format!(
            "Reduce mode {mode:?} does not match its native storage"
        )))
    }
}

#[cfg(test)]
use kernels::execute_reduce_rgb_sparse_2x2;
use kernels::{execute_reduce_default, execute_reduce_rgb, reduce_luma16};
pub(super) use kernels::{
    execute_reduce_rgb_with_sparse_rows, reduce_f_thumbnail, reduce_i_thumbnail,
};

/// Execute Reduce through the compatibility-string boundary.
pub(crate) fn execute_reduce(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // TODO(pillow-rs#3ptb): Carry the wrapper's cached Mode through Thumbnail
    // instead of reparsing its compatibility string at this family boundary.
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let Some(name) = explicit_mode else {
        return execute_reduce_default(
            img,
            x_factor.max(1),
            y_factor.max(1),
            untagged_alpha_layout(img),
        );
    };
    let mode = Mode::from_pillow_name(name)
        .ok_or_else(|| PilError::ValueError(format!("unsupported Pillow mode: {name}")))?;
    execute_reduce_with_mode(img, x_factor, y_factor, mode)
}

/// Execute Reduce after the wrapper has selected its logical mode.
pub(crate) fn execute_reduce_with_mode(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    if x_factor < 2 && y_factor < 2 {
        return Ok(img.clone());
    }
    let fx = x_factor.max(1);
    let fy = y_factor.max(1);
    let strategy = mode_strategy(mode);
    validate_native_storage(img, mode)?;
    let default = || execute_reduce_default(img, fx, fy, alpha_layout(mode));
    match strategy {
        ReduceStrategy::NativeBytes => default(),
        ReduceStrategy::NativeRgb => match execute_reduce_rgb(img, fx, fy, mode)? {
            Some(output) => Ok(output),
            None => default(),
        },
        ReduceStrategy::NativeI32 => reduce_i_thumbnail(
            img,
            img.width().div_ceil(fx),
            img.height().div_ceil(fy),
            fx,
            fy,
        ),
        ReduceStrategy::NativeF32 => reduce_f_thumbnail(
            img,
            img.width().div_ceil(fx),
            img.height().div_ceil(fy),
            fx,
            fy,
        ),
        ReduceStrategy::NativeI16 => match img {
            DynamicImage::ImageLuma16(image) => {
                reduce_luma16(image, fx, fy).map(DynamicImage::ImageLuma16)
            }
            _ => Err(PilError::InternalError(
                "I;16 Reduce storage changed after strategy selection".into(),
            )),
        },
        ReduceStrategy::NativeU16Pixels => match img {
            DynamicImage::ImageLumaA16(image) => {
                reduce_u16_pixels(image, fx, fy).map(DynamicImage::ImageLumaA16)
            }
            DynamicImage::ImageRgb16(image) => {
                reduce_u16_pixels(image, fx, fy).map(DynamicImage::ImageRgb16)
            }
            DynamicImage::ImageRgba16(image) => {
                reduce_u16_pixels(image, fx, fy).map(DynamicImage::ImageRgba16)
            }
            _ => Err(PilError::InternalError(
                "high-depth u16 Reduce storage changed after strategy selection".into(),
            )),
        },
        ReduceStrategy::NativeF32Pixels => match img {
            DynamicImage::ImageRgb32F(image) => {
                reduce_f32_pixels(image, fx, fy).map(DynamicImage::ImageRgb32F)
            }
            DynamicImage::ImageRgba32F(image) => {
                reduce_f32_pixels(image, fx, fy).map(DynamicImage::ImageRgba32F)
            }
            _ => Err(PilError::InternalError(
                "high-depth f32 Reduce storage changed after strategy selection".into(),
            )),
        },
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
