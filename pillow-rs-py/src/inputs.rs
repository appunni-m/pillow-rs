use super::{
    Bound, Py, PyAny, PyBytes, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult,
    PyString, PyTuple, PyTypeError, PyValueError, Python, chops_offset_from_python, map_error,
};
use pyo3::types::PyAnyMethods;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyStringMethods;

/// Classify Python text while preserving every Unicode codepoint accepted by
/// Python, including lone surrogate codepoints that Rust `String` cannot hold.
pub(crate) fn font_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontTextInput> {
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(pillow_rs::ImageFontTextInput::Bytes(
            bytes.as_bytes().to_vec(),
        ));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return match text.to_cow() {
            Ok(text) => Ok(pillow_rs::ImageFontTextInput::Text(text.into_owned())),
            Err(error)
                if error.is_instance_of::<pyo3::exceptions::PyUnicodeEncodeError>(value.py()) =>
            {
                let encode = value.py().get_type::<PyString>().getattr("encode")?;
                let encoded = encode
                    .call1((value, "utf-32-le", "surrogatepass"))?
                    .extract::<Vec<u8>>()?;
                let codepoints = encoded
                    .chunks_exact(4)
                    .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                    .collect();
                Ok(pillow_rs::ImageFontTextInput::TextCodepoints(codepoints))
            }
            Err(error) => Err(error),
        };
    }
    Ok(pillow_rs::ImageFontTextInput::InvalidType)
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
mod float_buffer;
pub(crate) use float_buffer::PillowFloatPointBuffer;
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
