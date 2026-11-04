pub(crate) type Input = crate::raster::GrayAlphaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;

// NO_PILLOW_SUPPORT: ImageOps.invert rejects PA; this route preserves native
// palette-index and alpha bytes plus attached palette metadata as a core-only
// extension.
declare_invert_native_mode!(
    ImagePaletteAlpha8,
    Input,
    2,
    2,
    Some("ImageOps.invert extends Pillow with native PA inversion"),
);

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds a
// native PA strategy that resolves palette colors and alpha during sampling.
pub(crate) fn execute_quantize_with_palette(
    samples: &[u8],
    palette: crate::compute::registry::support::PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
    selection: &crate::compute::registry::support::StrategySelection,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), crate::error::PilError> {
    let architecture =
        crate::compute::pool_cpu::ops::imageops::helpers::strategy::selected_quantize_architecture(
            selection,
            crate::raster::Mode::ImagePaletteAlpha8,
        )?;
    match architecture {
        crate::compute::Architecture::Default => {
            super::default::quantize_impl::execute(samples, palette, width, height, colors)
        }
        _ => Err(crate::error::PilError::InternalError(
            "PA Quantize selected an unimplemented architecture".into(),
        )),
    }
}
