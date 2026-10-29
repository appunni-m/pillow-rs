//! Native SIMD Offset adapter and mode-specific kernels.

#[path = "offset/kernels/byte.rs"]
mod byte;
#[path = "offset/kernels/luma16.rs"]
mod luma16;

use self::byte::native_offset;
use self::luma16::native_offset_luma16;
use super::{
    SimdImageShape, has_empty_native_bytes, has_vectorized_luma16_offset,
    has_vectorized_offset_rows, native_copy_layout, shape_has_empty_native_bytes,
    shape_has_vectorized_luma16_offset, shape_has_vectorized_offset_rows,
    shape_native_copy_channels, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(super) fn supports_for_image(img: &DynamicImage, xoffset: i32, mode: Option<&str>) -> bool {
    has_vectorized_luma16_offset(img, mode)
        || native_copy_layout(img, mode).is_some_and(|channels| {
            has_empty_native_bytes(img, channels)
                || has_vectorized_offset_rows(img, channels, xoffset)
        })
}

pub(super) fn supports_for_shape(shape: SimdImageShape, xoffset: i32, mode: Option<&str>) -> bool {
    shape_has_vectorized_luma16_offset(shape, mode)
        || shape_native_copy_channels(shape, mode).is_some_and(|channels| {
            shape_has_empty_native_bytes(shape, channels)
                || shape_has_vectorized_offset_rows(shape, channels, xoffset)
        })
}

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
