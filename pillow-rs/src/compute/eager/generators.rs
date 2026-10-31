//! Direct mode/architecture routes for eager generated rasters.

use super::super::backend_state::active_lock;
use super::super::{
    Backend, ExecutionContext, PipelineResourceTelemetry, PipelineTelemetry, elapsed_ns,
    begin_pipeline_operation_telemetry, finish_pipeline_operation_telemetry,
    pipeline_telemetry_enabled, pipeline_timestamp, record_pipeline_operation_architecture,
    record_pipeline_operation_mode_conversion, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_vector_blocks,
    record_pipeline_resource_telemetry, record_pipeline_telemetry,
    reset_pipeline_allocation_telemetry, reset_pipeline_operation_telemetry,
    take_pipeline_allocation_telemetry, take_pipeline_backend_override,
    take_pipeline_dispatch_count, take_pipeline_resize_coeff_cache_stats,
    take_pipeline_resource_telemetry,
};
use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

struct GeneratorRoute {
    context: ExecutionContext,
    requested_backend: Option<Backend>,
    route_ns: u64,
    timed: bool,
}

fn route_generator() -> Result<GeneratorRoute, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let active_set = active_lock()?.clone();
    let routed_backend = if active_set.contains(&Backend::Simd) {
        Backend::Simd
    } else {
        Backend::Cpu
    };
    let requested_backend = (active_set.len() == 1)
        .then(|| active_set.iter().next().copied())
        .flatten();
    Ok(GeneratorRoute {
        context: ExecutionContext::for_routed_backend(routed_backend),
        requested_backend,
        route_ns: elapsed_ns(route_start),
        timed,
    })
}

fn begin_generator_receipt(operation: &'static str, route: &GeneratorRoute) {
    if !route.timed {
        return;
    }
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    begin_pipeline_operation_telemetry(operation);
}

fn discard_generator_receipt(route: &GeneratorRoute) {
    if !route.timed {
        return;
    }
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
}

fn record_generator_receipt(
    operation: &'static str,
    route: &GeneratorRoute,
    validation_ns: u64,
    backend_ns: u64,
    selection: StrategySelection,
    generated: &GeneratedImage,
) {
    if !route.timed {
        return;
    }
    record_pipeline_operation_architecture(selection.implementation.architecture);
    record_pipeline_operation_vector_blocks(generated.vector_blocks);
    record_pipeline_operation_scalar_tail(generated.scalar_tail);
    record_pipeline_operation_path(if selection.implementation.architecture
        == crate::compute::Architecture::Default
    {
        "cpu"
    } else {
        "vector"
    });
    record_pipeline_operation_mode_conversion(0);
    finish_pipeline_operation_telemetry();

    let output_bytes = generated.image.as_bytes().len() as u64;
    let allocation = take_pipeline_allocation_telemetry();
    record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        host_buffer_count: u64::from(output_bytes != 0),
        host_buffer_bytes: output_bytes,
        peak_live_host_bytes: output_bytes,
        host_allocation_count: allocation.allocation_count,
        host_allocated_bytes: allocation.allocated_bytes,
        ..PipelineResourceTelemetry::default()
    });
    let resource = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let dispatch_count = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend: route.requested_backend,
        actual_backend: Backend::Cpu,
        operation_count: 1,
        route_ns: route.route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason: generator_fallback(operation, route, selection),
        resource,
        resize_coeff_cache_hits: 0,
        resize_coeff_cache_misses: 0,
    });
}

fn generator_fallback(
    operation: &'static str,
    route: &GeneratorRoute,
    selection: StrategySelection,
) -> Option<String> {
    selection
        .fallback
        .map(|reason| {
            format!(
                "{operation} selected {:?} after {reason:?} for requested {:?}",
                selection.implementation.architecture,
                selection.requested_architecture,
            )
        })
        .or_else(|| {
            (route.requested_backend == Some(Backend::Gpu)).then(|| {
                format!("{operation} has no GPU implementation; CPU Default was selected")
            })
        })
        .or_else(|| {
            (route.requested_backend == Some(Backend::Simd)
                && selection.requested_architecture == crate::compute::Architecture::Default)
                .then(|| format!("{operation} used CPU Default for a SIMD request"))
        })
}

pub(crate) fn execute_eager_cpu_linear_gradient(mode: Mode) -> Result<DynamicImage, PilError> {
    let route = route_generator()?;
    let validation_start = route.timed.then(pipeline_timestamp).flatten();
    let prepared = crate::compute::pool_cpu::ops::effects::ops::prepare_linear_gradient(
        mode,
        &route.context,
    )?;
    let validation_ns = elapsed_ns(validation_start);
    let selection = *prepared.selection();
    begin_generator_receipt("LinearGradient", &route);
    let backend_start = route.timed.then(pipeline_timestamp).flatten();
    let generated = match prepared.execute() {
        Ok(generated) => generated,
        Err(error) => {
            discard_generator_receipt(&route);
            return Err(error);
        }
    };
    record_generator_receipt(
        "LinearGradient",
        &route,
        validation_ns,
        elapsed_ns(backend_start),
        selection,
        &generated,
    );
    Ok(generated.image)
}

pub(crate) fn execute_eager_cpu_radial_gradient(mode: Mode) -> Result<DynamicImage, PilError> {
    let route = route_generator()?;
    let validation_start = route.timed.then(pipeline_timestamp).flatten();
    let prepared = crate::compute::pool_cpu::ops::effects::ops::prepare_radial_gradient(
        mode,
        &route.context,
    )?;
    let validation_ns = elapsed_ns(validation_start);
    let selection = *prepared.selection();
    begin_generator_receipt("RadialGradient", &route);
    let backend_start = route.timed.then(pipeline_timestamp).flatten();
    let generated = match prepared.execute() {
        Ok(generated) => generated,
        Err(error) => {
            discard_generator_receipt(&route);
            return Err(error);
        }
    };
    record_generator_receipt(
        "RadialGradient",
        &route,
        validation_ns,
        elapsed_ns(backend_start),
        selection,
        &generated,
    );
    Ok(generated.image)
}

pub(crate) fn execute_eager_cpu_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<DynamicImage, PilError> {
    let route = route_generator()?;
    let validation_start = route.timed.then(pipeline_timestamp).flatten();
    let prepared = crate::compute::pool_cpu::ops::effects::ops::prepare_mandelbrot(
        size,
        extent,
        quality,
        &route.context,
    )?;
    let validation_ns = elapsed_ns(validation_start);
    let selection = *prepared.selection();
    begin_generator_receipt("EffectMandelbrot", &route);
    let backend_start = route.timed.then(pipeline_timestamp).flatten();
    let generated = match prepared.execute() {
        Ok(generated) => generated,
        Err(error) => {
            discard_generator_receipt(&route);
            return Err(error);
        }
    };
    record_generator_receipt(
        "EffectMandelbrot",
        &route,
        validation_ns,
        elapsed_ns(backend_start),
        selection,
        &generated,
    );
    Ok(generated.image)
}
