pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `RGBX`; this Rust strategy
// inverts native RGB and padding bytes independently.
declare_invert_mode!(
    ImageRgbx8,
    Input,
    super::default::invert_impl::execute,
    4,
    Some("Invert extends Pillow with native RGBX inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::error::PilError;
// NO_PILLOW_SUPPORT: Pillow's public Quantize rejects visible RGBX; this adds
// native quantization and wrapper-mode-overridden palette mapping.
declare_quantize_native_mode!(u8, "ImageRgbx8", ImageRgbx8);

/// Maps the stored RGB lanes and ignores RGBX padding like Pillow's native mapper.
// Pillow wrapper-mode overrides can admit visible L/RGB while the exact raster
// remains RGBX; the native palette mapper reads its RGB lanes directly.
pub(crate) fn execute_quantize_with_palette(
    image: &Input,
    palette: &[[u8; 3]],
    dither: bool,
    selection: &StrategySelection,
) -> Result<Vec<u8>, PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        crate::raster::Mode::ImageRgbx8,
    )?;
    match architecture {
        Architecture::Default => {
            crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_interleaved_with_palette::<u8, 4>(
                image.as_raw(), image.width(), image.height(), palette, dither,
            )
        }
        _ => Err(PilError::InternalError(
            "RGBX Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageRgbx8,
        super::super::ops::CROP_BORDER_INPUTS,
    );
