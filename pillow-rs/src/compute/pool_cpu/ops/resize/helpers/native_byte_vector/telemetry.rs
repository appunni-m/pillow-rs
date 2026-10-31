pub(super) fn record_vector_work(
    horizontal_blocks: u64,
    vertical_blocks: u64,
    horizontal_tail: u64,
    vertical_tail: u64,
) {
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks.saturating_add(vertical_blocks),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_tail.saturating_add(vertical_tail),
    );
}
