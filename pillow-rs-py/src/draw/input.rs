use pyo3::prelude::{Bound, PyAny, PyErr, PyRef, PyResult};
use pyo3::types::{
    PyAnyMethods, PyBool, PyByteArray, PyDict, PyFloat, PyInt, PyList, PyListMethods, PySequence,
    PySequenceMethods, PyTuple, PyTupleMethods, PyTypeMethods,
};

use crate::PillowFloatPointBuffer;

pub(super) fn draw_bitmap_position_from_python(xy: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    let mut coordinates = [0.0; 2];
    let mut coordinate_count = 0;
    let flattened_count = draw_path_flatten_from_python(xy, |coordinate| {
        if coordinate_count < coordinates.len() {
            coordinates[coordinate_count] = coordinate;
        }
        coordinate_count = coordinate_count.saturating_add(1);
        Ok(())
    })?;
    if flattened_count != 2 {
        return Err(draw_bitmap_single_point_error());
    }
    Ok((coordinates[0] as i32, coordinates[1] as i32))
}

fn append_draw_path_coordinate(
    item: &Bound<'_, PyAny>,
    emit: &mut impl FnMut(f64) -> PyResult<()>,
    coordinate_count: &mut usize,
) -> PyResult<()> {
    if draw_path_value_is_numeric(item) {
        emit_draw_path_coordinate(draw_path_sequence_number(item)?, emit, coordinate_count)?;
        return Ok(());
    }
    if let Ok(list) = item.cast::<PyList>() {
        if list.len() != 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "coordinate list must contain exactly 2 coordinates",
            ));
        }
        let x = list.get_item(0)?;
        let y = list.get_item(1)?;
        if !draw_path_value_is_numeric(&x) || !draw_path_value_is_numeric(&y) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "coordinate list must contain numbers",
            ));
        }
        emit_draw_path_coordinate(draw_path_sequence_number(&x)?, emit, coordinate_count)?;
        emit_draw_path_coordinate(draw_path_sequence_number(&y)?, emit, coordinate_count)?;
        return Ok(());
    }
    if let Ok(tuple) = item.cast::<PyTuple>() {
        let pair = tuple
            .extract::<(f64, f64)>()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("incorrect coordinate type"))?;
        emit_draw_path_coordinate(pair.0, emit, coordinate_count)?;
        emit_draw_path_coordinate(pair.1, emit, coordinate_count)?;
        return Ok(());
    }
    Err(pyo3::exceptions::PyValueError::new_err(
        "incorrect coordinate type",
    ))
}

fn draw_path_sequence_number(value: &Bound<'_, PyAny>) -> PyResult<f64> {
    if value.is_instance_of::<PyFloat>() {
        return value.extract::<f64>();
    }
    if value.is_instance_of::<PyInt>() {
        // Pillow's path adapter narrows Python integers through C `long` and
        // then `float` before storing them as a double coordinate. PyLong_Check
        // also includes int subclasses, so keep their stored integer value on
        // this conversion path instead of invoking their Python `__float__`.
        return value
            .extract::<std::os::raw::c_long>()
            .map(|value| f64::from(value as f32));
    }
    value.extract::<f64>()
}

fn draw_path_coordinates_from_python(value: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    let mut coordinates = Vec::new();
    draw_path_flatten_from_python(value, |coordinate| {
        coordinates
            .try_reserve(1)
            .map_err(|_| pyo3::exceptions::PyMemoryError::new_err("path is too large"))?;
        coordinates.push(coordinate);
        Ok(())
    })?;
    Ok(coordinates)
}

fn draw_path_flatten_from_python(
    value: &Bound<'_, PyAny>,
    mut emit: impl FnMut(f64) -> PyResult<()>,
) -> PyResult<usize> {
    let mut coordinate_count = 0;
    if let Ok(path) = value.extract::<PyRef<'_, crate::image_path::PyImagePath>>() {
        for [x, y] in path.points_for_draw() {
            emit_draw_path_coordinate(x, &mut emit, &mut coordinate_count)?;
            emit_draw_path_coordinate(y, &mut emit, &mut coordinate_count)?;
        }
        return Ok(coordinate_count);
    }
    if let Some(buffer) = PillowFloatPointBuffer::from_python(value) {
        let mut coordinate_count = 0;
        for index in 0..buffer.coordinate_count() {
            emit_draw_path_coordinate(buffer.coordinate(index)?, &mut emit, &mut coordinate_count)?;
        }
        return Ok(coordinate_count);
    }
    if !draw_path_is_sequence(value) {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "argument must be sequence",
        ));
    }
    let length = draw_path_sequence_length(value)?;
    let is_builtin_sequence = value.cast::<PyList>().is_ok() || value.cast::<PyTuple>().is_ok();
    for index in 0..length {
        let item = match draw_path_sequence_item(value, index) {
            Ok(item) => item,
            Err(error)
                if !is_builtin_sequence
                    && error.is_instance_of::<pyo3::exceptions::PyIndexError>(value.py()) =>
            {
                break;
            }
            Err(error) => return Err(error),
        };
        append_draw_path_coordinate(&item, &mut emit, &mut coordinate_count)?;
    }
    if coordinate_count % 2 != 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "wrong number of coordinates",
        ));
    }
    Ok(coordinate_count)
}

fn emit_draw_path_coordinate(
    coordinate: f64,
    emit: &mut impl FnMut(f64) -> PyResult<()>,
    coordinate_count: &mut usize,
) -> PyResult<()> {
    emit(coordinate)?;
    *coordinate_count = coordinate_count.saturating_add(1);
    Ok(())
}

fn draw_bitmap_single_point_error() -> PyErr {
    pyo3::exceptions::PyTypeError::new_err("coordinate list must contain exactly 1 coordinate")
}

fn draw_path_value_is_numeric(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
        || value.get_type().hasattr("__float__").unwrap_or(false)
        || value.get_type().hasattr("__index__").unwrap_or(false)
}

fn draw_path_is_sequence(value: &Bound<'_, PyAny>) -> bool {
    value.cast::<PySequence>().is_ok()
}

fn draw_path_sequence_length(value: &Bound<'_, PyAny>) -> PyResult<usize> {
    Ok(value.cast::<PySequence>()?.len()?)
}

fn draw_path_sequence_item<'py>(
    value: &Bound<'py, PyAny>,
    index: usize,
) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(list) = value.cast::<PyList>() {
        return list.get_item(index);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return tuple.get_item(index);
    }
    value.cast::<PySequence>()?.get_item(index)
}

pub(super) fn draw_points_input_from_python(
    xy: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::DrawPointsInput> {
    Ok(pillow_rs::DrawPointsInput::PillowCoordinates(
        draw_path_coordinates_from_python(xy)?,
    ))
}

pub(super) fn draw_box_input_from_python(
    xy: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::DrawBoxInput> {
    Ok(pillow_rs::DrawBoxInput::PillowCoordinates(
        draw_path_coordinates_from_python(xy)?,
    ))
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

pub(crate) fn draw_color_input_from_python(
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
            None => val.extract::<i64>().map(pillow_rs::DrawColorInput::Integer),
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
        contains_wide_integer |=
            matches!(&value, pillow_rs::PaletteColorComponent::WideInteger { .. });
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
            values.push(crate::inputs::palette_integer_component_from_python(
                &component,
            )?);
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

pub(crate) fn draw_features_presence_from_python(features: Option<&Bound<'_, PyAny>>) -> bool {
    // This build has BASIC layout only. Pillow rejects every non-None feature
    // object before iterating or converting it, so retain presence without
    // invoking Python sequence callbacks.
    features.is_some()
}

pub(crate) fn draw_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontTextInput> {
    let input = crate::inputs::font_text_input_from_python(value)?;
    if !matches!(&input, pillow_rs::ImageFontTextInput::InvalidType) {
        return Ok(input);
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
