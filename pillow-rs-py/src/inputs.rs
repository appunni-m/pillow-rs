use super::{
    Bound, Py, PyAny, PyBytes, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyString,
    PyTuple, Python, chops_offset_from_python, map_error,
};
use pyo3::types::PyAnyMethods;

/// Classify a host text argument without invoking Python sequence callbacks.
pub(crate) fn font_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> pillow_rs::ImageFontTextInput {
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return pillow_rs::ImageFontTextInput::Bytes(bytes.as_bytes().to_vec());
    }
    if let Ok(text) = value.cast::<PyString>() {
        return pillow_rs::ImageFontTextInput::Text(text.to_string_lossy().into_owned());
    }
    pillow_rs::ImageFontTextInput::InvalidType
}

pub(crate) fn c_int_from_python(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    // Match PyArg_ParseTuple's C `i` conversion: PyLong_AsLong, then int bounds.
    // TODO(CPU parity): verify None, bool, custom __index__, C-long overflow,
    // and both signed C-int boundaries.
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
