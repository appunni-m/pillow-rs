//! Direct eager operations and execution receipts outside lazy image pipelines.
//!
use super::backend_state::active_lock;
#[cfg(feature = "gpu")]
use super::pool_gpu;
use super::{
    Backend, DynamicImage, ExecutionContext, PilError, PipelineOp, PipelineResourceTelemetry, PipelineTelemetry,
    begin_pipeline_operation_telemetry, elapsed_ns, finish_pipeline_operation_telemetry,
    host_resource_telemetry, pipeline_telemetry_enabled, pipeline_timestamp,
    record_pipeline_allocation, record_pipeline_operation_handoff,
    record_pipeline_operation_architecture, record_pipeline_operation_mode_conversion,
    record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_vector_blocks,
    record_pipeline_resource_telemetry, record_pipeline_telemetry,
    reset_pipeline_allocation_telemetry, reset_pipeline_operation_telemetry,
    take_pipeline_allocation_telemetry, take_pipeline_backend_override,
    take_pipeline_dispatch_count, take_pipeline_resize_coeff_cache_stats,
    take_pipeline_resource_telemetry, take_pipeline_telemetry,
};
mod generators;
pub(crate) use generators::{
    execute_eager_cpu_linear_gradient, execute_eager_cpu_mandelbrot,
    execute_eager_cpu_radial_gradient,
};

fn reset_eager_image_operation_telemetry() {
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
}

fn discard_eager_image_operation_telemetry() {
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    let _ = take_pipeline_telemetry();
}

pub(crate) fn execute_eager_cpu_getprojection(
    input: &DynamicImage,
    context: &ExecutionContext,
    requested_backend: Option<Backend>,
    fallback_reason: Option<String>,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    let prepared = super::prepare_cpu_getprojection(input, context)?;
    let selection = *prepared.selection();
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let output = prepared.execute()?;
    let backend_ns = elapsed_ns(backend_start);
    let result = output.axes;
    if timed {
        begin_pipeline_operation_telemetry("GetProjection");
        record_pipeline_operation_architecture(selection.implementation.architecture);
        record_pipeline_operation_vector_blocks(output.vector_blocks);
        record_pipeline_operation_scalar_tail(output.scalar_tail);
        record_pipeline_operation_path(if output.vector_blocks == 0 { "cpu" } else { "vector" });
        finish_pipeline_operation_telemetry();
        let selected_fallback = selection
            .fallback
            .map(|reason| format!("GetProjection strategy fell back: {reason:?}"));
        let fallback_reason = fallback_reason.or(selected_fallback);
        if fallback_reason.is_some() {
            record_pipeline_operation_handoff(1);
        }

        let mut resource = host_resource_telemetry(input);
        let output_bytes =
            (result.0.len() + result.1.len()).saturating_mul(std::mem::size_of::<u32>()) as u64;
        resource.host_buffer_count = resource.host_buffer_count.saturating_add(2);
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        resource.fused_operation_count = 0;
        record_pipeline_allocation(result.0.len() * std::mem::size_of::<u32>());
        record_pipeline_allocation(result.1.len() * std::mem::size_of::<u32>());
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend,
            actual_backend: Backend::Cpu,
            operation_count: 1,
            route_ns: 0,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(result)
}

#[cfg(feature = "gpu")]
pub(crate) fn execute_eager_gpu_rgb_getprojection(
    input: &DynamicImage,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    if !pool_gpu::GpuPool::rgb_projection_supported(input) {
        return Err(PilError::NotImplementedError(
            "GPU does not support this RGB projection".into(),
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
        pool_gpu::GpuPool.execute_rgb_projection(input)?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        begin_pipeline_operation_telemetry("GetProjection");
        record_pipeline_operation_path("gpu");
        finish_pipeline_operation_telemetry();
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
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count().or(Some(dispatches));
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(Backend::Gpu),
            actual_backend: Backend::Gpu,
            operation_count: 1,
            route_ns: 0,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason: None,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok((horizontal, vertical))
}

/// Execute one CPU image operation directly for a loaded source while
/// retaining the same backend and host-buffer evidence as pipeline dispatch.
/// Set `manual_output_allocation` when the closure does not record its result
/// buffer through the normal allocation telemetry hook.
#[cfg(test)]
pub(crate) fn execute_eager_cpu_image_operation(
    operation: &'static str,
    input: &DynamicImage,
    manual_output_allocation: bool,
    execute: impl FnOnce(&ExecutionContext) -> Result<DynamicImage, PilError>,
) -> Result<DynamicImage, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }

    let route_start = timed.then(pipeline_timestamp).flatten();
    let context = ExecutionContext::for_routed_backend(Backend::Cpu);
    let route_ns = elapsed_ns(route_start);
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = match execute(&context) {
        Ok(result) => result,
        Err(error) => {
            if timed {
                discard_eager_image_operation_telemetry();
            }
            return Err(error);
        }
    };
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        // Match pipeline boundary accounting for the actual physical color
        // change without adding work to telemetry-disabled eager calls.
        let mode_conversion_count = u64::from(input.color() != result.color());
        begin_pipeline_operation_telemetry(operation);
        record_pipeline_operation_path("cpu");
        record_pipeline_operation_mode_conversion(mode_conversion_count);
        finish_pipeline_operation_telemetry();

        let mut resource = host_resource_telemetry(input);
        resource.mode_conversion_count = resource
            .mode_conversion_count
            .saturating_add(mode_conversion_count);
        let output_bytes = result.as_bytes().len();
        let output_bytes_u64 = output_bytes as u64;
        resource.host_buffer_count = resource
            .host_buffer_count
            .saturating_add(u64::from(output_bytes != 0));
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes_u64);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes_u64));
        if manual_output_allocation {
            record_pipeline_allocation(output_bytes);
        }
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(Backend::Cpu),
            actual_backend: Backend::Cpu,
            operation_count: 1,
            route_ns,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason: None,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(result)
}

/// Executes a prepared CPU architecture strategy for one eager image operation.
///
/// The active operation frame surrounds preparation and execution so a selected
/// vector implementation can attach its block and tail evidence to this receipt.
pub(crate) fn execute_eager_cpu_arch_image_operation(
    operation: &'static str,
    input: &DynamicImage,
    requested_backend: Backend,
    execute: impl FnOnce(
        &ExecutionContext,
    ) -> Result<(DynamicImage, crate::compute::registry::support::StrategySelection), PilError>,
) -> Result<DynamicImage, PilError> {
    execute_eager_cpu_arch_image_operation_with_route(
        operation,
        input,
        requested_backend,
        requested_backend,
        None,
        execute,
    )
}

/// Executes a CPU architecture strategy while retaining the original routed backend receipt.
pub(crate) fn execute_eager_cpu_arch_image_operation_with_route(
    operation: &'static str,
    input: &DynamicImage,
    requested_backend: Backend,
    execution_backend: Backend,
    fallback_reason: Option<String>,
    execute: impl FnOnce(
        &ExecutionContext,
    ) -> Result<(DynamicImage, crate::compute::registry::support::StrategySelection), PilError>,
) -> Result<DynamicImage, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let context = ExecutionContext::for_routed_backend(execution_backend);
    let route_ns = elapsed_ns(route_start);
    execute_cpu_arch_operation(
        operation,
        input,
        context,
        Some(requested_backend),
        route_ns,
        0,
        fallback_reason,
        execute,
    )
}

/// Executes a prepared CPU architecture strategy into a caller-owned byte slice.
pub(crate) fn execute_eager_cpu_arch_bytes(
    operation: &'static str,
    input: &DynamicImage,
    requested_backend: Backend,
    output: &mut [u8],
    execute: impl FnOnce(
        &ExecutionContext,
        &mut [u8],
    ) -> Result<Option<(
        crate::compute::registry::support::StrategySelection,
        (u64, u64, &'static str),
    )>, PilError>,
) -> Result<Option<()>, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let context = ExecutionContext::for_routed_backend(requested_backend);
    let route_ns = elapsed_ns(route_start);
    if timed {
        reset_eager_image_operation_telemetry();
    }
    if context.backend() != Backend::Cpu {
        if timed {
            discard_eager_image_operation_telemetry();
        }
        return Err(PilError::InternalError(
            "CPU architecture byte execution received a non-CPU context".into(),
        ));
    }
    if timed {
        begin_pipeline_operation_telemetry(operation);
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (selection, (vector_blocks, scalar_tail, path)) = match execute(&context, output) {
        Ok(Some(result)) => result,
        Ok(None) => {
            if timed {
                discard_eager_image_operation_telemetry();
            }
            return Ok(None);
        }
        Err(error) => {
            if timed {
                discard_eager_image_operation_telemetry();
            }
            return Err(error);
        }
    };
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        record_pipeline_operation_architecture(selection.implementation.architecture);
        record_pipeline_operation_vector_blocks(vector_blocks);
        record_pipeline_operation_scalar_tail(scalar_tail);
        record_pipeline_operation_path(path);
        record_pipeline_operation_mode_conversion(0);
        finish_pipeline_operation_telemetry();

        let mut resource = host_resource_telemetry(input);
        let output_bytes = output.len() as u64;
        resource.host_buffer_count = resource
            .host_buffer_count
            .saturating_add(u64::from(output_bytes != 0));
        resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes);
        resource.peak_live_host_bytes = resource
            .peak_live_host_bytes
            .max((input.as_bytes().len() as u64).saturating_add(output_bytes));
        let allocation = take_pipeline_allocation_telemetry();
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        record_pipeline_resource_telemetry(resource);
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        let fallback_reason = selection.fallback.map_or_else(
            || {
                (requested_backend == Backend::Simd
                    && selection.requested_architecture == crate::compute::Architecture::Default)
                    .then(|| format!("{operation} used CPU Default for a SIMD request"))
            },
            |reason| Some(format!(
                "{operation} selected {:?} after {reason:?} for requested {:?}",
                selection.implementation.architecture,
                selection.requested_architecture,
            )),
        );
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(requested_backend),
            actual_backend: Backend::Cpu,
            operation_count: 1,
            route_ns,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(Some(()))
}

/// Executes one CPU architecture strategy with caller-routed context and phase timings.
pub(crate) fn execute_cpu_arch_operation(
    operation: &'static str,
    input: &DynamicImage,
    context: ExecutionContext,
    requested_backend: Option<Backend>,
    route_ns: u64,
    validation_ns: u64,
    routed_fallback_reason: Option<String>,
    execute: impl FnOnce(
        &ExecutionContext,
    ) -> Result<(DynamicImage, crate::compute::registry::support::StrategySelection), PilError>,
) -> Result<DynamicImage, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }

    if context.backend() != Backend::Cpu {
        if timed {
            discard_eager_image_operation_telemetry();
        }
        return Err(PilError::InternalError(
            "CPU architecture execution received a non-CPU context".into(),
        ));
    }

    if timed {
        begin_pipeline_operation_telemetry(operation);
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (result, selection) = match execute(&context) {
        Ok(result) => result,
        Err(error) => {
            if timed {
                discard_eager_image_operation_telemetry();
            }
            return Err(error);
        }
    };
    let backend_ns = elapsed_ns(backend_start);

    if timed {
        record_cpu_arch_operation_telemetry(
            operation,
            input,
            &result,
            selection,
            requested_backend,
            route_ns,
            validation_ns,
            backend_ns,
            routed_fallback_reason,
        );
    }
    Ok(result)
}

fn record_cpu_arch_operation_telemetry(
    operation: &'static str,
    input: &DynamicImage,
    result: &DynamicImage,
    selection: crate::compute::registry::support::StrategySelection,
    requested_backend: Option<Backend>,
    route_ns: u64,
    validation_ns: u64,
    backend_ns: u64,
    routed_fallback_reason: Option<String>,
) {
    record_pipeline_operation_architecture(selection.implementation.architecture);
    let mode_conversion = u64::from(input.mode() != result.mode());
    if selection.implementation.architecture == crate::compute::Architecture::Default {
        record_pipeline_operation_path("cpu");
    }
    record_pipeline_operation_mode_conversion(mode_conversion);
    finish_pipeline_operation_telemetry();

    let mut resource = host_resource_telemetry(input);
    resource.mode_conversion_count = mode_conversion;
    let output_bytes = result.as_bytes().len();
    let output_bytes_u64 = output_bytes as u64;
    resource.host_buffer_count = resource
        .host_buffer_count
        .saturating_add(u64::from(output_bytes != 0));
    resource.host_buffer_bytes = resource.host_buffer_bytes.saturating_add(output_bytes_u64);
    resource.peak_live_host_bytes = resource
        .peak_live_host_bytes
        .max((input.as_bytes().len() as u64).saturating_add(output_bytes_u64));
    let allocation = take_pipeline_allocation_telemetry();
    resource.host_allocation_count = allocation.allocation_count;
    resource.host_allocated_bytes = allocation.allocated_bytes;
    record_pipeline_resource_telemetry(resource);
    let resource = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let dispatch_count = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    let fallback_reason = selection.fallback.map_or_else(
        || {
            routed_fallback_reason.or_else(|| {
                (requested_backend == Some(Backend::Simd)
                    && selection.requested_architecture == crate::compute::Architecture::Default)
                    .then(|| format!("{operation} used CPU Default for a SIMD request"))
            })
        },
        |reason| {
            Some(format!(
                "{operation} selected {:?} after {reason:?} for requested {:?}",
                selection.implementation.architecture,
                selection.requested_architecture,
            ))
        },
    );
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend,
        actual_backend: Backend::Cpu,
        operation_count: 1,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason,
        resource,
        resize_coeff_cache_hits: 0,
        resize_coeff_cache_misses: 0,
    });
}

/// Execute a CPU operation whose independent result shares immutable pixels
/// with its input until either image is mutated.
pub(crate) fn execute_eager_cpu_shared_result<T>(
    operation: &'static str,
    input: &DynamicImage,
    execute: impl FnOnce() -> Result<T, PilError>,
) -> Result<T, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }

    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = match execute() {
        Ok(result) => result,
        Err(error) => {
            if timed {
                discard_eager_image_operation_telemetry();
            }
            return Err(error);
        }
    };
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        begin_pipeline_operation_telemetry(operation);
        record_pipeline_operation_path("cpu");
        finish_pipeline_operation_telemetry();

        // The returned image retains this existing pixel buffer by Arc; do
        // not count a second buffer or a full-frame copy.
        record_pipeline_resource_telemetry(host_resource_telemetry(input));
        let resource = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let dispatch_count = take_pipeline_dispatch_count();
        let _ = take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(Backend::Cpu),
            actual_backend: Backend::Cpu,
            operation_count: 1,
            route_ns: 0,
            validation_ns: 0,
            backend_ns,
            dispatch_count,
            fallback_reason: None,
            resource,
            resize_coeff_cache_hits: 0,
            resize_coeff_cache_misses: 0,
        });
    }
    Ok(result)
}

#[cfg(feature = "gpu")]
pub(crate) fn gpu_cmyk_projection_terminal_supported(
    image: &DynamicImage,
    ops: &[PipelineOp],
) -> bool {
    pool_gpu::GpuPool::cmyk_projection_terminal_supported(image, ops)
}

#[cfg(feature = "gpu")]
pub(crate) fn gpu_rgb_projection_supported(image: &DynamicImage) -> bool {
    pool_gpu::GpuPool::rgb_projection_supported(image)
}

#[cfg(feature = "gpu")]
pub(crate) fn gpu_rgb_projection_terminal_supported(
    image: &DynamicImage,
    ops: &[PipelineOp],
) -> bool {
    pool_gpu::GpuPool::rgb_projection_terminal_supported(image, ops)
}
