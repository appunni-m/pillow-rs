use super::{
    Bound, Py, PyAny, PyBytes, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyString,
    PyTuple, PyTypeError, PyValueError, Python, chops_offset_from_python, map_error,
};
use pyo3::types::PyAnyMethods;

pub(crate) fn c_int_from_python(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    // Match PyArg_ParseTuple's C `i` conversion: PyLong_AsLong, then int bounds.
    // TODO(CPU parity): cover omitted versus explicit None, bool and __index__
    // objects, C-long overflow, and both signed C-int boundaries.
    let value_as_long = value.extract::<std::os::raw::c_long>()?;
    i32::try_from(value_as_long).map_err(|_| {
        PyOverflowError::new_err(if value_as_long < 0 {
            "signed integer is less than minimum"
        } else {
            "signed integer is greater than maximum"
        })
    })
}

mod filters;
pub(crate) use filters::*;
mod analysis;
pub(crate) use analysis::*;
mod paths;
pub(crate) use paths::*;
mod ops;
pub(crate) use ops::*;
mod images;
pub(crate) use images::*;
mod masks;
pub(crate) use masks::*;
mod pixels;
pub(crate) use pixels::*;
