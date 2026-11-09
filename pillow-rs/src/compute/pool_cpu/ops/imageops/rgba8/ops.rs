pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::RgbaImage;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects RGBA; this route preserves the
// native carrier as a core-only extension.
declare_invert_native_mode!(
    ImageRgba8,
    Input,
    4,
    4,
    Some("ImageOps.invert extends Pillow with native RGBA inversion"),
);

pub(crate) fn execute_quantize(
    image: &RgbaImage,
    width: u32,
    height: u32,
    colors: usize,
    selection: &StrategySelection,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        crate::raster::Mode::ImageRgba8,
    )?;
    match architecture {
        Architecture::Default => {
            super::default::quantize_impl::execute(image, width, height, colors)
        }
        _ => Err(PilError::InternalError(
            "RGBA Quantize selected an unimplemented architecture".into(),
        )),
    }
}

/// Maps the stored RGB lanes to a supplied palette without expanding pixels.
// Pillow wrapper-mode overrides can admit visible L/RGB while the exact raster
// remains RGBA; the native palette mapper reads its stored RGB lanes directly.
pub(crate) fn execute_quantize_with_palette(
    image: &RgbaImage,
    palette: &[[u8; 3]],
    dither: bool,
    selection: &StrategySelection,
) -> Result<Vec<u8>, PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        crate::raster::Mode::ImageRgba8,
    )?;
    match architecture {
        Architecture::Default => {
            crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_interleaved_with_palette::<u8, 4>(
                image.as_raw(), image.width(), image.height(), palette, dither,
            )
        }
        _ => Err(PilError::InternalError(
            "RGBA Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageRgba8,
        super::super::ops::CROP_BORDER_INPUTS,
    );
