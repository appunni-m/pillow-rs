//! Terminal operations that fold pending image pipelines into host results.

#[cfg(feature = "gpu")]
use super::pool_gpu;
use super::{
    Backend, DynamicImage, PilError, PipelineOp, PipelineTelemetry, PreparedExecution,
    begin_pipeline_operation_telemetry, elapsed_ns, finish_pipeline_operation_telemetry,
    host_resource_telemetry, pipeline_telemetry_enabled, pipeline_timestamp,
    record_pipeline_allocation, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_vector_blocks,
    record_pipeline_resource_telemetry, record_pipeline_telemetry, registry,
    reset_pipeline_allocation_telemetry, reset_pipeline_operation_telemetry,
    take_pipeline_allocation_telemetry, take_pipeline_backend_override,
    take_pipeline_dispatch_count, take_pipeline_resize_coeff_cache_stats,
    take_pipeline_resource_telemetry,
};

/// Runs a serial CPU terminal that folds a pending image pipeline into its
/// result instead of materializing an intermediate image. The closure must
/// produce the exact public result for every operation in `ops`.
pub(crate) fn execute_prepared_cpu_terminal<T>(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    input: &DynamicImage,
    output_buffer_sizes: [usize; 2],
    execute: impl FnOnce() -> Result<T, PilError>,
) -> Result<T, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = execute()?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        for op in ops {
            begin_pipeline_operation_telemetry(registry::variant_key(op));
            record_pipeline_operation_path("cpu");
            finish_pipeline_operation_telemetry();
        }

        let mut resource = host_resource_telemetry(input);
        let output_bytes = output_buffer_sizes
            .iter()
            .map(|&size| size as u64)
            .fold(0u64, u64::saturating_add);
        let output_count = output_buffer_sizes
            .iter()
            .filter(|&&size| size != 0)
            .count() as u64;
        resource.host_buffer_count = resource.host_buffer_count.saturating_add(output_count);
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        resource.fused_operation_count = ops.len() as u64;
        for size in output_buffer_sizes {
            if size != 0 {
                record_pipeline_allocation(size);
            }
        }
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: prepared.requested_backend,
            actual_backend: Backend::Cpu,
            operation_count: ops.len(),
            route_ns: prepared.route_ns,
            validation_ns: prepared.validation_ns,
            backend_ns,
            dispatch_count: None,
            fallback_reason: prepared.fallback_reason.clone(),
            resource: Some(resource),
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(result)
}

/// Runs a SIMD terminal that folds a supported pending image pipeline into
/// its result instead of materializing an intermediate image. The closure
/// returns the measured SIMD block and scalar-tail counts with the result.
pub(crate) fn execute_prepared_simd_terminal<T>(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    input: &DynamicImage,
    output_buffer_sizes: [usize; 2],
    execute: impl FnOnce() -> Result<(T, u64, u64), PilError>,
) -> Result<T, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (result, vector_blocks, scalar_tail) = execute()?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        for (index, op) in ops.iter().enumerate() {
            begin_pipeline_operation_telemetry(registry::variant_key(op));
            if index == 0 {
                record_pipeline_operation_vector_blocks(vector_blocks);
                record_pipeline_operation_scalar_tail(scalar_tail);
            } else {
                record_pipeline_operation_path("scalar-control");
            }
            finish_pipeline_operation_telemetry();
        }

        let mut resource = host_resource_telemetry(input);
        let output_bytes = output_buffer_sizes
            .iter()
            .map(|&size| size as u64)
            .fold(0u64, u64::saturating_add);
        let output_count = output_buffer_sizes
            .iter()
            .filter(|&&size| size != 0)
            .count() as u64;
        resource.host_buffer_count = resource.host_buffer_count.saturating_add(output_count);
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        resource.fused_operation_count = ops.len() as u64;
        for size in output_buffer_sizes {
            if size != 0 {
                record_pipeline_allocation(size);
            }
        }
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: prepared.requested_backend,
            actual_backend: Backend::Simd,
            operation_count: ops.len(),
            route_ns: prepared.route_ns,
            validation_ns: prepared.validation_ns,
            backend_ns,
            dispatch_count: None,
            fallback_reason: prepared.fallback_reason.clone(),
            resource: Some(resource),
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(result)
}

#[cfg(feature = "gpu")]
pub(crate) fn execute_prepared_gpu_cmyk_projection_terminal(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    input: &DynamicImage,
    source_is_known_zero: bool,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    if prepared.selected_backend != Backend::Gpu || prepared.fallback_reason.is_some() {
        return Err(PilError::InternalError(
            "GPU projection terminal was called without an admitted GPU route".into(),
        ));
    }
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (horizontal, vertical, mut resource, dispatches) =
        pool_gpu::GpuPool.execute_cmyk_projection_terminal(input, ops, source_is_known_zero)?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        for op in ops {
            begin_pipeline_operation_telemetry(registry::variant_key(op));
            record_pipeline_operation_path("gpu");
            finish_pipeline_operation_telemetry();
        }
        let output_bytes =
            (horizontal.len() + vertical.len()).saturating_mul(std::mem::size_of::<u32>()) as u64;
        resource.host_buffer_count = resource.host_buffer_count.saturating_add(2);
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: prepared.requested_backend,
            actual_backend: Backend::Gpu,
            operation_count: ops.len(),
            route_ns: prepared.route_ns,
            validation_ns: prepared.validation_ns,
            backend_ns,
            dispatch_count: Some(dispatches),
            fallback_reason: None,
            resource: Some(resource),
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok((horizontal, vertical))
}

#[cfg(feature = "gpu")]
pub(crate) fn execute_prepared_gpu_rgb_projection_terminal(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    input: &DynamicImage,
    source_is_known_zero: bool,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    if prepared.selected_backend != Backend::Gpu || prepared.fallback_reason.is_some() {
        return Err(PilError::InternalError(
            "GPU projection terminal was called without an admitted GPU route".into(),
        ));
    }
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_pipeline_allocation_telemetry();
        reset_pipeline_operation_telemetry();
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (horizontal, vertical, mut resource, dispatches) =
        pool_gpu::GpuPool.execute_rgb_projection_terminal(input, ops, source_is_known_zero)?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        for op in ops {
            begin_pipeline_operation_telemetry(registry::variant_key(op));
            record_pipeline_operation_path("gpu");
            finish_pipeline_operation_telemetry();
        }
        let output_bytes =
            (horizontal.len() + vertical.len()).saturating_mul(std::mem::size_of::<u32>()) as u64;
        resource.host_buffer_count = resource.host_buffer_count.saturating_add(2);
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: prepared.requested_backend,
            actual_backend: Backend::Gpu,
            operation_count: ops.len(),
            route_ns: prepared.route_ns,
            validation_ns: prepared.validation_ns,
            backend_ns,
            dispatch_count: Some(dispatches),
            fallback_reason: None,
            resource: Some(resource),
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok((horizontal, vertical))
}
