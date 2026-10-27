use super::{
    Bound, PyAny, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyTuple, PyTypeError,
    PyValueError,
};
use pyo3::types::{PyAnyMethods, PyBool, PyListMethods, PyTupleMethods, PyTypeMethods};

pub(crate) fn resample_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<pillow_rs::ResampleInput>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Ok(code) = value.extract::<i64>() {
        return Ok(Some(pillow_rs::ResampleInput::Code(code)));
    }
    if let Ok(name) = value.extract::<String>() {
        return Ok(Some(pillow_rs::ResampleInput::Name(name)));
    }
    let display = value.str()?.to_string();
    Ok(Some(pillow_rs::ResampleInput::Name(display)))
}

pub(crate) fn rotate_resample_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::RotateResampleInput> {
    match value {
        None => Ok(pillow_rs::RotateResampleInput::None),
        Some(value) => {
            if let Ok(code) = value.extract::<i64>() {
                return Ok(pillow_rs::RotateResampleInput::Code(code));
            }
            if let Ok(name) = value.extract::<String>() {
                return Ok(pillow_rs::RotateResampleInput::Name(name));
            }
            // Pillow's Image.transform formats unsupported host objects in
            // its unknown-filter error; pass that neutral display form to
            // core, where the validation and error remain centralized.
            Ok(pillow_rs::RotateResampleInput::Name(
                value.str()?.to_string(),
            ))
        }
    }
}

pub(crate) fn rotate_expand_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::RotateExpandInput> {
    match value {
        Some(value) if value.is_instance_of::<PyBool>() => Ok(
            pillow_rs::RotateExpandInput::Boolean(value.extract::<bool>()?),
        ),
        None => Ok(pillow_rs::RotateExpandInput::Boolean(false)),
        Some(value) => match value.extract::<i64>() {
            Ok(value) => Ok(pillow_rs::RotateExpandInput::Integer(value)),
            Err(_) => Ok(pillow_rs::RotateExpandInput::Boolean(value.is_truthy()?)),
        },
    }
}

pub(crate) fn rotate_point_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::RotatePointInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::RotatePointInput::Default);
    };
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return Ok(pillow_rs::RotatePointInput::Values(values));
    }
    Ok(pillow_rs::RotatePointInput::Invalid {
        type_name: value.get_type().name()?.to_string(),
        truthy: value.is_truthy()?,
    })
}

pub(crate) fn imageops_color_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> pillow_rs::ImageOpsColor {
    let Some(value) = value else {
        return pillow_rs::ImageOpsColor::None;
    };
    if let Ok(value) = value.extract::<String>() {
        return pillow_rs::ImageOpsColor::Name(value);
    }
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::ImageOpsColor::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::ImageOpsColor::Components(values);
    }
    pillow_rs::ImageOpsColor::Invalid
}

pub(crate) fn convert_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ConvertModeInput> {
    match value {
        None => Ok(pillow_rs::ConvertModeInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::ConvertModeInput::Name(value)),
            Err(_) => Ok(pillow_rs::ConvertModeInput::Invalid(
                value.get_type().name()?.to_string(),
            )),
        },
    }
}

pub(crate) fn convert_palette_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ConvertPaletteInput> {
    match value {
        None => Ok(pillow_rs::ConvertPaletteInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::ConvertPaletteInput::Name(value)),
            Err(_) => Ok(pillow_rs::ConvertPaletteInput::Invalid(
                value.get_type().name()?.to_string(),
            )),
        },
    }
}

pub(crate) fn transform_data_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<pillow_rs::TransformData>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if let Ok(matrix) = value.extract::<Vec<f64>>() {
        return Ok(Some(pillow_rs::TransformData::Affine(matrix)));
    }
    if let Ok(mesh) = value.extract::<Vec<(Vec<f64>, Vec<f64>)>>() {
        return Ok(Some(pillow_rs::TransformData::Mesh(mesh)));
    }
    if let Ok(mesh) = value.extract::<Vec<Vec<Vec<f64>>>>() {
        return Ok(Some(pillow_rs::TransformData::RawMesh(mesh)));
    }
    if value.is_instance_of::<PyDict>() {
        return Ok(Some(pillow_rs::TransformData::Mapping));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(Some(pillow_rs::TransformData::Text(value)));
    }
    Ok(Some(pillow_rs::TransformData::Invalid(
        value.get_type().name()?.to_string(),
    )))
}

pub(crate) fn palette_transform_fill_type_error() -> PyErr {
    PyTypeError::new_err("color must be int or single-element tuple")
}

pub(crate) fn palette_transform_integer_from_python(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    value.extract::<i64>().map_err(|error| {
        if error.is_instance_of::<PyOverflowError>(value.py()) {
            PyOverflowError::new_err("int too big to convert")
        } else {
            error
        }
    })
}

pub(crate) fn palette_transform_sequence_from_python<'py>(
    values: impl IntoIterator<Item = Bound<'py, PyAny>>,
    len: usize,
    allow_single: bool,
) -> PyResult<pillow_rs::TransformFill> {
    let mut values = values.into_iter();
    if allow_single && len == 1 {
        let Some(value) = values.next() else {
            return Err(palette_transform_fill_type_error());
        };
        if !value.is_instance_of::<PyInt>() {
            return Err(palette_transform_fill_type_error());
        }
        return palette_transform_integer_from_python(&value).map(pillow_rs::TransformFill::Scalar);
    }
    if !matches!(len, 3 | 4) {
        return Err(palette_transform_fill_type_error());
    }

    let (Some(first), Some(second), Some(third)) = (values.next(), values.next(), values.next())
    else {
        return Err(palette_transform_fill_type_error());
    };
    let alpha = if len == 4 {
        let Some(value) = values.next() else {
            return Err(palette_transform_fill_type_error());
        };
        Some(value)
    } else {
        None
    };
    if !first.is_instance_of::<PyInt>()
        || !second.is_instance_of::<PyInt>()
        || !third.is_instance_of::<PyInt>()
        || alpha
            .as_ref()
            .is_some_and(|value| !value.is_instance_of::<PyInt>())
    {
        return Err(palette_transform_fill_type_error());
    }
    if alpha
        .as_ref()
        .is_some_and(|value| value.extract::<i64>().ok() != Some(i64::from(u8::MAX)))
    {
        return Err(PyValueError::new_err(
            "cannot add non-opaque RGBA color to RGB palette",
        ));
    }

    let mut components = Vec::with_capacity(len);
    for value in [first, second, third] {
        components.push(
            value
                .extract::<i64>()
                .map_err(|_| PyValueError::new_err("bytes must be in range(0, 256)"))?,
        );
    }
    if len == 4 {
        components.push(i64::from(u8::MAX));
    }
    Ok(pillow_rs::TransformFill::Components(components))
}

pub(crate) fn palette_transform_fill_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::TransformFill> {
    if value.is_instance_of::<PyInt>() {
        return palette_transform_integer_from_python(value).map(pillow_rs::TransformFill::Scalar);
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(pillow_rs::TransformFill::Name(value));
    }
    if let Ok(values) = value.cast::<PyTuple>() {
        return palette_transform_sequence_from_python(values.iter(), values.len(), true);
    }
    if let Ok(values) = value.cast::<PyList>() {
        return palette_transform_sequence_from_python(values.iter(), values.len(), false);
    }
    Err(palette_transform_fill_type_error())
}

pub(crate) fn transform_fill_from_python(
    value: Option<&Bound<'_, PyAny>>,
    mode: &str,
) -> PyResult<Option<pillow_rs::TransformFill>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if mode == "P" {
        return palette_transform_fill_from_python(value).map(Some);
    }
    if let Ok(value) = value.extract::<i64>() {
        return Ok(Some(pillow_rs::TransformFill::Scalar(value)));
    }
    // Pillow's mode-F transform accepts a scalar real fill. Keep scalar floats
    // distinct from float tuples so the core can enforce the mode-specific
    // scalar-versus-sequence contract; byte and integer modes still report
    // their native type error.
    if let Ok(value) = value.extract::<f64>() {
        return Ok(Some(pillow_rs::TransformFill::FloatingScalar(value)));
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(Some(pillow_rs::TransformFill::Name(value)));
    }
    if let Ok(value) = value.extract::<Vec<i64>>() {
        return Ok(Some(pillow_rs::TransformFill::Components(value)));
    }
    if let Ok(value) = value.extract::<Vec<f64>>() {
        return Ok(Some(pillow_rs::TransformFill::FloatingComponents(value)));
    }
    Ok(Some(pillow_rs::TransformFill::Invalid))
}

pub(crate) fn reduce_factor_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ReduceFactor> {
    if let Ok(value) = value.extract::<i64>() {
        return Ok(pillow_rs::ReduceFactor::Scalar(value));
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return Ok(pillow_rs::ReduceFactor::Sequence(values));
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return Ok(pillow_rs::ReduceFactor::FloatingSequence(values));
    }
    Ok(pillow_rs::ReduceFactor::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

pub(crate) fn reduce_box_from_python(value: &Bound<'_, PyAny>) -> PyResult<pillow_rs::ReduceBox> {
    let value = match value.extract::<Vec<i64>>() {
        Ok(value) => pillow_rs::ReduceBox::Sequence(value),
        Err(_) => pillow_rs::ReduceBox::InvalidType(value.get_type().name()?.to_string()),
    };
    Ok(value)
}

pub(crate) fn centering_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::CenteringInput {
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::CenteringInput::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return pillow_rs::CenteringInput::Values(values);
    }
    pillow_rs::CenteringInput::Invalid
}
