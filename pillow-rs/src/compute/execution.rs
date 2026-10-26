//! Routing, validating, and executing materialized pipeline batches.

use super::automatic_segments;
use super::backend_state::{active_lock, pools};
#[cfg(feature = "gpu")]
use super::pool_gpu;
use super::pool_simd;
use super::registry;
use super::{
    Backend, BackendImpl, DynamicImage, PilError, PipelineOp, PipelineResourceTelemetry,
    PipelineTelemetry, begin_pipeline_operation_telemetry, elapsed_ns,
    finish_pipeline_operation_telemetry, host_resource_telemetry, pipeline_telemetry_enabled,
    pipeline_timestamp, record_pipeline_allocation, record_pipeline_backend_fallback,
    record_pipeline_operation_handoff, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_unsupported,
    record_pipeline_operation_vector_blocks, record_pipeline_resource_telemetry,
    record_pipeline_telemetry, reset_pipeline_allocation_telemetry,
    reset_pipeline_operation_telemetry, take_pipeline_allocation_telemetry,
    take_pipeline_backend_override, take_pipeline_dispatch_count,
    take_pipeline_resize_coeff_cache_stats, take_pipeline_resource_telemetry,
};
use std::sync::Arc;
use std::time::Instant;

mod terminals;
pub(crate) use terminals::{execute_prepared_cpu_terminal, execute_prepared_simd_terminal};
#[cfg(feature = "gpu")]
pub(crate) use terminals::{
    execute_prepared_gpu_cmyk_projection_terminal, execute_prepared_gpu_rgb_projection_terminal,
};

// ── Router ─────────────────────────────────────────────────────────────────

fn route_decision(
    ops: &[PipelineOp],
    explicit: Option<Backend>,
) -> Result<(Backend, Option<String>, bool), PilError> {
    if let Some(b) = explicit {
        // An explicit request must still cross the normal validation boundary;
        // callers may request an inactive or unsupported backend deliberately.
        return Ok((b, None, false));
    }
    // Snapshot activation policy before walking the operation list. Support
    // checks can inspect registry metadata and may grow as the descriptor
    // table expands; holding the global policy mutex across that scan would
    // serialize unrelated pipeline construction and backend toggles.
    let active_set = active_lock()?.clone();
    let mut fallback_reason = None;
    for pool in pools() {
        if active_set.contains(&pool.name()) {
            let mut supports_all = true;
            let mut supports_any = false;
            for op in ops {
                if pool.supports(op)? {
                    supports_any = true;
                } else {
                    supports_all = false;
                    if pool.name() != Backend::Cpu && fallback_reason.is_none() {
                        fallback_reason = Some(format!(
                            "{} does not support {}",
                            backend_label(pool.name()),
                            registry::variant_key(op)
                        ));
                    }
                    // SIMD can execute a mixed host-resident pipeline. Keep
                    // scanning its descriptors so an operation after a
                    // CPU-only boundary can still select the SIMD planner.
                    // GPU remains an all-operations route and stops at the
                    // first unsupported descriptor.
                    if pool.name() != Backend::Simd {
                        break;
                    }
                }
            }
            if pool.name() == Backend::Simd && supports_any {
                return Ok((pool.name(), fallback_reason, true));
            }
            if supports_all {
                // The support scan above is the validation pass for automatic
                // routing. Carry that fact to preparation so the same
                // registry queries are not repeated immediately before the
                // backend executor runs.
                return Ok((pool.name(), fallback_reason, true));
            }
        }
    }
    Ok((
        Backend::Cpu,
        fallback_reason.or_else(|| {
            active_set
                .iter()
                .any(|backend| *backend != Backend::Cpu)
                .then(|| "no active non-CPU backend supports the complete pipeline".to_string())
        }),
        false,
    )) // universal fallback
}

fn backend_label(backend: Backend) -> &'static str {
    match backend {
        Backend::Cpu => "CPU",
        Backend::Gpu => "GPU",
        Backend::Simd => "SIMD",
    }
}

/// Describe an automatic SIMD-to-CPU handoff without misclassifying a valid
/// public operation as an unsupported outcome.  Contextual SIMD capability is
/// checked only after the image layout and logical mode are known; when that
/// check rejects a step, CPU remains the exact semantic executor and the
/// receipt should expose that fact while retaining the operation key for
/// diagnosis.
fn simd_host_control_reason(op: &PipelineOp) -> String {
    format!(
        "exact host semantic control: SIMD image-layout guard for {}",
        registry::variant_key(op)
    )
}

fn is_simd_capability_error(error: &PilError) -> bool {
    matches!(error, PilError::NotImplementedError(message) if message.starts_with("SIMD "))
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedExecution {
    requested_backend: Option<Backend>,
    selected_backend: Backend,
    route_ns: u64,
    validation_ns: u64,
    fallback_reason: Option<String>,
}

impl PreparedExecution {
    pub(crate) fn is_serial_cpu_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Cpu && self.fallback_reason.is_none()
    }

    pub(crate) fn is_simd_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Simd && self.fallback_reason.is_none()
    }

    pub(crate) fn is_gpu_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Gpu && self.fallback_reason.is_none()
    }
}

/// Performs the route and validation phases before a source image is
/// materialized. Keeping this separate preserves the explicit-backend contract
/// while allowing the actual backend phase to be measured later.
pub(crate) fn prepare_execution(
    ops: &[PipelineOp],
    requested_backend: Option<Backend>,
) -> Result<PreparedExecution, PilError> {
    let timed = pipeline_telemetry_enabled();
    let route_start = timed.then(pipeline_timestamp).flatten();
    let (selected_backend, fallback_reason, support_checked) =
        route_decision(ops, requested_backend)?;
    let route_ns = elapsed_ns(route_start);

    let validation_start = timed.then(pipeline_timestamp).flatten();
    if !support_checked {
        validate_backend_support(selected_backend, ops)?;
    }
    let validation_ns = elapsed_ns(validation_start);

    Ok(PreparedExecution {
        requested_backend,
        selected_backend,
        route_ns,
        validation_ns,
        fallback_reason,
    })
}

/// Validates that a compiled backend has native support for every operation.
///
/// Image pipelines call this before materializing their source so an explicit
/// backend lock reports the first unsupported operation before any decode or
/// nested-pipeline error.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for an unavailable backend or the first
/// operation without a native implementation on the selected backend.
pub fn validate_backend_support(backend: Backend, ops: &[PipelineOp]) -> Result<(), PilError> {
    let pool = pools()
        .iter()
        .find(|pool| pool.name() == backend)
        .ok_or_else(|| PilError::ValueError(format!("Backend {:?} not available", backend)))?;
    for op in ops {
        if pool.supports(op)? {
            continue;
        }
        // GPU strict parity must reach the executor even when a public
        // operation has not yet acquired a native shader contract.  The GPU
        // pool performs its contextual checks after materialization and can
        // run the exact host implementation for that operation, preserving
        // Pillow's result/error semantics instead of manufacturing an
        // operation-level NotImplementedError before the image is seen.
        // Keep genuinely descriptor-only operations (no CPU implementation)
        // on the existing validation error path.
        if backend == Backend::Gpu && registry::cpu_supports(op)? {
            continue;
        }
        // GPU convolution support is contextual: the same shader is exact for
        // byte layouts only for the operation-level fixed-point subset, while
        // the I-mode path applies the normalized f32 kernel to signed samples.
        // The source image is intentionally not materialized in this phase,
        // so defer these two operation-only checks to the GPU's image-aware
        // preflight instead of rejecting valid I-mode filters prematurely.
        if backend == Backend::Gpu
            && matches!(
                op,
                PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
            )
        {
            continue;
        }
        if matches!(backend, Backend::Simd | Backend::Gpu) {
            let key = registry::variant_key(op);
            record_pipeline_operation_unsupported(key);
            return Err(PilError::NotImplementedError(format!(
                "{} does not support {key}",
                backend_label(backend)
            )));
        }
        let name = match backend {
            Backend::Cpu => "CPU",
            Backend::Gpu => "GPU",
            Backend::Simd => "SIMD",
        };
        return Err(PilError::ValueError(format!(
            "{name}: no native impl for {}",
            registry::variant_key(op)
        )));
    }
    Ok(())
}

pub(crate) fn merge_pipeline_resource_telemetry(
    total: &mut Option<PipelineResourceTelemetry>,
    next: Option<PipelineResourceTelemetry>,
) {
    let Some(next) = next else {
        return;
    };
    let total = total.get_or_insert_with(PipelineResourceTelemetry::default);
    total.upload_bytes = total.upload_bytes.saturating_add(next.upload_bytes);
    total.readback_bytes = total.readback_bytes.saturating_add(next.readback_bytes);
    total.auxiliary_bytes = total.auxiliary_bytes.saturating_add(next.auxiliary_bytes);
    total.parameter_bytes = total.parameter_bytes.saturating_add(next.parameter_bytes);
    total.retained_cache_bytes = total.retained_cache_bytes.max(next.retained_cache_bytes);
    total.full_frame_copy_count = total
        .full_frame_copy_count
        .saturating_add(next.full_frame_copy_count);
    total.mode_conversion_count = total
        .mode_conversion_count
        .saturating_add(next.mode_conversion_count);
    total.host_buffer_count = total
        .host_buffer_count
        .saturating_add(next.host_buffer_count);
    total.host_buffer_bytes = total
        .host_buffer_bytes
        .saturating_add(next.host_buffer_bytes);
    total.peak_live_host_bytes = total.peak_live_host_bytes.max(next.peak_live_host_bytes);
    total.fused_operation_count = total
        .fused_operation_count
        .saturating_add(next.fused_operation_count);
    total.host_allocation_count = total
        .host_allocation_count
        .saturating_add(next.host_allocation_count);
    total.host_allocated_bytes = total
        .host_allocated_bytes
        .saturating_add(next.host_allocated_bytes);
}

/// Execute an automatic CPU/SIMD pipeline as contiguous host-resident
/// segments.
///
/// This planner is intentionally conservative about segment extension. A
/// segment may include operations that preserve the current concrete layout,
/// plus a consecutive transpose run that the SIMD executor composes against
/// the original input. Other shape/mode-changing operations end the segment.
/// The next operation is then checked against the actual owned output, so no
/// backend is entered with stale capability information and no Python-level
/// materialization is needed at a CPU↔SIMD boundary.
pub(super) fn execute_automatic_simd_segments(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<
    (
        DynamicImage,
        Backend,
        Option<String>,
        Option<PipelineResourceTelemetry>,
    ),
    PilError,
> {
    let simd = pools()
        .iter()
        .find(|pool| pool.name() == Backend::Simd)
        .ok_or_else(|| PilError::ValueError("SIMD backend not available".into()))?;
    let cpu = pools()
        .iter()
        .find(|pool| pool.name() == Backend::Cpu)
        .ok_or_else(|| PilError::ValueError("CPU backend not available".into()))?;

    let mut current: Option<DynamicImage> = None;
    let mut previous_backend = None;
    let mut last_backend = None;
    let mut fallback_reason = None;
    let mut resources = None;
    let mut index = 0usize;
    let mut current_mode = pool_simd::ops::adapters::simd_initial_mode(img, ops, mode);

    while index < ops.len() {
        let input = current.as_ref().unwrap_or(img);
        let op_mode = current_mode.as_deref();
        let backend = if simd.supports_for_image(&ops[index], input, op_mode)? {
            Backend::Simd
        } else {
            fallback_reason.get_or_insert_with(|| simd_host_control_reason(&ops[index]));
            Backend::Cpu
        };

        let mut end = index + 1;
        if backend == Backend::Simd {
            // Every operation in this segment is checked against the same
            // concrete layout. A transpose-only run is the exception: the
            // SIMD batch executor composes it before dispatch, and each
            // transform is checked against this original image layout.
            while end < ops.len()
                && automatic_segments::can_extend(&ops[end - 1], &ops[end])
                && simd.supports_for_image(&ops[end], input, op_mode)?
            {
                end += 1;
            }
        } else {
            // CPU can absorb adjacent CPU-only work, including draw batches,
            // as long as the current layout contract remains unchanged. Stop
            // before an operation that SIMD can execute so the next segment
            // can return to the vector path without replaying this segment.
            while end < ops.len()
                && automatic_segments::preserves_native_contract(&ops[end - 1])
                && !simd.supports_for_image(&ops[end], input, op_mode)?
            {
                if !cpu.supports(&ops[end])? {
                    break;
                }
                end += 1;
            }
        }

        if previous_backend.is_some_and(|previous| previous != backend) {
            record_pipeline_operation_handoff(1);
        }
        let pool = if backend == Backend::Simd { simd } else { cpu };
        let segment_ops = &ops[index..end];
        let next = pool.execute_batch(segment_ops, input, op_mode)?;
        merge_pipeline_resource_telemetry(&mut resources, take_pipeline_resource_telemetry());
        current = Some(next);
        previous_backend = Some(backend);
        // Automatic routing can mix host-resident SIMD and CPU segments. The
        // receipt must identify the executor that produced the returned
        // pixels, while operation telemetry preserves every segment and
        // handoff. Pillow's sequential operation semantics expose no composite
        // backend identity, so reporting any SIMD use would mislabel a final
        // CPU segment as SIMD.
        last_backend = Some(backend);
        for op in segment_ops {
            current_mode =
                pool_simd::ops::adapters::simd_mode_after_op(op, current_mode.as_deref());
        }
        index = end;
    }

    let result = current.unwrap_or_else(|| img.clone());
    if let Some(resource) = resources {
        record_pipeline_resource_telemetry(resource);
    }
    let actual_backend = last_backend.unwrap_or(Backend::Cpu);
    Ok((result, actual_backend, fallback_reason, resources))
}

/// Executes a previously routed and validated pipeline, recording the bounded
/// backend phase when telemetry is enabled.
pub(crate) fn execute_prepared(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // A pure conversion of an empty image has no samples to transform. Return
    // the correctly typed empty storage directly; this is not a CPU fallback
    // and must not publish a fabricated backend receipt.
    if let Some(result) = empty_convert_result(ops, img) {
        return Ok(result);
    }
    // Quantize returns a palette-index pipeline with an intentionally empty
    // operation list so its palette metadata can remain attached to the
    // lazy result.  There is no backend work at this boundary and therefore
    // no receipt to publish; in particular, do not clear or overwrite a
    // preceding deferred operation's telemetry with a zero-operation sample.
    if ops.is_empty() {
        return Ok(img.clone());
    }
    let timed = pipeline_telemetry_enabled();
    if timed {
        reset_execution_telemetry();
    }
    let backend_start = timed.then(pipeline_timestamp).flatten();
    if prepared.requested_backend.is_none() && prepared.selected_backend == Backend::Simd {
        return execute_automatic_simd(prepared, ops, img, mode, timed, backend_start);
    }
    validate_explicit_simd_support(prepared, ops, img, mode)?;
    execute_prepared_on_backend(prepared, ops, img, mode, timed, backend_start)
}

fn empty_convert_result(ops: &[PipelineOp], img: &DynamicImage) -> Option<DynamicImage> {
    if img.width() != 0 && img.height() != 0 {
        return None;
    }
    let [
        PipelineOp::Convert {
            mode,
            matrix: None,
            dither: None,
        },
    ] = ops
    else {
        return None;
    };
    let width = img.width();
    let height = img.height();
    Some(match mode {
        crate::pipeline::ColorMode::L => {
            DynamicImage::ImageLuma8(crate::raster::GrayImage::new(width, height))
        }
        crate::pipeline::ColorMode::LA => {
            DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(width, height))
        }
        crate::pipeline::ColorMode::LaPremultiplied => {
            DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(width, height))
        }
        crate::pipeline::ColorMode::RGB
        | crate::pipeline::ColorMode::YCbCr
        | crate::pipeline::ColorMode::HSV => {
            DynamicImage::ImageRgb8(crate::raster::RgbImage::new(width, height))
        }
        crate::pipeline::ColorMode::RGBA
        | crate::pipeline::ColorMode::RgbPremultiplied
        | crate::pipeline::ColorMode::Rgbx
        | crate::pipeline::ColorMode::CMYK
        | crate::pipeline::ColorMode::I
        | crate::pipeline::ColorMode::F => {
            DynamicImage::ImageRgba8(crate::raster::RgbaImage::new(width, height))
        }
        crate::pipeline::ColorMode::I16 => {
            DynamicImage::ImageLuma16(crate::raster::ImageBuffer::new(width, height))
        }
        crate::pipeline::ColorMode::P | crate::pipeline::ColorMode::Mode1 => return None,
    })
}

fn reset_execution_telemetry() {
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
}

fn execute_automatic_simd(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
    timed: bool,
    backend_start: Option<Instant>,
) -> Result<DynamicImage, PilError> {
    let (result, actual_backend, mixed_reason, mut resource) =
        execute_automatic_simd_segments(ops, img, mode)?;
    if timed {
        let allocation = take_pipeline_allocation_telemetry();
        if let Some(resource) = resource.as_mut() {
            resource.host_allocation_count = allocation.allocation_count;
            resource.host_allocated_bytes = allocation.allocated_bytes;
        }
        // The segment executor publishes the aggregate so the ordinary
        // receipt boundary remains compatible with single-backend runs.
        let _ = take_pipeline_resource_telemetry();
        let _ = take_pipeline_backend_override();
        let _ = take_pipeline_dispatch_count();
        let (resize_coeff_cache_hits, resize_coeff_cache_misses) =
            take_pipeline_resize_coeff_cache_stats();
        record_pipeline_telemetry(PipelineTelemetry {
            requested_backend: prepared.requested_backend,
            actual_backend,
            operation_count: ops.len(),
            route_ns: prepared.route_ns,
            validation_ns: prepared.validation_ns,
            backend_ns: elapsed_ns(backend_start),
            dispatch_count: None,
            fallback_reason: mixed_reason.or_else(|| prepared.fallback_reason.clone()),
            resource,
            resize_coeff_cache_hits,
            resize_coeff_cache_misses,
        });
    }
    Ok(result)
}

fn validate_explicit_simd_support(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<(), PilError> {
    if prepared.selected_backend != Backend::Simd
        || prepared.requested_backend != Some(Backend::Simd)
    {
        return Ok(());
    }
    if let Some(op) = pool_simd::ops::adapters::first_unsupported_simd_op(img, ops, mode) {
        let key = registry::variant_key(op);
        record_pipeline_operation_unsupported(key);
        return Err(PilError::NotImplementedError(format!(
            "SIMD does not support {key} for the current image layout/mode"
        )));
    }
    Ok(())
}

fn execute_prepared_on_backend(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
    timed: bool,
    backend_start: Option<Instant>,
) -> Result<DynamicImage, PilError> {
    let effective_backend = prepared.selected_backend;
    let pool = pools()
        .iter()
        .find(|pool| pool.name() == effective_backend)
        .ok_or_else(|| {
            PilError::ValueError(format!(
                "Backend {:?} not available",
                prepared.selected_backend
            ))
        })?;
    let estimated_dispatch_count = timed.then(|| pool.dispatch_count(ops)).flatten();
    let mut execution_backend = effective_backend;
    let mut fallback_reason = prepared.fallback_reason.clone();
    let execution = execute_pool_with_simd_recovery(
        prepared,
        pool.as_ref(),
        ops,
        img,
        mode,
        timed,
        &mut execution_backend,
        &mut fallback_reason,
    );
    match execution {
        Ok(result) => {
            if timed {
                record_prepared_backend_telemetry(
                    prepared,
                    ops,
                    backend_start,
                    execution_backend,
                    fallback_reason,
                    estimated_dispatch_count,
                );
            }
            Ok(result)
        }
        Err(error) => {
            if timed {
                clear_failed_execution_telemetry();
            }
            Err(error)
        }
    }
}

fn execute_pool_with_simd_recovery(
    prepared: &PreparedExecution,
    pool: &dyn BackendImpl,
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
    timed: bool,
    effective_backend: &mut Backend,
    fallback_reason: &mut Option<String>,
) -> Result<DynamicImage, PilError> {
    let execution = if prepared.requested_backend == Some(Backend::Gpu) {
        pool.execute_batch_strict(ops, img, mode)
    } else {
        pool.execute_batch(ops, img, mode)
    };
    match execution {
        Err(error)
            if *effective_backend == Backend::Simd
                && prepared.requested_backend != Some(Backend::Simd)
                && is_simd_capability_error(&error) =>
        {
            retry_simd_capability_failure(ops, img, mode, timed, effective_backend, fallback_reason)
        }
        result => result,
    }
}

fn retry_simd_capability_failure(
    ops: &[PipelineOp],
    img: &DynamicImage,
    mode: Option<&str>,
    timed: bool,
    effective_backend: &mut Backend,
    fallback_reason: &mut Option<String>,
) -> Result<DynamicImage, PilError> {
    // SIMD never calls CPU from inside an adapter. If a later operation
    // changes layout, retry the original host image through the CPU pool.
    *effective_backend = Backend::Cpu;
    let host_control_reason = pool_simd::ops::adapters::first_unsupported_simd_op(img, ops, mode)
        .map(simd_host_control_reason)
        .unwrap_or_else(|| {
            "exact host semantic control: SIMD runtime capability boundary".to_owned()
        });
    fallback_reason.get_or_insert_with(|| host_control_reason.clone());
    if timed {
        reset_execution_telemetry();
    }
    record_pipeline_backend_fallback(host_control_reason);
    let cpu = pools()
        .iter()
        .find(|candidate| candidate.name() == Backend::Cpu)
        .ok_or_else(|| PilError::ValueError("CPU backend not available".into()))?;
    cpu.execute_batch(ops, img, mode)
}

fn record_prepared_backend_telemetry(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    backend_start: Option<Instant>,
    effective_backend: Backend,
    contextual_fallback_reason: Option<String>,
    estimated_dispatch_count: Option<u64>,
) {
    let allocation = take_pipeline_allocation_telemetry();
    let resource = take_pipeline_resource_telemetry().map(|mut resource| {
        resource.host_allocation_count = allocation.allocation_count;
        resource.host_allocated_bytes = allocation.allocated_bytes;
        resource
    });
    let backend_override = take_pipeline_backend_override();
    let observed_dispatch_count = take_pipeline_dispatch_count();
    let actual_backend = backend_override
        .as_ref()
        .map(|(backend, _)| *backend)
        // Contextual support is checked after the source image is available.
        // Report the executor that actually produced the pixels.
        .unwrap_or(effective_backend);
    let fallback_reason = backend_override
        .as_ref()
        .map(|(_, reason)| reason.clone())
        .or(contextual_fallback_reason);
    let (resize_coeff_cache_hits, resize_coeff_cache_misses) =
        take_pipeline_resize_coeff_cache_stats();
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend: prepared.requested_backend,
        actual_backend,
        operation_count: ops.len(),
        route_ns: prepared.route_ns,
        validation_ns: prepared.validation_ns,
        backend_ns: elapsed_ns(backend_start),
        dispatch_count: (actual_backend == effective_backend)
            .then_some(observed_dispatch_count.or(estimated_dispatch_count))
            .flatten(),
        fallback_reason,
        resource,
        resize_coeff_cache_hits,
        resize_coeff_cache_misses,
    });
}

fn clear_failed_execution_telemetry() {
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
}

/// Executes a prepared pipeline while retaining an immutable source buffer
/// when a proven I-mode 3x3 identity filter can share it safely.
///
/// The source and result images remain independent at the public API: image
/// mutation uses `Arc::make_mut`, so sharing here only avoids copying when the
/// filtered result remains read-only.
pub(crate) fn execute_prepared_shared(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &Arc<DynamicImage>,
    mode: Option<&str>,
) -> Result<Arc<DynamicImage>, PilError> {
    if matches!(prepared.selected_backend, Backend::Cpu | Backend::Simd)
        && mode == Some("I")
        && ops.len() == 1
    {
        if let PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } = &ops[0]
        {
            if matches!(img.as_ref(), DynamicImage::ImageRgba8(_)) {
                let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
                let timed = pipeline_telemetry_enabled();
                let backend_start = timed.then(pipeline_timestamp).flatten();
                let identity = match prepared.selected_backend {
                    Backend::Cpu => {
                        crate::compute::pool_cpu::ops::filter::uniform_i32_filter3x3_is_identity(
                            img.as_bytes(),
                            img.width() as usize,
                            img.height() as usize,
                            &normalized_kernel,
                            offset + 0.5,
                        )
                    }
                    Backend::Simd => {
                        crate::compute::pool_simd::ops::adapters::uniform_i32_filter3x3_is_identity_simd(
                            img.as_bytes(),
                            img.width() as usize,
                            img.height() as usize,
                            &normalized_kernel,
                            offset + 0.5,
                        )
                    }
                    Backend::Gpu => false,
                };
                if identity {
                    if timed {
                        reset_pipeline_allocation_telemetry();
                        reset_pipeline_operation_telemetry();
                        let _ = take_pipeline_resource_telemetry();
                        let _ = take_pipeline_backend_override();
                        let _ = take_pipeline_dispatch_count();
                        let _ = take_pipeline_resize_coeff_cache_stats();
                        begin_pipeline_operation_telemetry(registry::variant_key(&ops[0]));
                        record_pipeline_operation_path(
                            if prepared.selected_backend == Backend::Cpu {
                                "cpu"
                            } else {
                                "scalar-control"
                            },
                        );
                        if prepared.selected_backend == Backend::Simd {
                            let bytes = img.as_bytes().len();
                            record_pipeline_operation_vector_blocks((bytes / 16) as u64);
                            record_pipeline_operation_scalar_tail((bytes % 16) as u64);
                        }
                        finish_pipeline_operation_telemetry();
                        let _ = take_pipeline_allocation_telemetry();
                        record_pipeline_telemetry(PipelineTelemetry {
                            requested_backend: prepared.requested_backend,
                            actual_backend: prepared.selected_backend,
                            operation_count: ops.len(),
                            route_ns: prepared.route_ns,
                            validation_ns: prepared.validation_ns,
                            backend_ns: elapsed_ns(backend_start),
                            dispatch_count: None,
                            fallback_reason: prepared.fallback_reason.clone(),
                            resource: Some(PipelineResourceTelemetry::default()),
                            resize_coeff_cache_hits: 0,
                            resize_coeff_cache_misses: 0,
                        });
                    }
                    return Ok(Arc::clone(img));
                }
            }
        }
    }

    execute_prepared(prepared, ops, img, mode).map(Arc::new)
}
