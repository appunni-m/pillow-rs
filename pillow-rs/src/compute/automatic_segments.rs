//! Eligibility rules for adjacent automatic CPU/SIMD execution segments.
//!
//! TODO(pillow-rs#3ptb): replace these pool-level segment rules with prepared
//! mode/architecture strategies injected through `ExecutionContext`.

use super::pool_simd::ops::adapters;
use super::{Backend, BackendImpl, DynamicImage, PilError};
use crate::pipeline::PipelineOp;

pub(super) fn preserves_native_contract(op: &PipelineOp) -> bool {
    adapters::preserves_native_contract(op)
}

pub(super) fn can_extend(previous: &PipelineOp, next: &PipelineOp) -> bool {
    matches!(
        (previous, next),
        (PipelineOp::Transpose { .. }, PipelineOp::Transpose { .. })
    ) || preserves_native_contract(previous)
}

#[inline(always)]
pub(super) fn segment_end(
    ops: &[PipelineOp],
    start: usize,
    backend: Backend,
    input: &DynamicImage,
    mode: Option<&str>,
    simd: &dyn BackendImpl,
    cpu: &dyn BackendImpl,
) -> Result<usize, PilError> {
    let mut end = start + 1;
    if backend == Backend::Simd {
        // Every operation in this segment is checked against the same
        // concrete layout. A transpose-only run is the exception: the
        // SIMD batch executor composes it before dispatch, and each
        // transform is checked against this original image layout.
        while end < ops.len()
            && can_extend(&ops[end - 1], &ops[end])
            && simd.supports_for_image(&ops[end], input, mode)?
        {
            end += 1;
        }
    } else {
        // CPU can absorb adjacent CPU-only work, including draw batches,
        // as long as the current layout contract remains unchanged. Stop
        // before an operation that SIMD can execute so the next segment
        // can return to the vector path without replaying this segment.
        while end < ops.len()
            && preserves_native_contract(&ops[end - 1])
            && !simd.supports_for_image(&ops[end], input, mode)?
        {
            if !cpu.supports(&ops[end])? {
                break;
            }
            end += 1;
        }
    }
    Ok(end)
}
