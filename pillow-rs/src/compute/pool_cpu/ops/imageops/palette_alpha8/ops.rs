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

pub(crate) fn execute_quantize_with_palette(
    samples: &[u8],
    palette: crate::compute::registry::support::PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
    architecture: crate::compute::Architecture,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), crate::error::PilError> {
    match architecture {
        crate::compute::Architecture::Default => {
            super::default::quantize_impl::execute(samples, palette, width, height, colors)
        }
        _ => Err(crate::error::PilError::InternalError(
            "PA Quantize selected an unimplemented architecture".into(),
        )),
    }
}
