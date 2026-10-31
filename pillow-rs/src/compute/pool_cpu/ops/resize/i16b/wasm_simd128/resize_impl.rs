use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{ImageBuffer, Luma};

pub(crate) fn execute(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    super::super::super::helpers::luma16_vector::resize::<true>(input, width, height, filter)
}
