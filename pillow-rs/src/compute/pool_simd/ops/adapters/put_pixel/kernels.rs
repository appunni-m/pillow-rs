//! Narrow entry point for the specialized SIMD PutPixel batch kernel.

mod rgba_batch;

use crate::error::PilError;
use crate::pipeline::PipelineOp;

#[inline(always)]
pub(super) fn apply_rgba_batch(
    output: &mut [u8],
    row_bytes: usize,
    ops: &[PipelineOp],
) -> Result<bool, PilError> {
    rgba_batch::apply_rgba_batch(output, row_bytes, ops)
}
