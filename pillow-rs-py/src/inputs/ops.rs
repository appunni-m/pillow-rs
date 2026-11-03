use super::{
    Bound, PyAny, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyTuple, PyTypeError,
    PyValueError, Python,
};
use pyo3::types::{PyAnyMethods, PyBool, PyFloat, PyListMethods, PyTupleMethods, PyTypeMethods};

pub(crate) fn autocontrast_cutoff_counts_from_python(
    py: Python<'_>,
    histogram: &[u32],
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<(i128, i128)> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok((0, 0));
    };
    let total = histogram
        .iter()
        .map(|&count| u64::from(count))
        .sum::<u64>();
    let total = i64::try_from(total)
        .map_err(|_| PyOverflowError::new_err("histogram count exceeds Python integer range"))?;
    if value.cast::<PyTuple>().is_ok() {
        let operator = py.import("operator")?;
        let low = operator.getattr("getitem")?.call1((value, 0))?;
        let low = autocontrast_cutoff_count_from_python(py, total, &low)?;
        let high = operator.getattr("getitem")?.call1((value, 1))?;
        let high = autocontrast_cutoff_count_from_python(py, total, &high)?;
        return Ok((low, high));
    }

    let cutoff = autocontrast_cutoff_count_from_python(py, total, value)?;
    Ok((cutoff, cutoff))
}

fn autocontrast_cutoff_count_from_python(
    py: Python<'_>,
    total: i64,
    cutoff: &Bound<'_, PyAny>,
) -> PyResult<i128> {
    let operator = py.import("operator")?;
    let product = operator.getattr("mul")?.call1((total, cutoff))?;
    let percentage = operator
        .getattr("floordiv")?
        .call1((product, 100))?;
    let count = py.import("builtins")?.getattr("int")?.call1((percentage,))?;
    if operator
        .getattr("lt")?
        .call1((count.clone(), 0))?
        .extract::<bool>()?
    {
        // Only the sign of a negative trim can affect the final LUT: it makes
        // the corresponding edge bin nonempty; its magnitude cannot move the
        // output bounds beyond that edge.
        return Ok(-1);
    }
    if operator
        .getattr("gt")?
        .call1((count.clone(), total))?
        .extract::<bool>()?
    {
        // A trim beyond the whole channel removes every existing bin. One past
        // the total is sufficient even if Python produced a larger int.
        return Ok(i128::from(total) + 1);
    }
    count.extract::<i64>().map(i128::from)
}

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
    // ImageOps callers that validate geometry first must run that core
    // preflight before entering this host-value conversion boundary.
    let display = value.str()?.to_string();
    Ok(Some(pillow_rs::ResampleInput::Name(display)))
}

pub(crate) fn resample_filter_code(filter: pillow_rs::ResampleFilter) -> i32 {
    match filter {
        pillow_rs::ResampleFilter::Nearest => 0,
        pillow_rs::ResampleFilter::Lanczos => 1,
        pillow_rs::ResampleFilter::Bilinear => 2,
        pillow_rs::ResampleFilter::Bicubic => 3,
        pillow_rs::ResampleFilter::Box => 4,
        pillow_rs::ResampleFilter::Hamming => 5,
    }
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
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::ImageOpsColor::FloatingScalar(value);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        if let Ok(values) = tuple.extract::<Vec<i64>>() {
            return pillow_rs::ImageOpsColor::TupleComponents(values);
        }
        if let Ok(values) = tuple.extract::<Vec<f64>>() {
            return pillow_rs::ImageOpsColor::FloatingComponents(values);
        }
        // TODO(pillow-rs#3ptb): retain tuple values for 1/2-component host
        // coercion and custom numeric types; P/PA 3/4 arity faults are handled
        // at the Expand operation bridge.
        return pillow_rs::ImageOpsColor::Invalid;
    }
    if let Ok(list) = value.cast::<PyList>() {
        if let Ok(values) = list.extract::<Vec<i64>>() {
            return pillow_rs::ImageOpsColor::ListComponents(values);
        }
        // TODO(pillow-rs#3ptb): preserve oversized integer-list components for
        // the remaining ImageOps paths; Expand handles them separately below.
        return pillow_rs::ImageOpsColor::InvalidList;
    }
    pillow_rs::ImageOpsColor::Invalid
}

/// Retains built-in mixed and arbitrary-width tuple numbers for P fills.
///
/// Pillow's palette lookup compares the tuple before converting it to bytes,
/// so widening mixed numeric values through `f64` changes both lookup and
/// error ordering.
pub(crate) fn imageops_expand_color_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> pillow_rs::ImageOpsColor {
    if let Some(value) = value {
        if let Some(components) = imageops_expand_tuple_numeric_components(value) {
            return pillow_rs::ImageOpsColor::TupleComponentsWithFallback {
                components,
                fallback: Box::new(imageops_color_from_python(Some(value))),
            };
        }
        if let Some(components) = imageops_expand_list_wide_integer_components(value) {
            return pillow_rs::ImageOpsColor::ListNumericComponents(components);
        }
    }
    imageops_color_from_python(value)
}

/// Preserves wide integer tuples accepted by Pillow's P-mode `Image.new` path.
///
/// Pillow 12.2 only routes all-integer tuples of length three or four through
/// its temporary RGB palette. Mixed numeric tuples continue to the ordinary
/// per-mode conversion, where P rejects them as a single-band fill.
pub(crate) fn imageops_pad_color_from_python(
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::ImageOpsColor> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageOpsColor::None);
    };
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::ImageOpsColor::Name(name));
    }
    if mode == pillow_rs::Mode::ImageF32 {
        return imageops_pad_float_color_from_python(value);
    }
    if matches!(mode, pillow_rs::Mode::ImageRgb32F | pillow_rs::Mode::ImageRgba32F) {
        return Ok(imageops_color_from_python(Some(value)));
    }
    if mode == pillow_rs::Mode::ImagePalette8 {
        return imageops_pad_palette_color_from_python(value);
    }

    if value.is_instance_of::<PyInt>() {
        return value
            .extract::<i64>()
            .map(pillow_rs::ImageOpsColor::Scalar);
    }
    if value.is_instance_of::<PyFloat>() {
        return value
            .extract::<f64>()
            .map(pillow_rs::ImageOpsColor::FloatingScalar);
    }
    if value.cast::<PyList>().is_ok() {
        return Ok(pillow_rs::ImageOpsColor::InvalidList);
    }
    let Ok(tuple) = value.cast::<PyTuple>() else {
        return Err(PyTypeError::new_err(if mode.band_count() == 1 {
            "color must be int or single-element tuple"
        } else {
            "color must be int or tuple"
        }));
    };

    if !pad_integer_tuple_arity_is_valid(tuple.len(), mode) {
        return Ok(pillow_rs::ImageOpsColor::TupleComponents(Vec::new()));
    }
    if tuple.len() == 1 && mode.band_count() == 2 {
        let value = tuple.get_item(0)?;
        if !value.is_instance_of::<PyInt>() {
            return Err(pyo3::exceptions::PySystemError::new_err(
                "new style getargs format but argument is not a tuple",
            ));
        }
    }
    if tuple.len() == 1
        && mode.band_count() == 1
        && !tuple.get_item(0)?.is_instance_of::<PyInt>()
    {
        return Ok(pillow_rs::ImageOpsColor::TupleComponents(Vec::new()));
    }
    if tuple.len() == 1
        && mode.band_count() >= 3
        && !tuple.get_item(0)?.is_instance_of::<PyInt>()
    {
        return Ok(pillow_rs::ImageOpsColor::TupleComponents(Vec::new()));
    }

    let mut components = Vec::with_capacity(tuple.len());
    for (index, value) in tuple.iter().enumerate() {
        let component = if index == 0 {
            value.extract::<i64>()?
        } else {
            i64::from(value.extract::<i32>()?)
        };
        components.push(component);
    }
    Ok(pillow_rs::ImageOpsColor::TupleComponents(components))
}

fn imageops_pad_palette_color_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageOpsColor> {
    // TODO(pillow-rs#3ptb): Pillow's Image.new routes three/four-component
    // integer lists and tuples through a temporary RGB ImagePalette. It calls
    // tuple(value) and ImagePalette.getcolor before core fill validation; this
    // bridge currently preserves numeric results but bypasses those host
    // iteration, hash, comparison, and byte-conversion callbacks.
    if value.is_instance_of::<PyInt>() {
        return value
            .extract::<i64>()
            .map(pillow_rs::ImageOpsColor::Scalar);
    }
    if value.is_instance_of::<PyFloat>() {
        return value
            .extract::<f64>()
            .map(pillow_rs::ImageOpsColor::FloatingScalar);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        if !matches!(tuple.len(), 1 | 3 | 4) {
            // P has one logical band. Reject invalid tuple shapes before
            // examining components, preserving Pillow's arity-first fault.
            return Ok(pillow_rs::ImageOpsColor::FloatingComponents(Vec::new()));
        }
        if tuple.len() == 1 {
            let component = tuple.get_item(0)?;
            if component.is_instance_of::<PyInt>() {
                return component
                    .extract::<i64>()
                    .map(pillow_rs::ImageOpsColor::Scalar);
            }
        }
        if let Some(components) = imageops_pad_wide_integer_tuple_components(value) {
            return Ok(pillow_rs::ImageOpsColor::WideTupleComponents {
                components,
                fallback: Box::new(imageops_color_from_python(Some(value))),
            });
        }
        if tuple.iter().all(|component| {
            component.is_instance_of::<PyInt>() || component.is_instance_of::<PyFloat>()
        }) {
            return Ok(imageops_color_from_python(Some(value)));
        }
        return Err(PyTypeError::new_err(
            "color must be int or single-element tuple",
        ));
    }
    if let Ok(list) = value.cast::<PyList>() {
        if let Some(components) = imageops_expand_list_wide_integer_components(value) {
            return Ok(pillow_rs::ImageOpsColor::ListNumericComponents(components));
        }
        if list.iter().all(|component| component.is_instance_of::<PyInt>()) {
            return Ok(imageops_color_from_python(Some(value)));
        }
        return Ok(pillow_rs::ImageOpsColor::InvalidList);
    }
    Err(PyTypeError::new_err(
        "color must be int or single-element tuple",
    ))
}

fn pad_integer_tuple_arity_is_valid(len: usize, mode: pillow_rs::Mode) -> bool {
    match mode.band_count() {
        1 => len == 1,
        2 => matches!(len, 1 | 2),
        _ => matches!(len, 1 | 3 | 4),
    }
}

fn imageops_pad_float_color_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageOpsColor> {
    let scalar = if let Ok(tuple) = value.cast::<PyTuple>() {
        if tuple.len() == 1 {
            tuple.get_item(0)?
        } else {
            return value
                .extract::<f64>()
                .map(pillow_rs::ImageOpsColor::FloatingScalar);
        }
    } else {
        value.clone()
    };
    if value.cast::<PyTuple>().is_ok() {
        scalar
            .extract::<f64>()
            .map(|value| pillow_rs::ImageOpsColor::FloatingComponents(vec![value]))
    } else {
        scalar
            .extract::<f64>()
            .map(pillow_rs::ImageOpsColor::FloatingScalar)
    }
}

fn imageops_pad_wide_integer_tuple_components(
    value: &Bound<'_, PyAny>,
) -> Option<Vec<pillow_rs::PaletteColorComponent>> {
    let components = imageops_expand_tuple_numeric_components(value)?;
    let has_wide_integer = components.iter().any(|component| {
        matches!(
            component,
            pillow_rs::PaletteColorComponent::WideInteger { .. }
        )
    });
    let contains_float = components
        .iter()
        .any(|component| matches!(component, pillow_rs::PaletteColorComponent::Float(_)));
    (matches!(components.len(), 3 | 4) && has_wide_integer && !contains_float)
        .then_some(components)
}

fn imageops_expand_tuple_numeric_components(
    value: &Bound<'_, PyAny>,
) -> Option<Vec<pillow_rs::PaletteColorComponent>> {
    let tuple = value.cast::<PyTuple>().ok()?;
    let mut components = Vec::with_capacity(tuple.len());
    let mut contains_integer = false;
    let mut contains_float = false;
    let mut contains_wide_integer = false;
    for component in tuple.iter() {
        if component.is_instance_of::<PyInt>() {
            contains_integer = true;
            let parsed = palette_integer_component_from_python(&component)?;
            contains_wide_integer |= matches!(
                &parsed,
                pillow_rs::PaletteColorComponent::WideInteger { .. }
            );
            components.push(parsed);
        } else if component.is_instance_of::<PyFloat>() {
            contains_float = true;
            components.push(pillow_rs::PaletteColorComponent::Float(
                component.extract::<f64>().ok()?,
            ));
        } else {
            return None;
        }
    }
    (contains_wide_integer || (contains_integer && contains_float)).then_some(components)
}

fn imageops_expand_list_wide_integer_components(
    value: &Bound<'_, PyAny>,
) -> Option<Vec<pillow_rs::PaletteColorComponent>> {
    let list = value.cast::<PyList>().ok()?;
    let mut components = Vec::with_capacity(list.len());
    let mut contains_wide_integer = false;
    for component in list.iter() {
        if !component.is_instance_of::<PyInt>() {
            return None;
        }
        let parsed = palette_integer_component_from_python(&component)?;
        contains_wide_integer |= matches!(
            &parsed,
            pillow_rs::PaletteColorComponent::WideInteger { .. }
        );
        components.push(parsed);
    }
    contains_wide_integer.then_some(components)
}

/// Converts a Python integer without narrowing values outside `i64`.
pub(crate) fn palette_integer_component_from_python(
    component: &Bound<'_, PyAny>,
) -> Option<pillow_rs::PaletteColorComponent> {
    if let Ok(value) = component.extract::<i64>() {
        return Some(pillow_rs::PaletteColorComponent::Integer(value));
    }
    let int_type = component.py().get_type::<PyInt>();
    let negative = int_type
        .getattr("__lt__")
        .ok()?
        .call1((component, 0))
        .ok()?
        .extract::<bool>()
        .ok()?;
    let magnitude = int_type
        .getattr("__abs__")
        .ok()?
        .call1((component,))
        .ok()?;
    let bits = int_type
        .getattr("bit_length")
        .ok()?
        .call1((&magnitude,))
        .ok()?
        .extract::<usize>()
        .ok()?;
    let length = bits.checked_add(7)? / 8;
    let bytes = if length == 0 {
        Vec::new()
    } else {
        int_type
            .getattr("to_bytes")
            .ok()?
            .call1((&magnitude, length, "big"))
            .ok()?
            .extract::<Vec<u8>>()
            .ok()?
    };
    Some(pillow_rs::PaletteColorComponent::WideInteger {
        negative,
        magnitude: bytes,
    })
}

pub(crate) fn convert_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ConvertModeInput> {
    match value {
        None => Ok(pillow_rs::ConvertModeInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::Mode::from_pillow_name(&value)
                .map(pillow_rs::ConvertModeInput::Mode)
                .unwrap_or_else(|| pillow_rs::ConvertModeInput::InvalidName(value))),
            Err(_) => Ok(pillow_rs::ConvertModeInput::InvalidType(
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
    mode: pillow_rs::Mode,
) -> PyResult<Option<pillow_rs::TransformFill>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if mode == pillow_rs::Mode::ImagePalette8 {
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

pub(crate) fn pad_centering_from_python(
    value: &Bound<'_, PyAny>,
    padded_axes: (bool, bool),
    padding_gaps: (u32, u32),
    paste_size: (u32, u32),
) -> PyResult<pillow_rs::CenteringInput> {
    let zero = value.py().eval("0", None, None)?;
    let (x, y) = if padded_axes.0 {
        let coordinate = value.get_item(0)?;
        (
            pad_centering_component_from_python(&coordinate, padding_gaps.0)?,
            zero.clone(),
        )
    } else if padded_axes.1 {
        let coordinate = value.get_item(1)?;
        (
            zero.clone(),
            pad_centering_component_from_python(&coordinate, padding_gaps.1)?,
        )
    } else {
        (zero.clone(), zero)
    };

    // Image.paste first forms (x + width, y + height) in Python, then the
    // native `(iiii)` parser converts all four coordinates to C ints.
    let operator = value.py().import("operator")?;
    let right = operator.getattr("add")?.call1((x.clone(), paste_size.0))?;
    let lower = operator.getattr("add")?.call1((y.clone(), paste_size.1))?;
    let x = super::images::paste_box_coordinate_from_python(&x)? as i32;
    let y = super::images::paste_box_coordinate_from_python(&y)? as i32;
    super::images::paste_box_coordinate_from_python(&right)?;
    super::images::paste_box_coordinate_from_python(&lower)?;
    Ok(pillow_rs::CenteringInput::PadPosition(x, y))
}

fn pad_centering_component_from_python(
    value: &Bound<'_, PyAny>,
    gap: u32,
) -> PyResult<Bound<'_, PyAny>> {
    if value.is_exact_instance_of::<PyFloat>() {
        let numeric_value = value.extract::<f64>()?;
        let upper = if numeric_value > 1.0 {
            1.0
        } else {
            numeric_value
        };
        let clamped = if upper > 0.0 { upper } else { 0.0 };
        let builtins = value.py().import("builtins")?;
        return Ok(builtins
            .getattr("round")?
            .call1((f64::from(gap) * clamped,))?);
    }
    if value.is_exact_instance_of::<PyInt>() || value.is_exact_instance_of::<PyBool>() {
        let nonpositive = value.call_method1("__le__", (0,))?.extract::<bool>()?;
        let integer = value.py().import("builtins")?.getattr("int")?;
        return Ok(integer.call1((if nonpositive { 0 } else { gap },))?);
    }

    let builtins = value.py().import("builtins")?;
    let clamped_upper = builtins
        .getattr("min")?
        .call1((value.clone(), 1))?;
    let clamped = builtins
        .getattr("max")?
        .call1((0, clamped_upper))?;
    let product = value
        .py()
        .import("operator")?
        .getattr("mul")?
        .call1((gap, clamped.clone()))?;
    let rounded = builtins.getattr("round")?.call1((product,))?;
    Ok(rounded)
}
