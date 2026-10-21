//! Python tuple conversion for `Image.getcolors` results.

use pyo3::conversion::{IntoPyObject, IntoPyObjectExt};
use pyo3::prelude::{Bound, PyErr, PyResult, Python};
use pyo3::types::PyTuple;

pub(super) struct PyGetColorsEntry {
    pub(super) count: u32,
    pub(super) color: pillow_rs::FormattedPixelValue,
}

impl<'py> IntoPyObject<'py> for PyGetColorsEntry {
    type Target = PyTuple;
    type Output = Bound<'py, PyTuple>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        let color = match self.color {
            pillow_rs::FormattedPixelValue::Scalar(value) => value.into_py_any(py)?,
            pillow_rs::FormattedPixelValue::Integer(value) => value.into_py_any(py)?,
            pillow_rs::FormattedPixelValue::Float(value) => value.into_py_any(py)?,
            pillow_rs::FormattedPixelValue::Components(values) => {
                PyTuple::new(py, values)?.into_py_any(py)?
            }
            pillow_rs::FormattedPixelValue::InlineComponents { bytes, bands } => {
                PyTuple::new(py, bytes.into_iter().take(usize::from(bands)))?.into_py_any(py)?
            }
        };
        PyTuple::new(py, [self.count.into_py_any(py)?, color])
    }
}
