use super::{Bound, PyAny, PyResult, PyTuple};
use pyo3::exceptions::PyTypeError;
use pyo3::types::{PyAnyMethods, PyTupleMethods, PyTypeMethods};

/// Parses Pillow's optional histogram range only for its native I/F modes.
// TODO(pillow-rs#3ptb): parity-check tuple shape, __index__/float coercion,
// overflow text, mask precedence, and omitted/explicit extrema cases.
pub(crate) fn histogram_extrema_from_python(
    extrema: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
    masked: bool,
) -> PyResult<Option<pillow_rs::HistogramExtrema>> {
    let Some(extrema) = extrema.filter(|_| !masked) else {
        return Ok(None);
    };
    match mode {
        pillow_rs::Mode::ImageI32 => {
            let (minimum, maximum) = integer_extrema(extrema)?;
            Ok(Some(pillow_rs::HistogramExtrema::ImageI32 { minimum, maximum }))
        }
        pillow_rs::Mode::ImageF32 => {
            let (minimum, maximum) = float_extrema(extrema)?;
            Ok(Some(pillow_rs::HistogramExtrema::ImageF32 {
                minimum: minimum as f32,
                maximum: maximum as f32,
            }))
        }
        _ => Ok(None),
    }
}

fn extrema_tuple<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let tuple = match value.cast::<PyTuple>() {
        Ok(tuple) => tuple,
        Err(_) => {
            let type_name = value.get_type().name()?;
            return Err(PyTypeError::new_err(format!(
                "argument 1 must be tuple, not {type_name}"
            )));
        }
    };
    if tuple.len() != 2 {
        return Err(PyTypeError::new_err(format!(
            "expected 2 values for extrema, got {}",
            tuple.len()
        )));
    }
    Ok((tuple.get_item(0)?, tuple.get_item(1)?))
}

fn integer_extrema(value: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    let (minimum, maximum) = extrema_tuple(value)?;
    Ok((
        super::c_int_from_python(&minimum)?,
        super::c_int_from_python(&maximum)?,
    ))
}

fn float_extrema(value: &Bound<'_, PyAny>) -> PyResult<(f64, f64)> {
    let (minimum, maximum) = extrema_tuple(value)?;
    Ok((minimum.extract::<f64>()?, maximum.extract::<f64>()?))
}
