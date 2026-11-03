use super::{
    Architecture, Backend, Instant, PipelineResourceTelemetry, PipelineTelemetry,
    begin_pipeline_operation_telemetry, elapsed_ns, finish_pipeline_operation_telemetry,
    pipeline_telemetry_enabled, record_pipeline_operation_path, record_pipeline_resource_telemetry,
    record_pipeline_operation_selection, record_pipeline_telemetry,
    reset_pipeline_allocation_telemetry,
    reset_pipeline_operation_telemetry, take_pipeline_backend_override,
    take_pipeline_dispatch_count, take_pipeline_resize_coeff_cache_stats,
    take_pipeline_resource_telemetry, take_pipeline_telemetry,
};
pub(crate) struct EagerCpuOperationTelemetry {
    timed: bool,
    backend_start: Option<Instant>,
    requested_backend: Option<Backend>,
    fallback_reason: Option<String>,
}

/// Begin an execution receipt for an eager operation whose data path is
/// implemented on the host. GPU and SIMD requests begin with a provisional
/// fallback receipt; a prepared strategy refines SIMD into its actual selected
/// architecture, while GPU-to-CPU routing remains a backend fallback.
pub(crate) fn begin_eager_cpu_operation_telemetry(
    operation: &'static str,
    requested_backend: Option<Backend>,
) -> EagerCpuOperationTelemetry {
    begin_eager_cpu_operation_telemetry_with_reason(operation, requested_backend, None)
}

pub(crate) fn begin_eager_cpu_operation_telemetry_with_reason(
    operation: &'static str,
    requested_backend: Option<Backend>,
    fallback_detail: Option<&str>,
) -> EagerCpuOperationTelemetry {
    let timed = pipeline_telemetry_enabled();
    if !timed {
        return EagerCpuOperationTelemetry {
            timed,
            backend_start: None,
            requested_backend,
            fallback_reason: None,
        };
    }

    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    begin_pipeline_operation_telemetry(operation);

    EagerCpuOperationTelemetry {
        timed,
        backend_start: pipeline_timestamp(),
        requested_backend,
        fallback_reason: requested_backend
            .filter(|backend| *backend != Backend::Cpu)
            .map(|backend| match fallback_detail {
                Some(detail) => format!(
                    "{operation} ran on the host CPU while {backend:?} was requested: {detail}"
                ),
                None => format!("{operation} ran on the host CPU while {backend:?} was requested"),
            }),
    }
}

/// Record a host-owned data view that the binding copies into its final
/// language-level bytes object after this core method returns.
pub(crate) fn record_eager_cpu_operation_receipt(
    operation: &'static str,
    requested_backend: Option<Backend>,
    output_bytes: usize,
) {
    let telemetry = begin_eager_cpu_operation_telemetry(operation, requested_backend);
    finish_eager_cpu_operation_telemetry(telemetry, output_bytes);
}

/// Record a host-owned operation with a specific reason that prevents use of
/// the requested accelerated backend.
pub(crate) fn record_eager_cpu_operation_receipt_with_reason(
    operation: &'static str,
    requested_backend: Option<Backend>,
    output_bytes: usize,
    fallback_detail: &'static str,
) {
    let telemetry = begin_eager_cpu_operation_telemetry_with_reason(
        operation,
        requested_backend,
        Some(fallback_detail),
    );
    finish_eager_cpu_operation_telemetry(telemetry, output_bytes);
}

/// Record a host-owned operation with its reason and prepared strategy.
pub(crate) fn record_eager_cpu_operation_receipt_with_selection(
    operation: &'static str,
    requested_backend: Option<Backend>,
    output_bytes: usize,
    fallback_detail: &'static str,
    selection: crate::compute::registry::support::StrategySelection,
) {
    let telemetry = begin_eager_cpu_operation_telemetry_with_reason(
        operation,
        requested_backend,
        Some(fallback_detail),
    );
    finish_eager_cpu_operation_telemetry_with_selection(
        telemetry,
        output_bytes,
        operation,
        selection,
    );
}

/// Finish a host terminal-read receipt after the caller has filled its final
/// output buffer. The timing report still covers the complete Python call and
/// result materialization; this receipt identifies the executor and accounts
/// for the caller-owned output buffer without claiming a core allocation.
pub(crate) fn finish_eager_cpu_operation_telemetry(
    telemetry: EagerCpuOperationTelemetry,
    output_bytes: usize,
) {
    if !telemetry.timed {
        return;
    }

    record_pipeline_operation_path("cpu");
    finish_pipeline_operation_telemetry();
    let output_bytes = output_bytes as u64;
    record_pipeline_resource_telemetry(PipelineResourceTelemetry {
        host_buffer_count: u64::from(output_bytes != 0),
        host_buffer_bytes: output_bytes,
        peak_live_host_bytes: output_bytes,
        ..PipelineResourceTelemetry::default()
    });
    record_pipeline_telemetry(PipelineTelemetry {
        requested_backend: telemetry.requested_backend,
        actual_backend: Backend::Cpu,
        operation_count: 1,
        route_ns: 0,
        validation_ns: 0,
        backend_ns: elapsed_ns(telemetry.backend_start),
        dispatch_count: None,
        fallback_reason: telemetry.fallback_reason,
        resource: take_pipeline_resource_telemetry(),
        resize_coeff_cache_hits: 0,
        resize_coeff_cache_misses: 0,
    });
}

/// Finish a host operation receipt with its prepared architecture selection.
pub(crate) fn finish_eager_cpu_operation_telemetry_with_selection(
    mut telemetry: EagerCpuOperationTelemetry,
    output_bytes: usize,
    operation: &'static str,
    selection: crate::compute::registry::support::StrategySelection,
) {
    if !telemetry.timed {
        return;
    }

    record_pipeline_operation_selection(&selection);
    let selected_fallback = selection
        .fallback
        .map(|reason| {
            format!(
                "{operation} selected {:?} after strategy fallback: {reason:?}",
                selection.implementation.architecture,
            )
        })
        .or_else(|| {
            (telemetry.requested_backend == Some(Backend::Simd)
                && selection.implementation.architecture == Architecture::Default)
            .then(|| {
                format!(
                    "{operation} selected CPU Default while SIMD architecture was requested"
                )
            })
        });
    if telemetry.requested_backend == Some(Backend::Simd) {
        telemetry.fallback_reason = None;
    }
    if let Some(selected_fallback) = selected_fallback {
        telemetry.fallback_reason = Some(match telemetry.fallback_reason.take() {
            Some(existing) => format!("{existing}; {selected_fallback}"),
            None => selected_fallback,
        });
    }
    finish_eager_cpu_operation_telemetry(telemetry, output_bytes);
}

/// Discard a failed host terminal read without leaving a live operation frame
/// or a partial execution receipt for the next measured operation.
pub(crate) fn cancel_eager_cpu_operation_telemetry(telemetry: EagerCpuOperationTelemetry) {
    if !telemetry.timed {
        return;
    }
    reset_pipeline_allocation_telemetry();
    reset_pipeline_operation_telemetry();
    let _ = take_pipeline_resource_telemetry();
    let _ = take_pipeline_backend_override();
    let _ = take_pipeline_dispatch_count();
    let _ = take_pipeline_resize_coeff_cache_stats();
    let _ = take_pipeline_telemetry();
}

// ``std::time::Instant`` is not implemented by the bare wasm32 target used
// by the browser package.  Backend identity and dispatch receipts remain
// useful there, so keep telemetry available while reporting zero for its
// host-clock fields instead of panicking during lazy materialization.
pub(in crate::compute) fn pipeline_timestamp() -> Option<Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(Instant::now())
    }
}
