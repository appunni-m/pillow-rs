//! Ordered RGBA byte writes using the existing masked SIMD store.

use crate::error::PilError;
use crate::pipeline::PipelineOp;

/// Apply an already-admitted batch in source order to the caller's frame copy.
#[inline]
pub(super) fn apply_rgba_batch(
    output: &mut [u8],
    row_bytes: usize,
    ops: &[PipelineOp],
) -> Result<bool, PilError> {
    for op in ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Ok(false);
        };
        let target_start = (*y as usize)
            .checked_mul(row_bytes)
            .and_then(|row| row.checked_add((*x as usize).checked_mul(4)?))
            .ok_or_else(|| PilError::InternalError("SIMD PutPixel offset overflow".into()))?;
        let target_end = target_start
            .checked_add(4)
            .ok_or_else(|| PilError::InternalError("SIMD PutPixel end overflow".into()))?;
        if target_end > output.len() {
            return Ok(false);
        }
        let block_start = target_start.min(output.len() - 16);
        super::super::super::draw_line_vector_block(
            output,
            block_start,
            target_start,
            target_end,
            4,
            *color,
            false,
        );
    }
    Ok(true)
}
