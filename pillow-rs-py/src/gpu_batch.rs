//! Thin host adapters for core-owned GPU scheduling and resource leases.
use super::{PyImage, map_error};
use pillow_rs::{BatchError, BatchKey, BatchOutput, GpuBatchConfig, GpuBatchExecutor, GpuImage};
use pyo3::types::{PyAnyMethods, PyModule, PyModuleMethods};
use pyo3::{Bound, IntoPyObject, Py, PyAny, PyErr, PyResult, Python, pyclass, pymethods};
use std::collections::HashMap;

pyo3::create_exception!(_core, GpuBatchError, pyo3::exceptions::PyRuntimeError);

fn batch_error(error: BatchError) -> PyErr {
    let exception = GpuBatchError::new_err(error.message);
    Python::attach(|py| {
        let value = exception.value(py);
        let _ = value.setattr("job_id", error.job_id);
        let _ = value.setattr("stage", error.stage);
        let _ = value.setattr("kind", format!("{:?}", error.kind));
        let key = match error.input_key {
            Some(BatchKey::Integer(key)) => match key.into_pyobject(py) {
                Ok(value) => value.into_any().unbind(),
                Err(never) => match never {},
            },
            Some(BatchKey::Text(key)) => match key.into_pyobject(py) {
                Ok(value) => value.into_any().unbind(),
                Err(never) => match never {},
            },
            None => py.None(),
        };
        let _ = value.setattr("input_key", key);
    });
    exception
}

#[pyclass(name = "GpuImage")]
pub(super) struct PyGpuImage {
    inner: GpuImage,
}
#[pymethods]
impl PyGpuImage {
    #[getter]
    fn mode(&self) -> &str {
        self.inner.mode()
    }
    #[getter]
    fn size(&self) -> (u32, u32) {
        self.inner.size()
    }
    fn download(&self, py: Python<'_>) -> PyResult<PyImage> {
        py.detach(|| self.inner.download())
            .map(|inner| PyImage { inner })
            .map_err(map_error)
    }
}

#[pyclass(name = "GpuBatchExecutor")]
struct PyGpuBatchExecutor {
    inner: GpuBatchExecutor,
}
#[pymethods]
impl PyGpuBatchExecutor {
    #[new]
    #[pyo3(signature = (*, queue=true, gpu_bytes=536870912, host_bytes=268435456, max_jobs=64, max_in_flight=2))]
    fn new(
        py: Python<'_>,
        queue: bool,
        gpu_bytes: u64,
        host_bytes: u64,
        max_jobs: usize,
        max_in_flight: usize,
    ) -> PyResult<Self> {
        py.detach(|| {
            GpuBatchExecutor::new(GpuBatchConfig {
                queue,
                gpu_bytes,
                host_bytes,
                max_jobs,
                max_in_flight,
            })
        })
        .map(|inner| Self { inner })
        .map_err(batch_error)
    }
    #[pyo3(signature = (image, key=None, metadata_bytes=0))]
    fn submit(
        &mut self,
        py: Python<'_>,
        image: &PyImage,
        key: Option<&Bound<'_, PyAny>>,
        metadata_bytes: u64,
    ) -> PyResult<u64> {
        let key = match key {
            None => None,
            Some(key) if key.is_none() => None,
            Some(key) if key.is_instance_of::<pyo3::types::PyBool>() => {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "batch input keys must be strings or unsigned 64-bit integers",
                ));
            }
            Some(key) => Some(if let Ok(text) = key.extract::<String>() {
                BatchKey::Text(text)
            } else {
                BatchKey::Integer(key.extract::<u64>().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(
                        "batch input keys must be strings or unsigned 64-bit integers",
                    )
                })?)
            }),
        };
        py.detach(|| {
            self.inner
                .submit_with_metadata(&image.inner, key, metadata_bytes)
        })
        .map_err(batch_error)
    }
    fn set_resident_output(&mut self, resident: bool) -> PyResult<()> {
        self.inner
            .set_resident_output(resident)
            .map_err(batch_error)
    }
    fn next_result(&mut self, py: Python<'_>) -> PyResult<Option<(u64, Py<PyAny>, Py<PyAny>)>> {
        let Some(result) = py
            .detach(|| self.inner.next_result())
            .map_err(batch_error)?
        else {
            return Ok(None);
        };
        let key = match result.input_key {
            Some(BatchKey::Integer(key)) => key.into_pyobject(py)?.into_any().unbind(),
            Some(BatchKey::Text(key)) => key.into_pyobject(py)?.into_any().unbind(),
            None => py.None(),
        };
        let output = match result.image {
            BatchOutput::Cpu(inner) => Py::new(py, PyImage { inner: *inner })?.into_any(),
            BatchOutput::Gpu(inner) => Py::new(py, PyGpuImage { inner })?.into_any(),
        };
        Ok(Some((result.job_id, key, output)))
    }
    fn stats(&self) -> HashMap<&'static str, u64> {
        let stats = self.inner.stats();
        HashMap::from([
            ("gpu_bytes", stats.gpu_bytes),
            ("host_bytes", stats.host_bytes),
            ("live_jobs", stats.live_jobs as u64),
            ("in_flight", stats.in_flight as u64),
            ("submitted_jobs", stats.submitted_jobs),
            ("submissions", stats.submissions),
            ("dispatches", stats.dispatches),
            ("upload_bytes", stats.upload_bytes),
            ("readback_bytes", stats.readback_bytes),
            ("largest_submission", stats.largest_submission as u64),
        ])
    }
    fn close(&mut self, py: Python<'_>) {
        py.detach(|| self.inner.close());
    }
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("GpuBatchError", m.py().get_type::<GpuBatchError>())?;
    m.add_class::<PyGpuBatchExecutor>()?;
    m.add_class::<PyGpuImage>()?;
    Ok(())
}
