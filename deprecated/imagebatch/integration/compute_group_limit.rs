/// Return the largest safe number of equal-sized images for the explicit GPU
/// batch API. A zero result keeps the caller on its existing per-image route.
pub(crate) fn gpu_batch_group_limit(
    op: &PipelineOp,
    logical_mode: &str,
    dimensions: (u32, u32),
    requested: usize,
) -> usize {
    #[cfg(feature = "gpu")]
    {
        pool_gpu::gpu_batch_group_limit(op, logical_mode, dimensions, requested)
    }
    #[cfg(not(feature = "gpu"))]
    {
        let _ = (op, logical_mode, dimensions, requested);
        0
    }
}
