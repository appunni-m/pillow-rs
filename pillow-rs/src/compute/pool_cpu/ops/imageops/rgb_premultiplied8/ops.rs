pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_mode;

// NO_PILLOW_SUPPORT: Pillow ImageOps.invert rejects `RGBa`; this Rust strategy
// inverts the native premultiplied RGBA carrier byte by byte.
declare_invert_mode!(
    ImageRgbPremultiplied8,
    Input,
    super::default::invert_impl::execute,
    4,
    Some("Invert extends Pillow with native RGBa inversion"),
);

use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;
use crate::compute::{Architecture, registry::support::StrategySelection};
use crate::error::PilError;
// NO_PILLOW_SUPPORT: Pillow's public Quantize rejects visible RGBa; this adds
// native alpha-aware quantization and wrapper-mode-overridden palette mapping.
declare_quantize_native_mode!(u8, "ImageRgbPremultiplied8", ImageRgbPremultiplied8);

/// Maps stored premultiplied RGB lanes directly, matching Pillow's palette mapper.
// Pillow wrapper-mode overrides can admit visible L/RGB while the exact raster
// remains RGBa; the native palette mapper reads the stored lanes directly.
pub(crate) fn execute_quantize_with_palette(
    image: &Input,
    palette: &[[u8; 3]],
    dither: bool,
    selection: &StrategySelection,
) -> Result<Vec<u8>, PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        crate::raster::Mode::ImageRgbPremultiplied8,
    )?;
    match architecture {
        Architecture::Default => {
            crate::compute::pool_cpu::ops::imageops::helpers::quantize_rgb_source::execute_interleaved_with_palette::<u8, 4>(
                image.as_raw(), image.width(), image.height(), palette, dither,
            )
        }
        _ => Err(PilError::InternalError(
            "RGBa Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::imageops::helpers::strategy::crop_border_serial_support(
        crate::raster::Mode::ImageRgbPremultiplied8,
        super::super::ops::CROP_BORDER_INPUTS,
    );
