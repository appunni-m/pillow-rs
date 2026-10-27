use super::{
    Bound, PyAny, PyInt, PyList, PyOverflowError, PyResult, PyTuple, PyTypeError, PyValueError,
    chops_offset_from_python,
};
use pyo3::types::{PyAnyMethods, PyFloat, PySequenceMethods, PyTupleMethods, PyTypeMethods};

pub(crate) fn putpixel_value_from_python(
    value: &Bound<'_, PyAny>,
    mode: &str,
) -> PyResult<pillow_rs::PutPixelValue> {
    use pillow_rs::PutPixelValue;
    let tuple = value.cast::<PyTuple>().ok();
    let scalar = if let Some(tuple) = tuple.filter(|tuple| tuple.len() == 1) {
        tuple.get_item(0)?
    } else {
        value.clone()
    };
    if mode == "F" {
        return scalar.extract::<f64>().map(PutPixelValue::Float);
    }
    if scalar.is_instance_of::<PyInt>() {
        return scalar
            .extract::<i64>()
            .map(PutPixelValue::Integer)
            .map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyOverflowError::new_err("int too big to convert")
                } else {
                    error
                }
            });
    }
    let tuple_length = tuple.map(|tuple| tuple.len());
    let (length, palette_color) = putpixel_component_shape_from_python(value, mode, tuple_length)?;
    putpixel_components_from_python(value, length, palette_color)
}

pub(crate) fn putpixel_component_shape_from_python(
    value: &Bound<'_, PyAny>,
    mode: &str,
    tuple_length: Option<usize>,
) -> PyResult<(usize, bool)> {
    // Only P/PA's public palette wrapper accepts RGB(A) lists. Ordinary
    // getink accepts tuples, and unwraps a singleton before numeric coercion.
    let palette_list = matches!(mode, "P" | "PA")
        && value.is_instance_of::<PyList>()
        && matches!(value.len()?, 3 | 4);
    let length = if palette_list {
        Some(value.len()?)
    } else {
        tuple_length
    };
    let palette_color = matches!(mode, "P" | "PA") && matches!(length, Some(3 | 4));
    let single_band = matches!(mode, "1" | "L" | "P" | "I") || mode.starts_with("I;16");
    if single_band && !palette_color {
        return Err(PyTypeError::new_err(
            "color must be int or single-element tuple",
        ));
    }
    let Some(length) = length else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let two_band = matches!(mode, "LA" | "La" | "PA");
    if !palette_color
        && ((two_band && !matches!(length, 1 | 2)) || (!two_band && !matches!(length, 3 | 4)))
    {
        return Err(PyTypeError::new_err(if two_band {
            "color must be int, or tuple of one or two elements"
        } else {
            "color must be int, or tuple of one, three or four elements"
        }));
    }
    if length == 1 {
        return Err(pyo3::exceptions::PySystemError::new_err(
            "new style getargs format but argument is not a tuple",
        ));
    }
    Ok((length, palette_color))
}

pub(crate) fn putpixel_components_from_python(
    value: &Bound<'_, PyAny>,
    length: usize,
    palette_color: bool,
) -> PyResult<pillow_rs::PutPixelValue> {
    use pillow_rs::PutPixelValue;

    if palette_color
        && (0..length).any(|index| {
            value
                .get_item(index)
                .is_ok_and(|component| component.is_instance_of::<PyFloat>())
        })
    {
        return value
            .extract::<Vec<f64>>()
            .map(PutPixelValue::FloatComponents);
    }
    let mut components = Vec::with_capacity(length);
    for index in 0..length {
        let component = value.get_item(index)?;
        let integer = if palette_color && index < 3 {
            component.extract::<i64>().map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyValueError::new_err("bytes must be in range(0, 256)")
                } else {
                    error
                }
            })?
        } else if index == 0 || palette_color {
            component.extract::<i64>().map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyOverflowError::new_err("int too big to convert")
                } else {
                    error
                }
            })?
        } else {
            chops_offset_from_python(&component)? as i64
        };
        components.push(integer);
    }
    Ok(PutPixelValue::Components(components))
}

pub(crate) fn exact_rgb_tuple_from_python(value: &Bound<'_, PyAny>) -> Option<[u8; 3]> {
    let tuple = value.cast_exact::<PyTuple>().ok()?;
    if tuple.len() != 3 {
        return None;
    }
    let mut rgb = [0u8; 3];
    for (index, channel) in rgb.iter_mut().enumerate() {
        let value = tuple.get_item(index).ok()?;
        if value.cast_exact::<PyInt>().is_err() {
            return None;
        }
        *channel = value.extract::<u8>().ok()?;
    }
    Some(rgb)
}

pub(crate) fn new_color_input_from_python(
    color: Option<&Bound<'_, PyAny>>,
) -> pillow_rs::NewColorInput {
    let (hex, single, rgb, rgba, la, int32_val, float_val) = if let Some(value) = color {
        (
            value.extract::<String>().ok(),
            value.extract::<u8>().ok(),
            value.extract::<(u8, u8, u8)>().ok(),
            value.extract::<(u8, u8, u8, u8)>().ok(),
            value.extract::<(u8, u8)>().ok(),
            value.extract::<i32>().ok(),
            value.extract::<f64>().ok(),
        )
    } else {
        (None, None, None, None, None, None, None)
    };
    pillow_rs::NewColorInput::from_parts(
        hex,
        single,
        rgb,
        rgba,
        la,
        int32_val,
        float_val,
        color.is_some(),
    )
}

pub(crate) fn putpixel_coordinates_from_python(xy: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    let sequence = xy.cast::<pyo3::types::PySequence>().map_err(|_| {
        let name = if xy.is_none() {
            "None".into()
        } else {
            xy.get_type()
                .name()
                .map(|name| name.to_string())
                .unwrap_or_default()
        };
        PyTypeError::new_err(format!("argument 1 must be 2-item sequence, not {name}"))
    })?;
    let length = sequence.len()?;
    if length != 2 {
        return Err(PyTypeError::new_err(format!(
            "argument 1 must be sequence of length 2, not {length}"
        )));
    }
    Ok((
        chops_offset_from_python(&sequence.get_item(0)?)? as i32,
        chops_offset_from_python(&sequence.get_item(1)?)? as i32,
    ))
}

pub(crate) fn transpose_input_from_python(
    method: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::TransposeInput> {
    if let Ok(value) = method.extract::<i64>() {
        return Ok(pillow_rs::TransposeInput::Index(value));
    }
    if let Ok(value) = method.extract::<String>() {
        return Ok(pillow_rs::TransposeInput::Name(value));
    }
    Ok(pillow_rs::TransposeInput::Invalid(
        method.get_type().name()?.to_string(),
    ))
}
