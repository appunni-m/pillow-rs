//! Opt-in phase-level timings kept separate from scored end-to-end latency.

use super::{Backend, elapsed_ns, pipeline_telemetry_enabled, pipeline_timestamp};
use std::cell::RefCell;
use std::time::Instant;

#[derive(Debug, Default)]
struct PipelinePhaseTimings {
    transpose_output_allocation_zero_fill_ns: Option<u64>,
    transpose_avx2_executor_ns: Option<u64>,
    python_bytes_copy_ns: Option<u64>,
}

#[derive(Clone, Copy)]
enum PipelinePhase {
    TransposeOutputAllocationZeroFill,
    #[cfg(target_arch = "x86_64")]
    TransposeAvx2Executor,
    PythonBytesCopy,
}

thread_local! {
    static LAST_PIPELINE_PHASE_TIMINGS: RefCell<PipelinePhaseTimings> = const { RefCell::new(
        PipelinePhaseTimings {
            transpose_output_allocation_zero_fill_ns: None,
            transpose_avx2_executor_ns: None,
            python_bytes_copy_ns: None,
        }
    ) };
}

pub(super) fn clear() {
    LAST_PIPELINE_PHASE_TIMINGS.with(|last| {
        *last.borrow_mut() = PipelinePhaseTimings::default();
    });
}

/// Begin a phase timer only for an explicitly enabled diagnostic receipt.
pub(crate) fn pipeline_phase_timestamp() -> Option<Instant> {
    pipeline_telemetry_enabled()
        .then(pipeline_timestamp)
        .flatten()
}

/// Record the checked output allocation and zero-fill phase for native SIMD
/// Transpose. This is diagnostic evidence, not a replacement for end-to-end
/// call timing.
pub(crate) fn record_transpose_output_allocation_timing(start: Option<Instant>) {
    if let Some(start) = start {
        add_pipeline_phase_timing(
            PipelinePhase::TransposeOutputAllocationZeroFill,
            elapsed_ns(Some(start)),
        );
    }
}

/// Record a successfully admitted AVX2 Transpose executor call.
#[cfg(target_arch = "x86_64")]
pub(crate) fn record_transpose_avx2_executor_timing(start: Option<Instant>) {
    if let Some(start) = start {
        add_pipeline_phase_timing(
            PipelinePhase::TransposeAvx2Executor,
            elapsed_ns(Some(start)),
        );
    }
}

fn record_python_bytes_copy_timing(nanoseconds: u64) {
    if pipeline_telemetry_enabled() {
        add_pipeline_phase_timing(PipelinePhase::PythonBytesCopy, nanoseconds);
    }
}

fn add_pipeline_phase_timing(phase: PipelinePhase, nanoseconds: u64) {
    if !pipeline_telemetry_enabled() {
        return;
    }
    LAST_PIPELINE_PHASE_TIMINGS.with(|last| {
        let mut last = last.borrow_mut();
        let field = match phase {
            PipelinePhase::TransposeOutputAllocationZeroFill => {
                &mut last.transpose_output_allocation_zero_fill_ns
            }
            #[cfg(target_arch = "x86_64")]
            PipelinePhase::TransposeAvx2Executor => &mut last.transpose_avx2_executor_ns,
            PipelinePhase::PythonBytesCopy => &mut last.python_bytes_copy_ns,
        };
        *field = Some(field.unwrap_or(0).saturating_add(nanoseconds));
    });
}

fn take_pipeline_phase_timings() -> (Option<u64>, Option<u64>, Option<u64>) {
    LAST_PIPELINE_PHASE_TIMINGS.with(|last| {
        let timings = std::mem::take(&mut *last.borrow_mut());
        (
            timings.transpose_output_allocation_zero_fill_ns,
            timings.transpose_avx2_executor_ns,
            timings.python_bytes_copy_ns,
        )
    })
}

impl Backend {
    /// Takes opt-in diagnostic timings for the most recent managed pipeline.
    ///
    /// These timings are collected only while pipeline telemetry is enabled
    /// and are separate from the scored end-to-end latency phases.
    pub fn take_pipeline_phase_timings() -> (Option<u64>, Option<u64>, Option<u64>) {
        take_pipeline_phase_timings()
    }

    /// Records the binding's final Python-bytes copy while diagnostic
    /// pipeline telemetry is enabled.
    pub fn record_python_bytes_copy_timing(nanoseconds: u64) {
        record_python_bytes_copy_timing(nanoseconds);
    }
}
