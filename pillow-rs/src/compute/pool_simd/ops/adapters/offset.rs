//! Native SIMD Offset adapter and mode-specific kernels.

#[path = "offset/kernels/byte.rs"]
mod byte;
#[path = "offset/kernels/luma16.rs"]
mod luma16;

use self::byte::native_offset;
use self::luma16::native_offset_luma16;
use super::simd_unsupported;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(crate) fn simd_offset(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Offset { x, y } = op else {
        return Err(PilError::ValueError("expected Offset op".into()));
    };
    if let Some((result, vector_blocks, scalar_tail)) = native_offset_luma16(img, *x, *y, mode) {
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(result);
    }
    if let Some((result, vector_blocks, scalar_tail)) = native_offset(img, *x, *y, mode) {
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(result);
    }
    Err(simd_unsupported("Offset"))
}
