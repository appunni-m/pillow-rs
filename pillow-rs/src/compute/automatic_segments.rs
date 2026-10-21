//! Eligibility rules for adjacent automatic CPU/SIMD execution segments.

use super::pool_simd::ops::adapters;
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
