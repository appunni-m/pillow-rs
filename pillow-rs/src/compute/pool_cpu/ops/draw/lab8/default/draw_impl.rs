use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Rgb};

pub(crate) fn execute(
    image: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    ops: &[PipelineOp],
    ) -> Result<ImageBuffer<Rgb<u8>, Vec<u8>>, PilError> {
    super::super::super::helpers::execute_byte_buffer::<Rgb<u8>, 3, 1>(image, ops)
}
