//! Exhaustive mode support and orchestration for CPU enhancement operations.

use crate::error::PilError;
use crate::image::Mode;
use crate::raster::DynamicImage;

#[path = "kernels/mod.rs"]
mod kernels;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[derive(Clone, Copy)]
enum BrightnessStrategy {
    Luma8,
    Byte { channels: usize, active: usize },
    U16 { channels: usize, active: usize },
    F32 { channels: usize, active: usize },
    I32,
    Unsupported(&'static str),
}

/// Brightness applies its native-sample scale without inferring mode from the
/// raster carrier. Alpha is retained for the straight-alpha strategies.
const fn brightness_strategy(mode: Mode) -> BrightnessStrategy {
    match mode {
        Mode::One => BrightnessStrategy::Unsupported("image has wrong mode"),
        Mode::ImageLuma8 => BrightnessStrategy::Luma8,
        Mode::ImagePalette8 => {
            // NO_PILLOW_SUPPORT: Core brightness scales palette indices
            // directly without expanding them through a palette.
            BrightnessStrategy::Byte {
                channels: 1,
                active: 1,
            }
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            // NO_PILLOW_SUPPORT: Core brightness can operate directly on P/PA
            // index and alpha bytes without palette expansion.
            BrightnessStrategy::Byte {
                channels: 2,
                active: 1,
            }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            BrightnessStrategy::Byte {
                channels: 3,
                active: 3,
            }
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => BrightnessStrategy::Byte {
            channels: 4,
            active: 3,
        },
        Mode::ImageCmyk8 => BrightnessStrategy::Byte {
            channels: 4,
            active: 4,
        },
        Mode::ImageRgbx8 => BrightnessStrategy::Byte {
            channels: 4,
            active: 3,
        },
        Mode::ImageI32 => BrightnessStrategy::I32,
        Mode::ImageF32 => BrightnessStrategy::F32 {
            channels: 1,
            active: 1,
        },
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            BrightnessStrategy::U16 {
                channels: 1,
                active: 1,
            }
        }
        // NO_PILLOW_SUPPORT: Internal high-depth LA preserves its native
        // u16 storage and scales only the luma sample.
        Mode::ImageLumaA16 => BrightnessStrategy::U16 {
            channels: 2,
            active: 1,
        },
        // NO_PILLOW_SUPPORT: Internal RGB16 scales native u16 channels.
        Mode::ImageRgb16 => BrightnessStrategy::U16 {
            channels: 3,
            active: 3,
        },
        // NO_PILLOW_SUPPORT: Internal RGBA16 scales color and keeps alpha.
        Mode::ImageRgba16 => BrightnessStrategy::U16 {
            channels: 4,
            active: 3,
        },
        // NO_PILLOW_SUPPORT: Internal RGB32F scales native f32 channels.
        Mode::ImageRgb32F => BrightnessStrategy::F32 {
            channels: 3,
            active: 3,
        },
        // NO_PILLOW_SUPPORT: Internal RGBA32F scales color and keeps alpha.
        Mode::ImageRgba32F => BrightnessStrategy::F32 {
            channels: 4,
            active: 3,
        },
    }
}

#[derive(Clone, Copy)]
enum ContrastStrategy {
    NativeByteLut,
    PillowModeConversion,
}

/// Native LUT contracts and conversion-defined Pillow modes are classified
/// independently from their shared physical storage.
const fn contrast_strategy(mode: Mode) -> ContrastStrategy {
    match mode {
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => ContrastStrategy::NativeByteLut,
        Mode::One
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ContrastStrategy::PillowModeConversion,
    }
}

#[derive(Clone, Copy)]
enum ColorStrategy {
    NativeByte { channels: usize },
    NativeU16 { channels: usize },
    NativeF32 { channels: usize },
    NativeI32,
}

/// Color declares direct byte strategies by logical mode. Palette indices are
/// an intentional core extension; the Python ImageEnhance API keeps its
/// palette-preserving construction path.
const fn color_strategy(mode: Mode) -> ColorStrategy {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => {
            // NO_PILLOW_SUPPORT: Core processes One and P as scalar samples;
            // the P path uses indices without expanding through a palette.
            ColorStrategy::NativeByte { channels: 1 }
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            // NO_PILLOW_SUPPORT: Direct PA index-byte saturation is available
            // to core callers without materializing palette colors.
            ColorStrategy::NativeByte { channels: 2 }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            ColorStrategy::NativeByte { channels: 3 }
        }
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8 => ColorStrategy::NativeByte { channels: 4 },
        Mode::ImageI32 => ColorStrategy::NativeI32,
        Mode::ImageF32 => ColorStrategy::NativeF32 { channels: 1 },
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            ColorStrategy::NativeU16 { channels: 1 }
        }
        // NO_PILLOW_SUPPORT: These internal high-depth modes use native
        // sample-domain color strategies without conversion to byte modes.
        Mode::ImageLumaA16 => ColorStrategy::NativeU16 { channels: 2 },
        Mode::ImageRgb16 => ColorStrategy::NativeU16 { channels: 3 },
        Mode::ImageRgba16 => ColorStrategy::NativeU16 { channels: 4 },
        Mode::ImageRgb32F => ColorStrategy::NativeF32 { channels: 3 },
        Mode::ImageRgba32F => ColorStrategy::NativeF32 { channels: 4 },
        }
}

#[derive(Clone, Copy)]
enum SharpnessStrategy {
    NativeByte { channels: usize, active: usize },
    NativeU16 { channels: usize, active: usize },
    NativeF32 { channels: usize, active: usize },
    NativeI32,
    Unsupported(&'static str),
}

/// Sharpness processes the channels defined by the logical mode. It preserves
/// straight alpha and treats RGBX padding and CMYK K as active filter samples.
const fn sharpness_strategy(mode: Mode) -> SharpnessStrategy {
    match mode {
        Mode::One => SharpnessStrategy::Unsupported("image has wrong mode"),
        Mode::ImageLuma8 => {
            SharpnessStrategy::NativeByte {
                channels: 1,
                active: 1,
            }
        }
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 => SharpnessStrategy::NativeByte {
            channels: 2,
            active: 1,
        },
        Mode::ImagePalette8 => {
            // NO_PILLOW_SUPPORT: The core filters palette indices as scalar
            // samples, keeping the original P mode and avoiding palette casts.
            SharpnessStrategy::NativeByte {
                channels: 1,
                active: 1,
            }
        }
        Mode::ImagePaletteAlpha8 => {
            // NO_PILLOW_SUPPORT: The core filters PA indices natively while
            // retaining the second byte as alpha.
            SharpnessStrategy::NativeByte {
                channels: 2,
                active: 1,
            }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            SharpnessStrategy::NativeByte {
                channels: 3,
                active: 3,
            }
        }
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 => SharpnessStrategy::NativeByte {
            channels: 4,
            active: 3,
        },
        Mode::ImageRgbx8 | Mode::ImageCmyk8 => SharpnessStrategy::NativeByte {
            channels: 4,
            active: 4,
        },
        Mode::ImageI32 => SharpnessStrategy::NativeI32,
        Mode::ImageF32 => SharpnessStrategy::NativeF32 {
            channels: 1,
            active: 1,
        },
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            SharpnessStrategy::NativeU16 {
                channels: 1,
                active: 1,
            }
        }
        // NO_PILLOW_SUPPORT: Internal high-depth modes use typed neighborhood
        // kernels and preserve their exact sample representation.
        Mode::ImageLumaA16 => SharpnessStrategy::NativeU16 {
            channels: 2,
            active: 1,
        },
        Mode::ImageRgb16 => SharpnessStrategy::NativeU16 {
            channels: 3,
            active: 3,
        },
        Mode::ImageRgba16 => SharpnessStrategy::NativeU16 {
            channels: 4,
            active: 3,
        },
        Mode::ImageRgb32F => SharpnessStrategy::NativeF32 {
            channels: 3,
            active: 3,
        },
        Mode::ImageRgba32F => SharpnessStrategy::NativeF32 {
            channels: 4,
            active: 3,
        },
    }
}

fn storage_matches(img: &DynamicImage, mode: Mode) -> bool {
    match mode {
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
    }
}

fn validate_storage(img: &DynamicImage, mode: Mode, operation: &str) -> Result<(), PilError> {
    if storage_matches(img, mode) {
        Ok(())
    } else {
        Err(PilError::InternalError(format!(
            "{operation} mode {mode:?} does not match its declared native storage"
        )))
    }
}

pub fn op_enhance_brightness(
    img: &DynamicImage,
    factor: f64,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    validate_storage(img, mode, "Brightness")?;
    let strategy = brightness_strategy(mode);
    if let BrightnessStrategy::Unsupported(reason) = strategy {
        return Err(PilError::ValueError(reason.to_owned()));
    }
    if factor == 1.0 {
        return Ok(img.clone());
    }
    let mut result = img.clone();
    let bytes = result
        .as_bytes_mut()
        .ok_or_else(|| PilError::InternalError("Brightness storage is not mutable".into()))?;
    let width = img.width() as usize;
    let height = img.height() as usize;
    let succeeded = match strategy {
        BrightnessStrategy::Luma8 => {
            kernels::default::scale_luma_bytes(bytes, width, height, factor)
        }
        BrightnessStrategy::Byte { channels, active } => {
            kernels::default::scale_bytes(bytes, width, height, channels, active, factor)
        }
        BrightnessStrategy::U16 { channels, active } => {
            kernels::default::scale_u16_bytes(bytes, width, height, channels, active, factor)
        }
        BrightnessStrategy::F32 { channels, active } => kernels::default::scale_f32_bytes(
            bytes,
            width,
            height,
            channels,
            active,
            factor as f32,
        ),
        BrightnessStrategy::I32 => kernels::default::scale_i32_bytes(bytes, factor),
        BrightnessStrategy::Unsupported(_) => false,
    };
    if succeeded {
        Ok(result)
    } else {
        Err(PilError::InternalError(format!(
            "Brightness mode {mode:?} has an invalid native sample layout"
        )))
    }
}

pub fn op_enhance_contrast(
    img: &DynamicImage,
    factor: f64,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    validate_storage(img, mode, "Contrast")?;
    if matches!(contrast_strategy(mode), ContrastStrategy::NativeByteLut)
        && (factor as f32).is_finite()
        && let Some(base) = crate::ops::enhance::contrast_base_for_mode(img, mode)
    {
        let lut = crate::ops::enhance::contrast_lut(&base, factor);
        return super::effects::op_eval(img, &lut, mode);
    }
    // TODO(pillow-rs#3ptb): Replace this conversion-defined fallback with
    // native mode strategies that preserve each source sample.
    let image = crate::Image::from_dynamic_with_mode(img.clone(), mode)
        .use_backend(crate::compute::Backend::Cpu);
    let base = image.contrast_degenerate()?;
    crate::ops::module_fns::blend(&base, &image, factor)?
        .use_backend(crate::compute::Backend::Cpu)
        .materialize_for_ops()
}

pub fn op_enhance_color_saturation(
    img: &DynamicImage,
    factor: f64,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    validate_storage(img, mode, "ColorSaturation")?;
    let mut result = img.clone();
    let output = result
        .as_bytes_mut()
        .ok_or_else(|| PilError::InternalError("ColorSaturation storage is not mutable".into()))?;
    let succeeded = match color_strategy(mode) {
        ColorStrategy::NativeByte { channels } => kernels::default::blend_color_bytes(
            img.as_bytes(),
            output,
            img.width() as usize,
            img.height() as usize,
            channels,
            mode,
            factor as f32,
        ),
        ColorStrategy::NativeU16 { channels } => kernels::default::blend_color_u16_bytes(
            img.as_bytes(),
            output,
            img.width() as usize,
            img.height() as usize,
            channels,
            factor as f32,
        ),
        ColorStrategy::NativeF32 { channels } => kernels::default::blend_color_f32_bytes(
            img.as_bytes(),
            output,
            img.width() as usize,
            img.height() as usize,
            channels,
            factor as f32,
        ),
        ColorStrategy::NativeI32 => {
            kernels::default::blend_color_i32_bytes(img.as_bytes(), output, factor as f32)
        }
    };
    if succeeded {
        Ok(result)
    } else {
        Err(PilError::InternalError(format!(
            "ColorSaturation mode {mode:?} has an invalid native sample layout"
        )))
    }
}

pub fn op_enhance_sharpness(
    img: &DynamicImage,
    factor: f64,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    validate_storage(img, mode, "Sharpness")?;
    if factor == 1.0 {
        return Ok(img.clone());
    }
    let output = match sharpness_strategy(mode) {
        SharpnessStrategy::NativeByte { channels, active } => kernels::default::sharpness_bytes(
            img.as_bytes(),
            img.width() as usize,
            img.height() as usize,
            channels,
            active,
            factor as f32,
        ),
        SharpnessStrategy::NativeU16 { channels, active } => {
            kernels::default::sharpness_u16_bytes(
                img.as_bytes(),
                img.width() as usize,
                img.height() as usize,
                channels,
                active,
                factor as f32,
            )
        }
        SharpnessStrategy::NativeF32 { channels, active } => {
            kernels::default::sharpness_f32_bytes(
                img.as_bytes(),
                img.width() as usize,
                img.height() as usize,
                channels,
                active,
                factor as f32,
            )
        }
        SharpnessStrategy::NativeI32 => kernels::default::sharpness_i32_bytes(
            img.as_bytes(),
            img.width() as usize,
            img.height() as usize,
            factor as f32,
        ),
        SharpnessStrategy::Unsupported(reason) => {
            return Err(PilError::ValueError(reason.to_owned()));
        }
    }
    .ok_or_else(|| {
        PilError::InternalError(format!(
            "Sharpness mode {mode:?} has an invalid native sample layout"
        ))
    })?;
    let mut result = img.clone();
    result
        .as_bytes_mut()
        .ok_or_else(|| PilError::InternalError("Sharpness storage is not mutable".into()))?
        .copy_from_slice(&output);
    Ok(result)
}
