use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

#[pyfunction]
fn enable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::enable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// Deactivate a compute backend. Returns true if it was active.
#[pyfunction]
fn disable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::disable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// List backend implementations compiled into this extension.
#[pyfunction]
fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// Report whether this extension was compiled with the opt-in Rayon feature.
#[pyfunction]
fn parallel_feature_enabled() -> bool {
    cfg!(feature = "parallel")
}

/// List currently active backends (priority order).
#[pyfunction]
fn active_backends() -> PyResult<Vec<String>> {
    Ok(pillow_rs::active_backends()
        .map_err(map_error)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[pyfunction]
fn backend_enabled(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::backend_enabled(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// Enable or disable bounded image-pipeline execution telemetry.
#[pyfunction]
fn set_pipeline_telemetry(enabled: bool) -> bool {
    pillow_rs::Backend::set_pipeline_telemetry_enabled(enabled)
}

/// Take the most recent completed image-pipeline telemetry sample for this
/// thread, or return ``None`` when no sample is available.
fn pipeline_operation_telemetry_to_py(
    py: Python<'_>,
    samples: Vec<pillow_rs::PipelineOperationTelemetry>,
) -> PyResult<Py<PyAny>> {
    let values = PyList::empty(py);
    for sample in samples {
        let value = PyDict::new(py);
        value.set_item("operation", sample.operation)?;
        value.set_item("path", sample.path)?;
        value.set_item("vector_block_count", sample.vector_block_count)?;
        value.set_item("scalar_tail_count", sample.scalar_tail_count)?;
        value.set_item("mode_conversion_count", sample.mode_conversion_count)?;
        value.set_item("handoff_count", sample.handoff_count)?;
        values.append(value)?;
    }
    Ok(values.into())
}

#[pyfunction]
fn take_pipeline_telemetry(py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
    let operation_telemetry = pillow_rs::Backend::take_pipeline_operation_telemetry();
    let phase_timings = pillow_rs::Backend::take_pipeline_phase_timings();
    let Some((
        requested_backend,
        actual_backend,
        operation_count,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason,
        resource,
        resize_coeff_cache_hits,
        resize_coeff_cache_misses,
    )) = pillow_rs::Backend::take_pipeline_telemetry()
    else {
        if operation_telemetry.is_empty() && phase_timings == (None, None, None) {
            return Ok(None);
        }
        let result = PyDict::new(py);
        if !operation_telemetry.is_empty() {
            result.set_item(
                "operation_telemetry",
                pipeline_operation_telemetry_to_py(py, operation_telemetry)?,
            )?;
        }
        telemetry::set_pipeline_phase_diagnostics(&result, phase_timings)?;
        return Ok(Some(result.into()));
    };

    let result = PyDict::new(py);
    result.set_item(
        "requested_backend",
        requested_backend.map(|backend| format!("{:?}", backend).to_lowercase()),
    )?;
    result.set_item(
        "actual_backend",
        format!("{:?}", actual_backend).to_lowercase(),
    )?;
    result.set_item("operation_count", operation_count)?;
    result.set_item("route_ns", route_ns)?;
    result.set_item("validation_ns", validation_ns)?;
    result.set_item("backend_ns", backend_ns)?;
    result.set_item("dispatch_count", dispatch_count)?;
    result.set_item("fallback_reason", fallback_reason)?;
    result.set_item("resize_coeff_cache_hits", resize_coeff_cache_hits)?;
    result.set_item("resize_coeff_cache_misses", resize_coeff_cache_misses)?;
    result.set_item(
        "operation_telemetry",
        pipeline_operation_telemetry_to_py(py, operation_telemetry)?,
    )?;
    telemetry::set_pipeline_phase_diagnostics(&result, phase_timings)?;
    if let Some(resource) = resource {
        let resource_dict = PyDict::new(py);
        resource_dict.set_item("upload_bytes", resource.upload_bytes)?;
        resource_dict.set_item("readback_bytes", resource.readback_bytes)?;
        resource_dict.set_item("auxiliary_bytes", resource.auxiliary_bytes)?;
        resource_dict.set_item("parameter_bytes", resource.parameter_bytes)?;
        resource_dict.set_item("retained_cache_bytes", resource.retained_cache_bytes)?;
        resource_dict.set_item("full_frame_copy_count", resource.full_frame_copy_count)?;
        resource_dict.set_item("mode_conversion_count", resource.mode_conversion_count)?;
        resource_dict.set_item("host_buffer_count", resource.host_buffer_count)?;
        resource_dict.set_item("host_buffer_bytes", resource.host_buffer_bytes)?;
        resource_dict.set_item("peak_live_host_bytes", resource.peak_live_host_bytes)?;
        resource_dict.set_item("fused_operation_count", resource.fused_operation_count)?;
        resource_dict.set_item("host_allocation_count", resource.host_allocation_count)?;
        resource_dict.set_item("host_allocated_bytes", resource.host_allocated_bytes)?;
        result.set_item("resource", resource_dict)?;
    } else {
        result.set_item("resource", py.None())?;
    }
    Ok(Some(result.into()))
}

/// Enable or disable the bounded GPU shader-dispatch coverage collector.
#[pyfunction]
fn set_gpu_shader_coverage(enabled: bool) -> bool {
    pillow_rs::Backend::set_gpu_shader_coverage_enabled(enabled)
}

/// Take the embedded WGSL shader variants that actually dispatched in this
/// process. This reports shader execution coverage; source line and branch
/// coverage require a separate WGSL instrumentation pass.
#[pyfunction]
fn take_gpu_shader_coverage(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let result = PyList::empty(py);
    for record in pillow_rs::Backend::take_gpu_shader_coverage() {
        let item = PyDict::new(py);
        item.set_item("variant_name", record.variant_name)?;
        item.set_item("shader_file", record.shader_file)?;
        item.set_item("dispatches", record.dispatches)?;
        item.set_item("workgroups", record.workgroups)?;
        result.append(item)?;
    }
    Ok(result.into())
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(enable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(disable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(available_backends, m)?)?;
    m.add_function(wrap_pyfunction!(parallel_feature_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(active_backends, m)?)?;
    m.add_function(wrap_pyfunction!(backend_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(set_pipeline_telemetry, m)?)?;
    m.add_function(wrap_pyfunction!(take_pipeline_telemetry, m)?)?;
    m.add_function(wrap_pyfunction!(set_gpu_shader_coverage, m)?)?;
    m.add_function(wrap_pyfunction!(take_gpu_shader_coverage, m)?)?;
    Ok(())
}
