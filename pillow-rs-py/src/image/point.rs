use crate::*;
use pyo3::{pyclass, pymethods};

/// Temporary affine transform object passed to scalar Image.point callables.
///
/// Pillow uses this object to recognize expressions such as ``x + 1`` and
/// ``x * 0.5`` without enumerating a byte lookup table.  It is an adapter
/// object only; the resulting scale and offset are applied by the Rust core.
#[pyclass(name = "ImagePointTransform", from_py_object)]
#[derive(Clone, Copy)]
pub struct PyPointTransform {
    scale: f64,
    offset: f64,
}

fn point_transform_operand(other: &Bound<'_, PyAny>) -> PyResult<Result<PyPointTransform, f64>> {
    if let Ok(transform) = other.extract::<PyRef<'_, PyPointTransform>>() {
        return Ok(Ok(*transform));
    }
    if let Ok(value) = other.extract::<f64>() {
        return Ok(Err(value));
    }
    Err(PyTypeError::new_err(
        "point transform operands must be numeric",
    ))
}

/// Probes a callable with Pillow's affine point-transform sentinel.
///
/// The callback is intentionally invoked while the GIL is held.  Only the
/// resulting scalar parameters cross into the Rust core, so the core never
/// needs to retain or call a Python object.
pub(super) fn point_transform_from_python(
    input: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<(f64, f64)> {
    let seed = Py::new(
        py,
        PyPointTransform {
            scale: 1.0,
            offset: 0.0,
        },
    )?;
    let result = input.call1((seed,))?;
    if let Ok(transform) = result.extract::<PyRef<'_, PyPointTransform>>() {
        return Ok((transform.scale, transform.offset));
    }
    Ok((0.0, result.extract::<f64>()?))
}

#[pymethods]
impl PyPointTransform {
    #[new]
    fn new(scale: f64, offset: f64) -> Self {
        Self { scale, offset }
    }

    fn __neg__(&self) -> Self {
        Self {
            scale: -self.scale,
            offset: -self.offset,
        }
    }

    fn __add__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        match point_transform_operand(other)? {
            Ok(other) => Ok(Self {
                scale: self.scale + other.scale,
                offset: self.offset + other.offset,
            }),
            Err(value) => Ok(Self {
                scale: self.scale,
                offset: self.offset + value,
            }),
        }
    }

    fn __radd__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__add__(other)
    }

    fn __sub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        match point_transform_operand(other)? {
            Ok(other) => Ok(Self {
                scale: self.scale - other.scale,
                offset: self.offset - other.offset,
            }),
            Err(value) => Ok(Self {
                scale: self.scale,
                offset: self.offset - value,
            }),
        }
    }

    fn __rsub__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        match point_transform_operand(other)? {
            Ok(other) => Ok(Self {
                scale: other.scale - self.scale,
                offset: other.offset - self.offset,
            }),
            Err(value) => Ok(Self {
                scale: -self.scale,
                offset: value - self.offset,
            }),
        }
    }

    fn __mul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let value = match point_transform_operand(other)? {
            Ok(_) => {
                return Err(PyTypeError::new_err("cannot multiply two point transforms"));
            }
            Err(value) => value,
        };
        Ok(Self {
            scale: self.scale * value,
            offset: self.offset * value,
        })
    }

    fn __rmul__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__mul__(other)
    }

    fn __truediv__(&self, other: &Bound<'_, PyAny>) -> PyResult<Self> {
        let value = match point_transform_operand(other)? {
            Ok(_) => {
                return Err(PyTypeError::new_err("cannot divide by a point transform"));
            }
            Err(value) => value,
        };
        if value == 0.0 {
            return Err(pyo3::exceptions::PyZeroDivisionError::new_err(
                "float division by zero",
            ));
        }
        Ok(Self {
            scale: self.scale / value,
            offset: self.offset / value,
        })
    }
}
