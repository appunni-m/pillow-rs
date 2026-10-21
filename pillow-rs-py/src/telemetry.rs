//! Python object materialization and optional diagnostic telemetry conversion.

use pyo3::prelude::{Bound, Py, PyResult, Python};
use pyo3::types::{PyBytes, PyDict, PyDictMethods};

pub(super) fn materialize_python_bytes<'py>(py: Python<'py>, bytes: &[u8]) -> Py<PyBytes> {
    let started = pillow_rs::Backend::pipeline_telemetry_enabled().then(std::time::Instant::now);
    let output = PyBytes::new(py, bytes);
    if let Some(started) = started {
        let elapsed = started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        pillow_rs::Backend::record_python_bytes_copy_timing(elapsed);
    }
    output.unbind()
}

pub(super) fn set_pipeline_phase_diagnostics(
    result: &Bound<'_, PyDict>,
    timings: (Option<u64>, Option<u64>, Option<u64>),
) -> PyResult<()> {
    let diagnostics = PyDict::new(result.py());
    if let Some(nanoseconds) = timings.0 {
        diagnostics.set_item("transpose_output_allocation_zero_fill_ns", nanoseconds)?;
    }
    if let Some(nanoseconds) = timings.1 {
        diagnostics.set_item("transpose_avx2_executor_ns", nanoseconds)?;
    }
    if let Some(nanoseconds) = timings.2 {
        diagnostics.set_item("python_bytes_copy_ns", nanoseconds)?;
    }
    if !diagnostics.is_empty() {
        result.set_item("phase_diagnostics_ns", diagnostics)?;
    }
    Ok(())
}
