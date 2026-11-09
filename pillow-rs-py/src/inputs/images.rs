use super::{Bound, Py, PyAny, PyList, PyResult, PyTuple, Python};
use crate::PyImage;
use pillow_rs::Image as RsImage;
use pyo3::types::{
    PyAnyMethods, PyBytes, PyFloat, PyInt, PySequence, PySequenceMethods, PyTupleMethods,
    PyTypeMethods,
};

pub(crate) fn image_from_python(value: &Bound<'_, PyAny>) -> Option<RsImage> {
    if let Ok(image) = value.cast::<PyImage>() {
        return Some(image.borrow().inner.clone());
    }
    value.getattr("_rust_image").ok().and_then(|inner| {
        inner
            .cast::<PyImage>()
            .ok()
            .map(|image| image.borrow().inner.clone())
    })
}

/// Converts a host iterable into Rust-owned merge input classifications.
///
/// Object extraction and type-name preservation are the only binding
/// responsibilities here. The core owns invalid-item errors and merge
/// mode/arity validation through
/// `image_merge_inputs`.
pub(crate) fn merge_inputs_from_python(
    values: &Bound<'_, PyAny>,
) -> PyResult<Vec<pillow_rs::MergeInput>> {
    values
        .try_iter()?
        .map(|item| {
            let obj = item?;
            Ok(match image_from_python(&obj) {
                Some(image) => pillow_rs::MergeInput::Image(Box::new(image)),
                None => pillow_rs::MergeInput::Invalid(obj.get_type().name()?.to_string()),
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum PasteIntegerField {
    LongLong,
    CInt,
}

fn paste_component_from_python(
    value: &Bound<'_, PyAny>,
    integer_field: Option<PasteIntegerField>,
) -> PyResult<pillow_rs::PasteColorComponent> {
    if let Some(field) = integer_field {
        let integer = if value.is_instance_of::<PyInt>() {
            value.clone()
        } else {
            value
                .py()
                .import("operator")?
                .call_method1("index", (value,))?
        };
        return match field {
            PasteIntegerField::LongLong => Ok(pillow_rs::PasteColorComponent::Integer(
                integer.extract::<i64>()?,
            )),
            PasteIntegerField::CInt => {
                let value = integer.extract::<std::os::raw::c_long>()?;
                let value = i32::try_from(value).map_err(|_| {
                    pyo3::exceptions::PyOverflowError::new_err(if value < 0 {
                        "signed integer is less than minimum"
                    } else {
                        "signed integer is greater than maximum"
                    })
                })?;
                Ok(pillow_rs::PasteColorComponent::Integer(i64::from(value)))
            }
        };
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(match value.extract::<i64>() {
            Ok(value) => pillow_rs::PasteColorComponent::Integer(value),
            Err(_) => pillow_rs::PasteColorComponent::WideInteger { float_value: None },
        });
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(pillow_rs::PasteColorComponent::Float(
            value.extract::<f64>()?,
        ));
    }
    Ok(paste_invalid_component(value))
}

fn paste_tuple_integer_field(
    mode: pillow_rs::Mode,
    length: usize,
    index: usize,
) -> Option<PasteIntegerField> {
    if mode.pillow_name().is_none() {
        return None;
    }
    match (mode.band_count(), length, index) {
        (2, 2, 0) | (3 | 4, 3 | 4, 0) => Some(PasteIntegerField::LongLong),
        (2, 2, 1) | (3 | 4, 3 | 4, 1..=3) => Some(PasteIntegerField::CInt),
        _ => None,
    }
}

fn paste_invalid_component(value: &Bound<'_, PyAny>) -> pillow_rs::PasteColorComponent {
    pillow_rs::PasteColorComponent::Invalid {
        type_name: value
            .get_type()
            .name()
            .map_or_else(|_| "object".to_owned(), |name| name.to_string()),
    }
}

pub(crate) fn paste_source_from_python(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::PasteSourceInput> {
    if let Some(image) = image_from_python(value) {
        return Ok(pillow_rs::PasteSourceInput::Image(Box::new(image)));
    }
    if mode == pillow_rs::Mode::ImageF32 {
        if let Ok(values) = value.cast::<PyTuple>() {
            if values.len() == 1 {
                return Ok(pillow_rs::PasteSourceInput::Float(
                    values.get_item(0)?.extract::<f64>()?,
                ));
            }
            return Ok(pillow_rs::PasteSourceInput::TupleComponents(Vec::new()));
        }
        // Pillow calls PyFloat_AsDouble for every F-mode color, including
        // Python objects with __float__ and integer inputs.
        return Ok(pillow_rs::PasteSourceInput::Float(value.extract::<f64>()?));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(match value.extract::<i64>() {
            Ok(value) => pillow_rs::PasteSourceInput::Scalar(value),
            Err(_) => pillow_rs::PasteSourceInput::WideInteger { float_value: None },
        });
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(pillow_rs::PasteSourceInput::Float(value.extract::<f64>()?));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(pillow_rs::PasteSourceInput::String(value));
    }
    if let Ok(values) = value.cast::<PyTuple>() {
        let mut components = Vec::with_capacity(values.len());
        for (index, component) in values.iter().enumerate() {
            let field = paste_tuple_integer_field(mode, values.len(), index);
            components.push(paste_component_from_python(&component, field)?);
        }
        return Ok(pillow_rs::PasteSourceInput::TupleComponents(components));
    }
    if value.cast::<PyList>().is_ok() {
        // Pillow's C parser rejects lists before inspecting any list element.
        return Ok(pillow_rs::PasteSourceInput::List);
    }
    Ok(pillow_rs::PasteSourceInput::InvalidType(
        value
            .get_type()
            .name()
            .map_or_else(|_| "object".to_owned(), |name| name.to_string()),
    ))
}

pub(crate) fn paste_box_coordinate_from_python(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    // PyO3's c_long extractor calls PyLong_AsLong, matching the `(i)` field
    // conversion used by Pillow's PyArg_ParseTuple parser. Check C-int range
    // here so a failing value stops parsing before the next item is read.
    let coordinate = value.extract::<std::os::raw::c_long>()?;
    let coordinate = i32::try_from(coordinate).map_err(|_| {
        pyo3::exceptions::PyOverflowError::new_err(if coordinate < 0 {
            "signed integer is less than minimum"
        } else {
            "signed integer is greater than maximum"
        })
    })?;
    Ok(i64::from(coordinate))
}

fn paste_box_sequence_coordinates(value: &Bound<'_, PyAny>, _py: Python<'_>) -> PyResult<Vec<i64>> {
    let Ok(sequence) = value.cast::<PySequence>() else {
        let type_name = value.get_type().name()?.to_string();
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "argument 2 must be 4-item sequence, not {type_name}"
        )));
    };
    if value.is_instance_of::<PyBytes>() {
        let type_name = value.get_type().name()?.to_string();
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "argument 2 must be 4-item sequence, not {type_name}"
        )));
    }

    // Match PyArg_ParseTuple's `(iiii)` parser: check length once, then fetch
    // and convert each item in order so callbacks run at Pillow's boundaries.
    let length = sequence.len()?;
    if length != 4 {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "argument 2 must be sequence of length 4, not {length}"
        )));
    }

    let mut coordinates = Vec::with_capacity(4);
    for index in 0..4 {
        let item = match sequence.get_item(index) {
            Ok(item) => item,
            Err(_sequence_error) => {
                // Pillow clears failures from PySequence_GetItem and reports
                // the argument parser's stable retrieval error.
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "argument 2, item {index} is not retrievable"
                )));
            }
        };
        coordinates.push(paste_box_coordinate_from_python(&item)?);
    }
    Ok(coordinates)
}

pub(crate) fn resolve_deferred_paste_box_coordinates(
    coordinates: &DeferredPasteBoxCoordinates,
    py: Python<'_>,
) -> PyResult<Vec<i64>> {
    paste_box_sequence_coordinates(coordinates.value.bind(py), py)
}

#[derive(Clone)]
pub(crate) struct DeferredPasteBoxCoordinates {
    value: Py<PyAny>,
}

pub(crate) fn paste_box_for_staged_plan(
    value: Option<&Bound<'_, PyAny>>,
    known_length: Option<usize>,
) -> PyResult<(
    pillow_rs::PasteBoxInput,
    Option<DeferredPasteBoxCoordinates>,
)> {
    let Some(value) = value else {
        return Ok((pillow_rs::PasteBoxInput::None, None));
    };
    let _initial_length = match known_length {
        Some(length) => length,
        None => value.len()?,
    };
    // Pillow checks `len(box)` before source work, but its C paste parser checks
    // the final box arity and coordinates after destination/mask loading.
    // The facade has already handled Image-as-mask shorthand; an Image here
    // may be a custom two-coordinate `__iadd__` result and stays a raw box.
    let pending = DeferredPasteBoxCoordinates {
        value: value.clone().unbind(),
    };
    // This valid placeholder keeps the core plan non-failing until the Python
    // wrapper reaches the same post-load parsing point as Pillow's C method.
    Ok((pillow_rs::PasteBoxInput::Values(vec![0; 4]), Some(pending)))
}

pub(crate) fn paste_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PasteMaskInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::PasteMaskInput::None);
    };
    if let Some(image) = image_from_python(value) {
        return Ok(pillow_rs::PasteMaskInput::Image(Box::new(image)));
    }
    Ok(pillow_rs::PasteMaskInput::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

pub(crate) fn open_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::OpenModeInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::OpenModeInput::None);
    };
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::OpenModeInput::Name(name));
    }
    Ok(pillow_rs::OpenModeInput::Invalid(value.str()?.to_string()))
}

pub(crate) fn open_formats_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::OpenFormatsInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::OpenFormatsInput::None);
    };
    if value.cast::<PyList>().is_err() && value.cast::<PyTuple>().is_err() {
        return Ok(pillow_rs::OpenFormatsInput::Invalid(
            value.get_type().name()?.to_string(),
        ));
    }
    match value.extract::<Vec<String>>() {
        Ok(names) => Ok(pillow_rs::OpenFormatsInput::Names(names)),
        Err(_) => Ok(pillow_rs::OpenFormatsInput::Invalid(
            value.get_type().name()?.to_string(),
        )),
    }
}
