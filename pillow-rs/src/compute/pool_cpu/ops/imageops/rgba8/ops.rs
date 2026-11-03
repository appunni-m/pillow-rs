pub(crate) type Input = crate::raster::RgbaImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
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
    architecture: Architecture,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(image, width, height, colors),
        _ => Err(PilError::InternalError(
            "RGBA Quantize selected an unimplemented architecture".into(),
        )),
    }
}
