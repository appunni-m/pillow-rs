//! Routing, validating, and executing materialized pipeline batches.

use super::backend_state::{active_lock, pools};
#[cfg(feature = "gpu")]
use super::pool_gpu;
use super::registry;
use super::{
    Backend, DynamicImage, ExecutionContext, PilError, PipelineOp, PipelineResourceTelemetry,
    Mode, PipelineTelemetry, begin_pipeline_operation_telemetry, elapsed_ns,
    finish_pipeline_operation_telemetry, host_resource_telemetry, pipeline_telemetry_enabled,
    pipeline_timestamp, record_pipeline_allocation, record_pipeline_operation_path,
    record_pipeline_operation_scalar_tail, record_pipeline_operation_unsupported,
    record_pipeline_operation_vector_blocks, record_pipeline_resource_telemetry,
    record_pipeline_telemetry, reset_pipeline_allocation_telemetry,
    reset_pipeline_operation_telemetry, take_pipeline_allocation_telemetry,
    take_pipeline_backend_override, take_pipeline_dispatch_count,
    take_pipeline_resize_coeff_cache_stats, take_pipeline_resource_telemetry,
};
use std::sync::Arc;
use std::time::Instant;
use crate::compute::registry::support::PaletteInput;

mod terminals;
pub(crate) use terminals::{
    execute_prepared_cpu_arch_terminal, execute_prepared_cpu_terminal,
};
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
    let simd_alias_active = active_set.contains(&Backend::Simd);
    let mut fallback_reason = None;
    for pool in pools() {
        if active_set.contains(&pool.name()) {
            let mut supports_all = true;
            for op in ops {
                if !pool.supports(op)? {
                    supports_all = false;
                    if pool.name() != Backend::Cpu && fallback_reason.is_none() {
                        fallback_reason = Some(format!(
                            "{} does not support {}",
                            backend_label(pool.name()),
                            registry::variant_key(op)
                        ));
                    }
                    break;
                }
            }
            if pool.name() == Backend::Cpu && simd_alias_active && supports_all {
                return Ok((Backend::Simd, fallback_reason, true));
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
    if simd_alias_active {
        let mut supports_all = true;
        for op in ops {
            if !registry::cpu_supports(op)? {
                supports_all = false;
                break;
            }
        }
        if supports_all {
            return Ok((Backend::Simd, fallback_reason, true));
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
        Backend::Simd => "CPU architecture",
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedExecution {
    requested_backend: Option<Backend>,
    selected_backend: Backend,
    context: ExecutionContext,
    route_ns: u64,
    validation_ns: u64,
    fallback_reason: Option<String>,
}

impl PreparedExecution {
    pub(crate) fn context(&self) -> &ExecutionContext {
        &self.context
    }

    pub(crate) fn is_serial_cpu_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Cpu && self.fallback_reason.is_none()
    }

    pub(crate) fn is_cpu_architecture_alias_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Simd && self.fallback_reason.is_none()
    }

    pub(crate) fn is_gpu_without_fallback(&self) -> bool {
        self.selected_backend == Backend::Gpu && self.fallback_reason.is_none()
    }
}

/// Executes one family-prepared CPU operation using the pipeline's routed context.
pub(crate) fn execute_prepared_cpu_arch_operation(
    prepared: &PreparedExecution,
    op: &PipelineOp,
    input: crate::compute::registry::support::OperationInputs<'_>,
) -> Result<DynamicImage, PilError> {
    super::eager::execute_cpu_arch_operation(
        registry::variant_key(op),
        input.image,
        prepared.context,
        prepared.requested_backend,
        prepared.route_ns,
        prepared.validation_ns,
        prepared.fallback_reason.clone(),
        |context| {
            let operation = registry::prepare_cpu(op, input, context)?;
            let selection = *operation.selection();
            operation.execute().map(|result| (result, selection))
        },
    )
}

/// Executes a palette-target Convert while retaining its wrapper output data.
///
/// The raster strategy owns indices and per-pixel alpha. The Image wrapper
/// owns the generated palette and source-metadata policy, so keep those
/// results together across the lazy materialization boundary.
pub(crate) fn execute_prepared_cpu_arch_convert_output(
    prepared: &PreparedExecution,
    op: &PipelineOp,
    input: crate::compute::registry::support::OperationInputs<'_>,
) -> Result<crate::compute::pool_cpu::ops::color::convert::ops::PreparedConvertOutput, PilError> {
    let gpu_fallback = prepared.selected_backend == Backend::Gpu;
    let context = if gpu_fallback {
        ExecutionContext::for_routed_backend(Backend::Cpu)
    } else {
        prepared.context
    };
    let fallback_reason = if gpu_fallback {
        Some(format!(
            "Convert palette metadata is produced by the CPU wrapper route; used CPU {:?}",
            context.architecture()
        ))
    } else {
        prepared.fallback_reason.clone()
    };
    let mut output_data = None;
    let image = super::eager::execute_cpu_arch_operation(
        registry::variant_key(op),
        input.image,
        context,
        prepared.requested_backend,
        prepared.route_ns,
        prepared.validation_ns,
        fallback_reason,
        |context| {
            let operation = registry::prepare_cpu(op, input, context)?;
            let selection = *operation.selection();
            let output = operation.execute_convert_output()?;
            output_data = Some((output.palette, output.palette_alpha, output.wrapper_metadata));
            Ok((output.image, selection))
        },
    )?;
    let (palette, palette_alpha, wrapper_metadata) = output_data.ok_or_else(|| {
        PilError::InternalError("prepared palette conversion omitted its wrapper output".into())
    })?;
    Ok(
        crate::compute::pool_cpu::ops::color::convert::ops::PreparedConvertOutput {
            image,
            palette,
            palette_alpha,
            wrapper_metadata,
        },
    )
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
    let context = ExecutionContext::for_routed_backend(selected_backend);
    let route_ns = elapsed_ns(route_start);

    let validation_start = timed.then(pipeline_timestamp).flatten();
    if !support_checked {
        validate_backend_support(selected_backend, ops)?;
    }
    let validation_ns = elapsed_ns(validation_start);

    Ok(PreparedExecution {
        requested_backend,
        selected_backend,
        context,
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
    let executor_backend = if backend == Backend::Simd {
        Backend::Cpu
    } else {
        backend
    };
    let pool = pools()
        .iter()
        .find(|pool| pool.name() == executor_backend)
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
        let name = backend_label(backend);
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

/// Executes a previously routed and validated pipeline, recording the bounded
/// backend phase when telemetry is enabled. The logical source identity is a
/// typed `Mode`; CPU strategy selection always reads the exact mode from the
/// supplied `DynamicImage`.
pub(crate) fn execute_prepared(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    logical_mode: Option<Mode>,
) -> Result<DynamicImage, PilError> {
    execute_prepared_with_palette(prepared, ops, img, logical_mode, PaletteInput::default())
}

/// Executes a prepared pipeline with palette metadata resolved by its image
/// wrapper. The mode-bearing raster still owns the current mode identity.
pub(crate) fn execute_prepared_with_palette(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    logical_mode: Option<Mode>,
    palette: PaletteInput<'_>,
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
    execute_prepared_on_backend(prepared, ops, img, logical_mode, palette, timed, backend_start)
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
        crate::raster::Mode::ImageLuma8 => {
            DynamicImage::ImageLuma8(crate::raster::GrayImage::new(width, height))
        }
        crate::raster::Mode::ImageLumaA8 => {
            DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(width, height))
        }
        crate::raster::Mode::ImageLaPremultiplied8 => {
            DynamicImage::ImageLaPremultiplied8(crate::raster::GrayAlphaImage::new(width, height))
        }
        crate::raster::Mode::ImageRgb8 => {
            DynamicImage::ImageRgb8(crate::raster::RgbImage::new(width, height))
        }
        crate::raster::Mode::ImageRgba8 => {
            DynamicImage::ImageRgba8(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageRgbPremultiplied8 => {
            DynamicImage::ImageRgbPremultiplied8(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageRgbx8 => {
            DynamicImage::ImageRgbx8(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageCmyk8 => {
            DynamicImage::ImageCmyk8(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageYCbCr8 => {
            DynamicImage::ImageYCbCr8(crate::raster::RgbImage::new(width, height))
        }
        crate::raster::Mode::ImageLab8 => {
            DynamicImage::ImageLab8(crate::raster::RgbImage::new(width, height))
        }
        crate::raster::Mode::ImageHsv8 => {
            DynamicImage::ImageHsv8(crate::raster::RgbImage::new(width, height))
        }
        crate::raster::Mode::ImageI32 => {
            DynamicImage::ImageI32(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageF32 => {
            DynamicImage::ImageF32(crate::raster::RgbaImage::new(width, height))
        }
        crate::raster::Mode::ImageI16 => {
            DynamicImage::ImageI16(crate::raster::ImageBuffer::new(width, height))
        }
        crate::raster::Mode::ImageI16L => {
            DynamicImage::ImageI16L(crate::raster::ImageBuffer::new(width, height))
        }
        crate::raster::Mode::ImageI16B => {
            DynamicImage::ImageI16B(crate::raster::ImageBuffer::new(width, height))
        }
        crate::raster::Mode::ImageI16N => {
            DynamicImage::ImageI16N(crate::raster::ImageBuffer::new(width, height))
        }
        crate::raster::Mode::One | crate::raster::Mode::ImagePalette8 => return None,
        crate::raster::Mode::ImagePaletteAlpha8
        | crate::raster::Mode::ImageLumaA16
        | crate::raster::Mode::ImageRgb16
        | crate::raster::Mode::ImageRgba16
        | crate::raster::Mode::ImageRgb32F
        | crate::raster::Mode::ImageRgba32F => return None,
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

fn execute_prepared_on_backend(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &DynamicImage,
    logical_mode: Option<Mode>,
    palette: PaletteInput<'_>,
    timed: bool,
    backend_start: Option<Instant>,
) -> Result<DynamicImage, PilError> {
    let effective_backend = if prepared.selected_backend == Backend::Simd {
        Backend::Cpu
    } else {
        prepared.selected_backend
    };
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
    let execution = if prepared.requested_backend == Some(Backend::Gpu) {
        pool.execute_batch_strict(ops, img, logical_mode)
    } else {
        pool.execute_batch_with_context_and_palette(
            ops,
            img,
            logical_mode,
            prepared.context(),
            palette,
        )
    };
    match execution {
        Ok(result) => {
            if timed {
                record_prepared_backend_telemetry(
                    prepared,
                    ops,
                    backend_start,
                    effective_backend,
                    prepared.fallback_reason.clone(),
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

/// Shared-result counterpart to [`execute_prepared_with_palette`].
pub(crate) fn execute_prepared_shared_with_palette(
    prepared: &PreparedExecution,
    ops: &[PipelineOp],
    img: &Arc<DynamicImage>,
    logical_mode: Option<Mode>,
    palette: PaletteInput<'_>,
) -> Result<Arc<DynamicImage>, PilError> {
    if prepared.context.backend() == Backend::Cpu
        && img.mode() == crate::raster::Mode::ImageI32
        && ops.len() == 1
        && matches!(&ops[0], PipelineOp::Filter3x3 { .. })
        && matches!(img.as_ref(), DynamicImage::ImageI32(_))
    {
        let operation = registry::prepare_cpu(
            &ops[0],
            crate::compute::registry::support::OperationInputs { image: img.as_ref(), palette },
            &prepared.context,
        )?;
        let selection = *operation.selection();
        let timed = pipeline_telemetry_enabled();
        let backend_start = timed.then(pipeline_timestamp).flatten();
        let identity = operation.shared_identity_proof().unwrap_or(false);
        if identity {
            if timed {
                reset_pipeline_allocation_telemetry();
                reset_pipeline_operation_telemetry();
                let _ = take_pipeline_resource_telemetry();
                let _ = take_pipeline_backend_override();
                let _ = take_pipeline_dispatch_count();
                let _ = take_pipeline_resize_coeff_cache_stats();
                begin_pipeline_operation_telemetry(registry::variant_key(&ops[0]));
                crate::compute::record_pipeline_operation_selection(&selection);
                record_pipeline_operation_path(if selection.implementation.architecture
                    == crate::compute::Architecture::Default
                {
                    "cpu"
                } else {
                    "scalar-control"
                });
                if selection.implementation.architecture
                    != crate::compute::Architecture::Default
                {
                    let bytes = img.as_bytes().len();
                    record_pipeline_operation_vector_blocks((bytes / 16) as u64);
                    record_pipeline_operation_scalar_tail((bytes % 16) as u64);
                }
                finish_pipeline_operation_telemetry();
                let _ = take_pipeline_allocation_telemetry();
                record_pipeline_telemetry(PipelineTelemetry {
                    requested_backend: prepared.requested_backend,
                    actual_backend: Backend::Cpu,
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

        return super::eager::execute_cpu_arch_operation(
            registry::variant_key(&ops[0]),
            img.as_ref(),
            prepared.context,
            prepared.requested_backend,
            prepared.route_ns,
            prepared.validation_ns,
            prepared.fallback_reason.clone(),
            move |_context| operation.execute().map(|result| (result, selection)),
        )
        .map(Arc::new);
    }

    execute_prepared_with_palette(prepared, ops, img, logical_mode, palette).map(Arc::new)
}
