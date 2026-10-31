use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma};

pub(crate) fn execute(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    ops: &[PipelineOp],
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    super::super::super::helpers::execute_luma16_buffer::<false>(image, ops)
}
