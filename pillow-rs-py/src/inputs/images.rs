use super::{Bound, PyAny, PyList, PyResult, PyTuple};
use crate::PyImage;
use pillow_rs::Image as RsImage;
use pyo3::types::{PyAnyMethods, PyTypeMethods};

pub(crate) fn image_from_python(value: &Bound<'_, PyAny>) -> Option<RsImage> {
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

pub(crate) fn paste_source_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::PasteSourceInput {
    if let Some(image) = image_from_python(value) {
        return pillow_rs::PasteSourceInput::Image(Box::new(image));
    }
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::PasteSourceInput::Scalar(value);
    }
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::PasteSourceInput::Float(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::PasteSourceInput::Components(values);
    }
    if let Ok(value) = value.extract::<String>() {
        return pillow_rs::PasteSourceInput::String(value);
    }
    pillow_rs::PasteSourceInput::Invalid
}

pub(crate) fn paste_box_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PasteBoxInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::PasteBoxInput::None);
    };
    if let Some(image) = image_from_python(value) {
        return Ok(pillow_rs::PasteBoxInput::Image(Box::new(image)));
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return Ok(pillow_rs::PasteBoxInput::Values(values));
    }
    Ok(pillow_rs::PasteBoxInput::Invalid {
        length: value.len().ok(),
        type_name: value.get_type().name()?.to_string(),
    })
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
