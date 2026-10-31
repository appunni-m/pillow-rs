use crate::compute::pool_cpu::ops::draw::helpers::simd;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    image: &DynamicImage,
    ops: &[PipelineOp],
) -> Result<DynamicImage, PilError> {
    simd::execute::<super::super::ops::CHANNELS, super::super::ops::FLOAT_MODE>(
        image,
        ops,
        super::super::ops::MODE,
    )
}
