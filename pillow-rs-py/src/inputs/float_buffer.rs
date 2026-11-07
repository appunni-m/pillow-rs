use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyModule, PyResult};
use pyo3::types::PyAnyMethods;

/// Pillow's path adapter treats a simple buffer as native float32 x/y pairs.
pub(crate) struct PillowFloatPointBuffer {
    bytes: Vec<u8>,
    coordinate_count: usize,
}

impl PillowFloatPointBuffer {
    /// Copy a contiguous simple buffer, returning `None` when the value does
    /// not expose one so callers can continue through Pillow's sequence path.
    pub(crate) fn from_python(value: &Bound<'_, PyAny>) -> Option<Self> {
        let py = value.py();
        let memoryview = PyModule::import(py, "builtins")
            .ok()?
            .getattr("memoryview")
            .ok()?
            .call1((value,))
            .ok()?;
        if !memoryview
            .getattr("c_contiguous")
            .ok()?
            .extract::<bool>()
            .ok()?
        {
            return None;
        }
        let bytes = memoryview
            .call_method0("tobytes")
            .ok()?
            .extract::<Vec<u8>>()
            .ok()?;
        let point_count = bytes.len() / (2 * std::mem::size_of::<f32>());
        Some(Self {
            bytes,
            coordinate_count: point_count.saturating_mul(2),
        })
    }

    pub(crate) fn coordinate_count(&self) -> usize {
        self.coordinate_count
    }

    pub(crate) fn coordinate(&self, index: usize) -> PyResult<f64> {
        if index >= self.coordinate_count {
            return Err(PyValueError::new_err("incorrect coordinate type"));
        }
        let byte_offset = index * std::mem::size_of::<f32>();
        let bytes = self
            .bytes
            .get(byte_offset..byte_offset + std::mem::size_of::<f32>())
            .ok_or_else(|| PyValueError::new_err("incorrect coordinate type"))?;
        let value = f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        Ok(f64::from(value))
    }
}
