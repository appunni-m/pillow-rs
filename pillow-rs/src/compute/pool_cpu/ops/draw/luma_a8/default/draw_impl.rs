use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, LumaA};

pub(crate) fn execute(
    image: &ImageBuffer<LumaA<u8>, Vec<u8>>,
    ops: &[PipelineOp],
    ) -> Result<ImageBuffer<LumaA<u8>, Vec<u8>>, PilError> {
    super::super::super::helpers::execute_byte_buffer::<LumaA<u8>, 2>(image, ops)
}
