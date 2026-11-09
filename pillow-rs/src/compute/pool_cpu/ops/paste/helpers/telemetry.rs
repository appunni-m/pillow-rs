//! Completion metrics for architecture-specific Paste kernels.

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) fn record_l_mask_neon(
    width: u32,
    height: u32,
    channels: u64,
    path: &'static str,
    record_scalar_tail: bool,
) {
    let row_bytes = u64::from(width).saturating_mul(channels);
    let remainder = row_bytes % 16;
    let tail_blocks = match remainder {
        0 => 0,
        1..=8 => 1,
        _ => 2,
    };
    let vector_blocks = (row_bytes / 16)
        .saturating_add(tail_blocks)
        .saturating_mul(u64::from(height));
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if record_scalar_tail {
        let scalar_tail = (row_bytes % 8).saturating_mul(u64::from(height));
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
    }
    crate::compute::record_pipeline_operation_path(path);
}
