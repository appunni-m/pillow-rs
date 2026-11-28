use crate::compute::pool_cpu::ops::draw::helpers::simd;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn execute(image: &DynamicImage, ops: &[PipelineOp]) -> Result<DynamicImage, PilError> {
    simd::vector_execute::<{ super::super::ops::CHANNELS }, { super::super::ops::FLOAT_MODE }>(
        image,
        ops,
        super::super::ops::MODE,
    )
}
