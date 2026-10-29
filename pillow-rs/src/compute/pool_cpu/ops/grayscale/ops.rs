//! Native CPU execution and Pillow mode handling for ImageOps.grayscale.

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::Mode;
use crate::raster::DynamicImage;

#[path = "kernels/ycbcr.rs"]
mod ycbcr;

#[derive(Clone, Copy)]
enum GrayscaleStrategy {
    One,
    LaUnsupported(&'static str),
    RgbPremultiplied,
    I32,
    F32,
    Cmyk,
    Hsv,
    YCbCr,
    Generic,
    PaletteExpansionRequired,
}

/// Exhaustive mode support for grayscale conversion. Palette modes are
/// expanded by the Image wrapper before this backend family executes.
const fn mode_strategy(mode: Mode) -> GrayscaleStrategy {
    match mode {
        Mode::One => GrayscaleStrategy::One,
        Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbx8
        | Mode::ImageLab8 => GrayscaleStrategy::Generic,
        Mode::ImageLaPremultiplied8 => {
            GrayscaleStrategy::LaUnsupported("conversion from La to L not supported")
        }
        Mode::ImagePalette8 | Mode::ImagePaletteAlpha8 => {
            GrayscaleStrategy::PaletteExpansionRequired
        }
        Mode::ImageRgbPremultiplied8 => GrayscaleStrategy::RgbPremultiplied,
        Mode::ImageCmyk8 => GrayscaleStrategy::Cmyk,
        Mode::ImageYCbCr8 => GrayscaleStrategy::YCbCr,
        Mode::ImageHsv8 => GrayscaleStrategy::Hsv,
        Mode::ImageI32 => GrayscaleStrategy::I32,
        Mode::ImageF32 => GrayscaleStrategy::F32,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            GrayscaleStrategy::Generic
        }
        // NO_PILLOW_SUPPORT: Core's internal LA16/RGB16/RGBA16/RGB32F/RGBA32F
        // identities also use their native conversion path to produce L.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => GrayscaleStrategy::Generic,
    }
}

fn validate_native_storage(img: &DynamicImage, mode: Mode) -> Result<(), PilError> {
    let matches_storage = match mode {
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
    if matches_storage {
        Ok(())
    } else {
        Err(PilError::InternalError(format!(
            "Grayscale mode {mode:?} does not match its native storage"
        )))
    }
}

/// Execute the mode strategy selected by the Image wrapper.
pub(crate) fn op_grayscale_with_mode(
    img: &DynamicImage,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let strategy = mode_strategy(mode);
    if let GrayscaleStrategy::LaUnsupported(reason) = strategy {
        return Err(PilError::ValueError(reason.to_owned()));
    }
    if matches!(strategy, GrayscaleStrategy::PaletteExpansionRequired) {
        return Err(PilError::InternalError(
            "palette grayscale must be expanded by the Image wrapper".into(),
        ));
    }
    validate_native_storage(img, mode)?;
    let gray = match strategy {
        GrayscaleStrategy::One => {
            let DynamicImage::ImageLuma8(source) = img else {
                return Err(PilError::InternalError("One mode storage changed".into()));
            };
            let mut gray = source.clone();
            for pixel in gray.pixels_mut() {
                pixel[0] = if pixel[0] == 0 { 0 } else { u8::MAX };
            }
            gray
        }
        GrayscaleStrategy::RgbPremultiplied => {
            pil_grayscale(&crate::ops::pil_resize::unpremultiply_alpha(img))?
        }
        GrayscaleStrategy::I32 => match crate::color::i_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("i_to_l always returns L mode"),
        },
        GrayscaleStrategy::F32 => match crate::color::f_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("f_to_l always returns L mode"),
        },
        GrayscaleStrategy::Cmyk => crate::color::cmyk_to_grayscale(img)?,
        GrayscaleStrategy::Hsv => crate::color::pil_grayscale(&crate::color::hsv_to_rgb(img))?,
        GrayscaleStrategy::YCbCr => ycbcr::luma(img).ok_or_else(|| {
            PilError::ValueError("YCbCr grayscale requires native byte triples".into())
        })?,
        GrayscaleStrategy::Generic => pil_grayscale(img)?,
        GrayscaleStrategy::LaUnsupported(_) | GrayscaleStrategy::PaletteExpansionRequired => {
            unreachable!("unsupported mode strategies returned above")
        }
    };
    Ok(DynamicImage::ImageLuma8(gray))
}

/// Grayscale: convert to L-mode using Pillow's source-mode conversion path.
///
/// `ImageOps.grayscale` delegates to `Image.convert("L")`, whose C dispatch
/// does not reinterpret every source buffer as RGB. In particular, `I` and
/// `F` contain one scalar sample in a four-byte transport, `1` treats every
/// non-zero sample as white, and YCbCr copies its Y band directly. Keeping
/// those source semantics here is required before a later operation receives
/// the new L-mode segment.
pub(crate) fn op_grayscale(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // TODO(pillow-rs#3ptb): Pass the cached wrapper Mode through the registry
    // and conversion family instead of parsing the compatibility spelling.
    let mode = match mode {
        Some(name) => Mode::from_pillow_name(name)
            .ok_or_else(|| PilError::ValueError(format!("unsupported Pillow mode: {name}")))?,
        None => Mode::for_image(img, None),
    };
    op_grayscale_with_mode(img, mode)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
