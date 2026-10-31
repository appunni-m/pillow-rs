use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Rgba};

pub(crate) fn execute(
    image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    ops: &[PipelineOp],
    ) -> Result<ImageBuffer<Rgba<u8>, Vec<u8>>, PilError> {
    super::super::super::helpers::execute_byte_buffer::<Rgba<u8>, 4>(image, ops)
}
