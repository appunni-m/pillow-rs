//! Direct eager operations that select CPU, SIMD, or GPU without a lazy pipeline.
//!
//! TODO(pillow-rs#3ptb): replace the remaining direct SIMD adapter calls with
//! CPU-family strategies selected by the injected execution context.

use super::backend_state::active_lock;
#[cfg(feature = "gpu")]
use super::pool_gpu;
use super::pool_simd;
use super::{
    Backend, DynamicImage, ExecutionContext, PilError, PipelineOp, PipelineResourceTelemetry, PipelineTelemetry,
    begin_pipeline_operation_telemetry, elapsed_ns, finish_pipeline_operation_telemetry,
    host_resource_telemetry, pipeline_telemetry_enabled, pipeline_timestamp,
    record_pipeline_allocation, record_pipeline_operation_handoff,
    record_pipeline_operation_mode_conversion, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_vector_blocks,
    record_pipeline_resource_telemetry, record_pipeline_telemetry,
    reset_pipeline_allocation_telemetry, reset_pipeline_operation_telemetry,
    take_pipeline_allocation_telemetry, take_pipeline_backend_override,
    take_pipeline_dispatch_count, take_pipeline_resize_coeff_cache_stats,
    take_pipeline_resource_telemetry, take_pipeline_telemetry,
};
use std::time::Instant;

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

fn record_eager_simd_generator_telemetry(
    requested_backend: Option<Backend>,
    route_ns: u64,
    validation_ns: u64,
    backend_ns: u64,
    result: &Result<DynamicImage, PilError>,
) {
    if result.is_err() {
        record_pipeline_operation_path("unsupported");
    }
    finish_pipeline_operation_telemetry();

    let resource = result.as_ref().ok().map(|image| {
        let bytes = image.as_bytes().len() as u64;
        let allocation = take_pipeline_allocation_telemetry();
        PipelineResourceTelemetry {
            host_buffer_count: u64::from(bytes != 0),
            host_buffer_bytes: bytes,
            peak_live_host_bytes: bytes,
            host_allocation_count: allocation.allocation_count,
            host_allocated_bytes: allocation.allocated_bytes,
            ..PipelineResourceTelemetry::default()
        }
    });
    if let Some(resource) = resource {
        record_pipeline_resource_telemetry(resource);
    }
    let resource = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let dispatch_count = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend,
        actual_backend: Backend::Simd,
        operation_count: 1,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason: None,
        resource,
        resize_coeff_cache_hits: 0,
        resize_coeff_cache_misses: 0,
    });
}

/// Run the SIMD CMYK projection block scanner and return its vector/tail
/// evidence together with the two Pillow-compatible result axes.
pub(crate) fn simd_cmyk_projection(
    image: &crate::raster::RgbaImage,
) -> (Vec<u32>, Vec<u32>, u64, u64) {
    pool_simd::ops::adapters::simd_cmyk_projection(image)
}

/// Derive CMYK projection axes from a proven zero source and positive writes.
pub(crate) fn simd_cmyk_projection_from_positive_writes(
    width: u32,
    height: u32,
    ops: &[PipelineOp],
) -> Option<(Vec<u32>, Vec<u32>, u64, u64)> {
    pool_simd::ops::adapters::simd_cmyk_projection_from_positive_writes(width, height, ops)
}

pub(crate) fn simd_rgb_projection(
    image: &crate::raster::RgbImage,
) -> (Vec<u32>, Vec<u32>, u64, u64) {
    pool_simd::ops::adapters::simd_rgb_projection(image)
}

pub(crate) fn execute_eager_cpu_getprojection(
    input: &DynamicImage,
    requested_backend: Option<Backend>,
    fallback_reason: Option<String>,
    execute: impl FnOnce() -> Result<(Vec<u32>, Vec<u32>), PilError>,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = execute()?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        begin_pipeline_operation_telemetry("GetProjection");
        record_pipeline_operation_path("cpu");
        finish_pipeline_operation_telemetry();
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

pub(crate) fn execute_eager_simd_getprojection(
    input: &DynamicImage,
    execute: impl FnOnce() -> Result<((Vec<u32>, Vec<u32>), u64, u64), PilError>,
) -> Result<(Vec<u32>, Vec<u32>), PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (result, vector_blocks, scalar_tail) = execute()?;
    let backend_ns = elapsed_ns(backend_start);
    if timed {
        begin_pipeline_operation_telemetry("GetProjection");
        record_pipeline_operation_vector_blocks(vector_blocks);
        record_pipeline_operation_scalar_tail(scalar_tail);
        finish_pipeline_operation_telemetry();

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
            requested_backend: Some(Backend::Simd),
            actual_backend: Backend::Simd,
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
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let context = ExecutionContext::for_routed_backend(requested_backend);
    let route_ns = elapsed_ns(route_start);
    execute_cpu_arch_operation(
        operation,
        input,
        context,
        Some(requested_backend),
        route_ns,
        0,
        None,
        execute,
    )
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
    // TODO(pillow-rs#3ptb): include the selected Architecture in the
    // operation receipt; PipelineTelemetry currently records backend/path only.
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

/// Execute one SIMD image operation directly for a loaded source and publish
/// the vector and buffer evidence for its native kernel. The closure accounts
/// its checked image-buffer allocations through the normal allocation hooks.
pub(crate) fn execute_eager_simd_image_operation(
    operation: &'static str,
    input: &DynamicImage,
    execute: impl FnOnce() -> Result<(DynamicImage, u64, u64, &'static str), PilError>,
) -> Result<DynamicImage, PilError> {
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_eager_image_operation_telemetry();
    }

    let backend_start = timed.then(pipeline_timestamp).flatten();
    let (result, vector_blocks, scalar_tail, path) = match execute() {
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
        record_pipeline_operation_vector_blocks(vector_blocks);
        record_pipeline_operation_scalar_tail(scalar_tail);
        record_pipeline_operation_path(path);
        finish_pipeline_operation_telemetry();

        let mut resource = host_resource_telemetry(input);
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
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: Some(Backend::Simd),
            actual_backend: Backend::Simd,
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

/// Apply the exact native RGB 150% UnsharpMask blend through the SIMD adapter.
///
/// The caller is responsible for selecting the SIMD route and validating the
/// logical image mode before using this narrow byte kernel.
pub(crate) fn simd_unsharp_blend_rgb_150(
    original: &[u8],
    blurred_output: &mut [u8],
    threshold: i32,
) -> bool {
    pool_simd::ops::adapters::simd_unsharp_blend_rgb_150(original, blurred_output, threshold)
}

/// Try the eager `Image.linear_gradient` constructor on the active SIMD
/// backend.
///
/// Eager module-level generators do not have a source image on which to hang
/// a deferred pipeline.  Keep their backend boundary here so strict SIMD
/// parity can observe the actual data-plane executor and so an unsupported
/// SIMD contract returns an error instead of silently entering the CPU loop.
pub(crate) fn try_simd_linear_gradient(mode: &str) -> Result<Option<DynamicImage>, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let active_set = active_lock()?.clone();
    let route_ns = elapsed_ns(route_start);
    if !active_set.contains(&Backend::Simd) {
        return Ok(None);
    }

    if timed {
        reset_eager_image_operation_telemetry();
    }
    let validation_start = timed.then(pipeline_timestamp).flatten();
    let requested_backend = (active_set.len() == 1).then_some(Backend::Simd);
    let validation_ns = elapsed_ns(validation_start);
    if timed {
        begin_pipeline_operation_telemetry("LinearGradient");
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = pool_simd::ops::adapters::simd_linear_gradient_generate(mode);
    let backend_ns = elapsed_ns(backend_start);

    if timed {
        record_eager_simd_generator_telemetry(
            requested_backend,
            route_ns,
            validation_ns,
            backend_ns,
            &result,
        );
    }

    result.map(Some)
}

/// Try the eager native-L `Image.radial_gradient` generator on SIMD.
///
/// Other modes stay on their established constructors until they have their
/// own format-specific vector kernels.
pub(crate) fn try_simd_radial_gradient(mode: &str) -> Result<Option<DynamicImage>, PilError> {
    if mode != "L" {
        return Ok(None);
    }
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let active_set = active_lock()?.clone();
    let route_ns = elapsed_ns(route_start);
    if !active_set.contains(&Backend::Simd) {
        return Ok(None);
    }

    if timed {
        reset_eager_image_operation_telemetry();
    }
    let validation_start = timed.then(pipeline_timestamp).flatten();
    let requested_backend = (active_set.len() == 1).then_some(Backend::Simd);
    let validation_ns = elapsed_ns(validation_start);
    if timed {
        begin_pipeline_operation_telemetry("RadialGradient");
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = pool_simd::ops::adapters::simd_radial_gradient_generate();
    let backend_ns = elapsed_ns(backend_start);

    if timed {
        record_eager_simd_generator_telemetry(
            requested_backend,
            route_ns,
            validation_ns,
            backend_ns,
            &result,
        );
    }

    result.map(Some)
}

/// Execute the eager Mandelbrot generator on the SIMD backend when its native
/// byte kernel can process a full vector per row. Smaller widths retain the
/// exact scalar implementation instead of reporting a SIMD execution that
/// only ran scalar tail pixels.
pub(crate) fn try_simd_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<Option<DynamicImage>, PilError> {
    let (width, height) = size;
    if width < 8 || height == 0 {
        return Ok(None);
    }

    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let active_set = active_lock()?.clone();
    let route_ns = elapsed_ns(route_start);
    if !active_set.contains(&Backend::Simd) {
        return Ok(None);
    }

    if timed {
        reset_eager_image_operation_telemetry();
    }
    let validation_start = timed.then(pipeline_timestamp).flatten();
    let requested_backend = (active_set.len() == 1).then_some(Backend::Simd);
    let validation_ns = elapsed_ns(validation_start);
    if timed {
        begin_pipeline_operation_telemetry("EffectMandelbrot");
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    let result = pool_simd::ops::adapters::simd_mandelbrot_generate(size, extent, quality);
    let backend_ns = elapsed_ns(backend_start);

    if timed {
        record_eager_simd_generator_telemetry(
            requested_backend,
            route_ns,
            validation_ns,
            backend_ns,
            &result,
        );
    }

    result.map(Some)
}

/// Timer and backend request captured for the scalar eager Mandelbrot path.
///
/// Eager constructors bypass the pipeline router, so their direct CPU work
/// needs an explicit receipt instead of leaving benchmark execution unproven.
pub(crate) struct EagerCpuMandelbrotTelemetry {
    timed: bool,
    backend_start: Option<Instant>,
    requested_backend: Option<Backend>,
    fallback_reason: Option<String>,
}

pub(crate) fn begin_eager_cpu_mandelbrot_telemetry() -> EagerCpuMandelbrotTelemetry {
    let timed = pipeline_telemetry_enabled();
    if !timed {
        return EagerCpuMandelbrotTelemetry {
            timed,
            backend_start: None,
            requested_backend: None,
            fallback_reason: None,
        };
    }

    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();

    let active_set = active_lock()
        .map(|active| active.clone())
        .unwrap_or_default();
    let requested_backend = if active_set.len() == 1 {
        active_set.iter().next().copied()
    } else {
        None
    };
    let fallback_reason = active_set
        .iter()
        .any(|backend| *backend != Backend::Cpu)
        .then(|| "eager EffectMandelbrot has no active non-CPU implementation".to_string());
    begin_pipeline_operation_telemetry("EffectMandelbrot");

    EagerCpuMandelbrotTelemetry {
        timed,
        backend_start: pipeline_timestamp(),
        requested_backend,
        fallback_reason,
    }
}

pub(crate) fn finish_eager_cpu_mandelbrot_telemetry(
    telemetry: EagerCpuMandelbrotTelemetry,
    size: (u32, u32),
) {
    if !telemetry.timed {
        return;
    }

    record_pipeline_operation_path("cpu");
    finish_pipeline_operation_telemetry();

    let bytes = u64::from(size.0) * u64::from(size.1);
    let allocation = take_pipeline_allocation_telemetry();
    record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        host_buffer_count: u64::from(bytes != 0),
        host_buffer_bytes: bytes,
        peak_live_host_bytes: bytes,
        host_allocation_count: allocation.allocation_count,
        host_allocated_bytes: allocation.allocated_bytes,
        ..PipelineResourceTelemetry::default()
    });
    let resource = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let dispatch_count = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend: telemetry.requested_backend,
        actual_backend: Backend::Cpu,
        operation_count: 1,
        route_ns: 0,
        validation_ns: 0,
        backend_ns: elapsed_ns(telemetry.backend_start),
        dispatch_count,
        fallback_reason: telemetry.fallback_reason,
        resource,
        resize_coeff_cache_hits: 0,
        resize_coeff_cache_misses: 0,
    });
}
