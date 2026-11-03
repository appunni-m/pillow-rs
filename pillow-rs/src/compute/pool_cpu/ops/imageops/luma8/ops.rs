pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::GrayImage;

declare_invert_native_mode!(ImageLuma8, Input, 1, 1);

pub(crate) fn execute_quantize(
    image: &GrayImage,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
    architecture: Architecture,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(
            image, width, height, colors, kmeans, method,
        ),
        _ => Err(PilError::InternalError(
            "L Quantize selected an unimplemented architecture".into(),
        )),
    }
}

pub(crate) fn execute_quantize_with_palette(
    image: &GrayImage,
    palette: &[[u8; 3]],
    dither: bool,
    architecture: Architecture,
) -> Result<Vec<u8>, PilError> {
    match architecture {
        Architecture::Default => {
            super::default::quantize_impl::execute_with_palette(image, palette, dither)
        }
        _ => Err(PilError::InternalError(
            "L Quantize selected an unimplemented architecture".into(),
        )),
    }
}
