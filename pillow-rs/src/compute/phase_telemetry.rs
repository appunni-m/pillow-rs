//! Opt-in phase-level timings kept separate from scored end-to-end latency.

use super::{Backend, pipeline_telemetry_enabled};
use std::cell::RefCell;

#[derive(Debug, Default)]
struct PipelinePhaseTimings {
    python_bytes_copy_ns: Option<u64>,
}

#[derive(Clone, Copy)]
enum PipelinePhase {
    PythonBytesCopy,
}

thread_local! {
    static LAST_PIPELINE_PHASE_TIMINGS: RefCell<PipelinePhaseTimings> = const {
        RefCell::new(PipelinePhaseTimings {
            python_bytes_copy_ns: None,
        })
    };
}

pub(super) fn clear() {
    LAST_PIPELINE_PHASE_TIMINGS.with(|last| {
        *last.borrow_mut() = PipelinePhaseTimings::default();
    });
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
            PipelinePhase::PythonBytesCopy => &mut last.python_bytes_copy_ns,
        };
        *field = Some(field.unwrap_or(0).saturating_add(nanoseconds));
    });
}

fn take_pipeline_phase_timings() -> (Option<u64>, Option<u64>, Option<u64>) {
    LAST_PIPELINE_PHASE_TIMINGS.with(|last| {
        let timings = std::mem::take(&mut *last.borrow_mut());
        (None, None, timings.python_bytes_copy_ns)
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
