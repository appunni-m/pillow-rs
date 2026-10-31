use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Luma};

pub(crate) fn execute(
    image: &ImageBuffer<Luma<u8>, Vec<u8>>,
    ops: &[PipelineOp],
    ) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, PilError> {
    super::super::super::helpers::execute_byte_buffer::<Luma<u8>, 1>(image, ops)
}
