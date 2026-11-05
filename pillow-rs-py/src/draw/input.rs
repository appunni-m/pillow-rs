use pyo3::prelude::{Bound, PyAny, PyErr, PyResult};
use pyo3::types::{
    PyAnyMethods, PyBool, PyByteArray, PyBytes, PyBytesMethods, PyDict, PyFloat, PyInt, PyList,
    PyListMethods, PyString, PyTuple, PyTupleMethods, PyTypeMethods,
};

pub(super) fn draw_bitmap_position_from_python(
    xy: &Bound<'_, PyAny>,
) -> PyResult<(i32, i32)> {
    if let Some(position) = draw_bitmap_position_from_float_buffer(xy) {
        return position;
    }
    if !draw_bitmap_is_sequence(xy) {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "argument must be sequence",
        ));
    }
    let length = draw_bitmap_sequence_length(xy)?;
    let mut coordinates = [0.0; 2];
    let mut coordinate_count = 0;

    for index in 0..length {
        let item = match draw_bitmap_sequence_item(xy, index) {
            Ok(item) => item,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyIndexError>(xy.py()) => {
                break;
            }
            Err(error) => return Err(error),
        };
        append_draw_bitmap_coordinate(&item, &mut coordinates, &mut coordinate_count)?;
    }

    if coordinate_count % 2 != 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "wrong number of coordinates",
        ));
    }
    if coordinate_count != 2 {
        return Err(draw_bitmap_single_point_error());
    }
    Ok((coordinates[0] as i32, coordinates[1] as i32))
}

fn append_draw_bitmap_coordinate(
    item: &Bound<'_, PyAny>,
    coordinates: &mut [f64; 2],
    coordinate_count: &mut usize,
) -> PyResult<()> {
    if draw_bitmap_value_is_numeric(item) {
        append_draw_bitmap_value(
            draw_bitmap_sequence_number(item)?,
            coordinates,
            coordinate_count,
        );
        return Ok(());
    }
    if let Ok(list) = item.cast::<PyList>() {
        if list.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "coordinate list must contain exactly 2 coordinates",
            ));
        }
        for index in 0..2 {
            let value = list.get_item(index)?;
            if !draw_bitmap_value_is_numeric(&value) {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "coordinate list must contain numbers",
                ));
            }
            append_draw_bitmap_value(
                draw_bitmap_sequence_number(&value)?,
                coordinates,
                coordinate_count,
            );
        }
        return Ok(());
    }
    if let Ok(tuple) = item.cast::<PyTuple>() {
        let pair = tuple.extract::<(f64, f64)>().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err("incorrect coordinate type")
        })?;
        append_draw_bitmap_value(pair.0, coordinates, coordinate_count);
        append_draw_bitmap_value(pair.1, coordinates, coordinate_count);
        return Ok(());
    }
    Err(pyo3::exceptions::PyValueError::new_err(
        "incorrect coordinate type",
    ))
}

fn draw_bitmap_sequence_number(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    if value.is_instance_of::<PyInt>() {
        // Pillow's path adapter narrows Python integers through C `long` and
        // then `float` before storing them as a double coordinate.
        return value
            .extract::<std::os::raw::c_long>()
            .map(|value| f64::from(value as f32));
    }
    value.extract::<f64>()
}

fn draw_bitmap_position_from_float_buffer(
    xy: &Bound<'_, PyAny>,
) -> Option<PyResult<(i32, i32)>> {
    // SAFETY: Py_buffer is a C output structure whose all-zero state is valid
    // input for PyObject_GetBuffer to initialize.
    let mut view: pyo3::ffi::Py_buffer = unsafe { std::mem::zeroed() };
    // Pillow requests a simple buffer and interprets its contents as native
    // float32 coordinates, regardless of the exporter's declared format.
    let acquired = unsafe {
        pyo3::ffi::PyObject_GetBuffer(xy.as_ptr(), &mut view, pyo3::ffi::PyBUF_SIMPLE)
    };
    if acquired < 0 {
        // PyPath_Flatten clears buffer acquisition errors and tries sequences.
        unsafe { pyo3::ffi::PyErr_Clear() };
        return None;
    }

    let result = draw_bitmap_position_from_float_buffer_view(&view);
    // SAFETY: a successful PyObject_GetBuffer initialized `view`, and this
    // function releases exactly that acquired view while the GIL is held.
    unsafe { pyo3::ffi::PyBuffer_Release(&mut view) };
    Some(result)
}

fn draw_bitmap_position_from_float_buffer_view(
    view: &pyo3::ffi::Py_buffer,
) -> PyResult<(i32, i32)> {
    let float_pair_bytes = 2 * std::mem::size_of::<f32>();
    let point_count = usize::try_from(view.len)
        .ok()
        .map(|length| length / float_pair_bytes)
        .unwrap_or_default();
    if point_count != 1 || view.buf.is_null() {
        return Err(draw_bitmap_single_point_error());
    }

    // SAFETY: one point requires at least two f32 values. PyBUF_SIMPLE gives a
    // contiguous byte view; read_unaligned handles exporters without f32
    // alignment. The view stays acquired until both values have been copied.
    let coordinates = unsafe {
        let values = view.buf.cast::<f32>();
        [
            std::ptr::read_unaligned(values),
            std::ptr::read_unaligned(values.add(1)),
        ]
    };
    Ok((coordinates[0] as i32, coordinates[1] as i32))
}

fn draw_bitmap_single_point_error() -> PyErr {
    pyo3::exceptions::PyTypeError::new_err(
        "coordinate list must contain exactly 1 coordinate",
    )
}

fn append_draw_bitmap_value(
    value: f64,
    coordinates: &mut [f64; 2],
    coordinate_count: &mut usize,
) {
    if *coordinate_count < coordinates.len() {
        coordinates[*coordinate_count] = value;
    }
    *coordinate_count = (*coordinate_count).saturating_add(1);
}

fn draw_bitmap_value_is_numeric(value: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `value` is a live GIL-bound Python object. PyNumber_Check reads
    // its type slots and neither takes ownership nor retains the pointer.
    unsafe { pyo3::ffi::PyNumber_Check(value.as_ptr()) != 0 }
}

fn draw_bitmap_is_sequence(value: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `value` is a live GIL-bound Python object. PySequence_Check only
    // inspects its type slots and does not retain the pointer.
    unsafe { pyo3::ffi::PySequence_Check(value.as_ptr()) != 0 }
}

fn draw_bitmap_sequence_length(value: &Bound<'_, PyAny>) -> PyResult<usize> {
    // SAFETY: the live Python object passed PySequence_Check immediately before
    // this call, and the GIL is held for the duration of the binding.
    let length = unsafe { pyo3::ffi::PySequence_Size(value.as_ptr()) };
    if length < 0 {
        return Err(PyErr::fetch(value.py()));
    }
    Ok(length as usize)
}

fn draw_bitmap_sequence_item(
    value: &Bound<'_, PyAny>,
    index: usize,
) -> PyResult<Bound<'_, PyAny>> {
    // PySequence_Size returns Py_ssize_t, so every loop index is representable
    // as isize. The live sequence and GIL satisfy PySequence_GetItem's inputs.
    let index = isize::try_from(index).map_err(|_| {
        pyo3::exceptions::PyOverflowError::new_err("coordinate sequence is too large")
    })?;
    // SAFETY: `value` passed PySequence_Check and remains live under the GIL;
    // PySequence_GetItem returns a new owned reference or NULL with an error.
    unsafe {
        Bound::from_owned_ptr_or_err(
            value.py(),
            pyo3::ffi::PySequence_GetItem(value.as_ptr(), index),
        )
    }
}

pub(super) fn draw_points_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawPointsInput {
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawPointsInput::Flat(values);
    }
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawPointsInput::Nested(points);
    }
    if xy.is_instance_of::<PyList>() || xy.is_instance_of::<PyTuple>() {
        return pillow_rs::DrawPointsInput::InvalidSequence;
    }
    pillow_rs::DrawPointsInput::Invalid
}

pub(super) fn draw_box_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawBoxInput {
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawBoxInput::Nested(points);
    }
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawBoxInput::Flat(values);
    }
    pillow_rs::DrawBoxInput::Invalid
}

pub(super) fn draw_circle_center_input_from_python(
    xy: &Bound<'_, PyAny>,
) -> pillow_rs::DrawCircleCenterInput {
    if xy.is_instance_of::<PyInt>() && !xy.is_instance_of::<PyBool>() {
        return pillow_rs::DrawCircleCenterInput::Integer;
    }
    if xy.is_instance_of::<PyDict>() {
        return pillow_rs::DrawCircleCenterInput::Mapping;
    }
    if xy.extract::<String>().is_ok() {
        return pillow_rs::DrawCircleCenterInput::Text;
    }
    xy.extract::<Vec<f64>>().map_or(
        pillow_rs::DrawCircleCenterInput::Invalid,
        pillow_rs::DrawCircleCenterInput::Values,
    )
}

pub(super) fn draw_color_input_from_python(
    val: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::DrawColorInput> {
    let Some(val) = val else {
        return Ok(pillow_rs::DrawColorInput::None);
    };
    if let Ok(value) = val.extract::<String>() {
        return Ok(pillow_rs::DrawColorInput::String(value));
    }
    if val.is_instance_of::<PyInt>() {
        return match crate::inputs::palette_integer_component_from_python(val) {
            Some(pillow_rs::PaletteColorComponent::Integer(value)) => {
                Ok(pillow_rs::DrawColorInput::Integer(value))
            }
            Some(value @ pillow_rs::PaletteColorComponent::WideInteger { .. }) => {
                Ok(pillow_rs::DrawColorInput::WideInteger(value))
            }
            Some(pillow_rs::PaletteColorComponent::Float(_)) => {
                unreachable!("Python integers are retained as integer components")
            }
            None => val
                .extract::<i64>()
                .map(pillow_rs::DrawColorInput::Integer),
        };
    }
    if let Ok(value) = val.extract::<f64>() {
        return Ok(pillow_rs::DrawColorInput::Float(value));
    }
    if val.cast::<PyTuple>().is_ok() {
        return Ok(match draw_integer_tuple_components(val) {
            Some((values, false)) => pillow_rs::DrawColorInput::TupleComponents(
                values
                    .into_iter()
                    .map(|component| match component {
                        pillow_rs::PaletteColorComponent::Integer(value) => value,
                        _ => unreachable!("integer tuple values use integer components"),
                    })
                    .collect(),
            ),
            Some((values, true)) => {
                pillow_rs::DrawColorInput::TupleComponentsWithWideInteger(values)
            }
            None => match draw_float_tuple_components(val) {
                Some(values) => pillow_rs::DrawColorInput::TupleFloatComponents(values),
                None => match draw_mixed_numeric_tuple_components(val) {
                    Some(values) => pillow_rs::DrawColorInput::TupleMixedComponents(values),
                    None => pillow_rs::DrawColorInput::InvalidTuple,
                },
            },
        });
    }
    if val.cast::<PyList>().is_ok() {
        return Ok(val.extract::<Vec<i64>>().map_or(
            pillow_rs::DrawColorInput::InvalidList,
            pillow_rs::DrawColorInput::ListComponents,
        ));
    }
    Ok(pillow_rs::DrawColorInput::Invalid)
}

/// Keeps arbitrary-size Python integers on the integer-tuple route.
///
/// Their exact magnitude preserves `ImagePalette.colors` key equality after a
/// failed byte conversion without narrowing Python's integer value.
fn draw_integer_tuple_components(
    val: &Bound<'_, PyAny>,
) -> Option<(Vec<pillow_rs::PaletteColorComponent>, bool)> {
    if let Ok(values) = val.extract::<Vec<i64>>() {
        return Some((
            values
                .into_iter()
                .map(pillow_rs::PaletteColorComponent::Integer)
                .collect(),
            false,
        ));
    }

    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    let mut contains_wide_integer = false;
    for component in tuple.iter() {
        if !component.is_instance_of::<PyInt>() {
            return None;
        }
        let value = crate::inputs::palette_integer_component_from_python(&component)?;
        contains_wide_integer |= matches!(
            &value,
            pillow_rs::PaletteColorComponent::WideInteger { .. }
        );
        values.push(value);
    }
    Some((values, contains_wide_integer))
}

/// Preserves Python float tuple values for palette equality and error ordering.
fn draw_float_tuple_components(val: &Bound<'_, PyAny>) -> Option<Vec<f64>> {
    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    for component in tuple.iter() {
        if !component.is_instance_of::<PyFloat>() {
            return None;
        }
        values.push(component.extract::<f64>().ok()?);
    }
    Some(values)
}

/// Retains integer/float kinds so P palette errors follow tuple byte order.
fn draw_mixed_numeric_tuple_components(
    val: &Bound<'_, PyAny>,
) -> Option<Vec<pillow_rs::PaletteColorComponent>> {
    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    let mut contains_integer = false;
    let mut contains_float = false;
    for component in tuple.iter() {
        if component.is_instance_of::<PyInt>() {
            contains_integer = true;
            values.push(crate::inputs::palette_integer_component_from_python(&component)?);
        } else if component.is_instance_of::<PyFloat>() {
            contains_float = true;
            values.push(pillow_rs::PaletteColorComponent::Float(
                component.extract::<f64>().ok()?,
            ));
        } else {
            return None;
        }
    }
    (contains_integer && contains_float).then_some(values)
}

pub(super) fn draw_features_presence_from_python(
    features: Option<&Bound<'_, PyAny>>,
) -> bool {
    // This build has BASIC layout only. Pillow rejects every non-None feature
    // object before iterating or converting it, so retain presence without
    // invoking Python sequence callbacks.
    features.is_some()
}

pub(super) fn draw_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontTextInput> {
    if let Ok(value) = value.cast::<PyBytes>() {
        return Ok(pillow_rs::ImageFontTextInput::Bytes(
            value.as_bytes().to_vec(),
        ));
    }
    if let Ok(value) = value.cast::<PyString>() {
        return Ok(pillow_rs::ImageFontTextInput::Text(
            value.to_string_lossy().into_owned(),
        ));
    }
    if value.is_instance_of::<PyByteArray>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "expected string or bytes",
        ));
    }
    let type_name = value.get_type().name()?.to_string();
    Err(pyo3::exceptions::PyAttributeError::new_err(format!(
        "'{type_name}' object has no attribute 'split'"
    )))
}
