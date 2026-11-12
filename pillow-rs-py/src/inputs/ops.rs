use super::{
    Bound, Py, PyAny, PyDict, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyString, PyTuple,
    PyTypeError, PyValueError, Python, c_int_from_python,
};
use pyo3::types::{
    PyAnyMethods, PyBytes, PyFloat, PyListMethods, PySequence, PySequenceMethods, PyTupleMethods,
    PyTypeMethods,
};

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
        pillow_rs::ResampleFilter::Mks2013 => 6,
        pillow_rs::ResampleFilter::Mks2021 => 7,
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

pub(crate) fn nonnegative_output_size_items_from_python<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    if value.cast::<PyTuple>().is_err() && value.cast::<PyList>().is_err() {
        return Err(PyValueError::new_err("Size must be a list or tuple"));
    }
    if value.len()? != 2 {
        return Err(PyValueError::new_err("Size must be a sequence of length 2"));
    }
    let width = value.get_item(0)?;
    if width.lt(0)? {
        return Err(PyValueError::new_err("Width and height must be >= 0"));
    }
    let height = value.get_item(1)?;
    if height.lt(0)? {
        return Err(PyValueError::new_err("Width and height must be >= 0"));
    }
    Ok((width, height))
}

pub(crate) fn rotate_fill_preflight_from_python(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<(Option<pillow_rs::ImageOpsColor>, Option<Py<PyAny>>)> {
    let Some(value) = value else {
        return Ok((None, None));
    };
    if value.cast::<PyString>().is_ok() {
        return rotate_fill_from_python(Some(value), mode).map(|fill| (Some(fill), None));
    }
    let (fill, deferred) = palette_image_new_fill_preflight_from_python(py, value, mode)?;
    Ok((fill.map(imageops_color_from_transform_fill), deferred))
}

pub(crate) fn rotate_fill_from_python(
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::ImageOpsColor> {
    if mode == pillow_rs::Mode::ImagePalette8 {
        let Some(value) = value else {
            return Ok(pillow_rs::ImageOpsColor::None);
        };
        if value.cast::<PyTuple>().is_ok() {
            let tuple = value.cast::<PyTuple>()?;
            return transform_tuple_fill_from_python(tuple, mode)
                .map(imageops_color_from_transform_fill);
        }
        if value.cast::<PyList>().is_ok() {
            return Ok(pillow_rs::ImageOpsColor::InvalidList);
        }
        return transform_scalar_fill_from_python(value, mode, true)
            .map(imageops_color_from_transform_fill);
    }
    Ok(transform_fill_from_python(value, mode)?.map_or(
        pillow_rs::ImageOpsColor::None,
        imageops_color_from_transform_fill,
    ))
}

pub(crate) fn transform_fill_preflight_from_python(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<(Option<pillow_rs::TransformFill>, Option<Py<PyAny>>)> {
    let Some(value) = value else {
        return Ok((None, None));
    };
    if value.cast::<PyString>().is_ok() {
        return transform_fill_from_python(Some(value), mode).map(|fill| (fill, None));
    }
    palette_image_new_fill_preflight_from_python(py, value, mode)
}

fn palette_image_new_fill_preflight_from_python(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
) -> PyResult<(Option<pillow_rs::TransformFill>, Option<Py<PyAny>>)> {
    if mode != pillow_rs::Mode::ImagePalette8
        || (value.cast::<PyTuple>().is_err() && value.cast::<PyList>().is_err())
    {
        return Ok((None, None));
    }

    // Pillow's Image.new accepts three/four-component integer lists as P
    // colors, after checking each item and materializing tuple(color).
    let mut all_integers = true;
    for item in value.try_iter()? {
        if !item?.is_instance_of::<PyInt>() {
            all_integers = false;
            break;
        }
    }
    if !all_integers {
        return Ok((None, None));
    }

    let color = py.import("builtins")?.getattr("tuple")?.call1((value,))?;
    if !matches!(color.len()?, 3 | 4) {
        // Pillow keeps the original list when its materialized tuple does not
        // select the temporary RGB(A)-palette path; the native fill parser
        // then reports the list's original type.
        return Ok((None, None));
    }
    palette_transform_fill_from_python(&color).map(|fill| (Some(fill), None))
}

pub(crate) fn transform_fill_after_size_from_python(
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<Option<pillow_rs::TransformFill>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if mode == pillow_rs::Mode::ImagePalette8 {
        if value.cast::<PyTuple>().is_ok() {
            let tuple = value.cast::<PyTuple>()?;
            return transform_tuple_fill_from_python(tuple, mode).map(Some);
        }
        if value.cast::<PyList>().is_ok() {
            return Ok(Some(pillow_rs::TransformFill::InvalidList));
        }
        return transform_scalar_fill_from_python(value, mode, true).map(Some);
    }
    transform_fill_from_python(Some(value), mode)
}

fn imageops_color_from_transform_fill(fill: pillow_rs::TransformFill) -> pillow_rs::ImageOpsColor {
    match fill {
        pillow_rs::TransformFill::Scalar(value) => pillow_rs::ImageOpsColor::Scalar(value),
        pillow_rs::TransformFill::Components(values) => {
            pillow_rs::ImageOpsColor::Components(values)
        }
        pillow_rs::TransformFill::PaletteComponents(values) => {
            pillow_rs::ImageOpsColor::TupleNumericComponents(values)
        }
        pillow_rs::TransformFill::Name(value) => pillow_rs::ImageOpsColor::Name(value),
        pillow_rs::TransformFill::FloatingComponents(values) => {
            pillow_rs::ImageOpsColor::FloatingComponents(values)
        }
        pillow_rs::TransformFill::FloatingScalar(value) => {
            pillow_rs::ImageOpsColor::FloatingScalar(value)
        }
        pillow_rs::TransformFill::InvalidList => pillow_rs::ImageOpsColor::InvalidList,
        pillow_rs::TransformFill::Invalid => pillow_rs::ImageOpsColor::Invalid,
    }
}

fn imageops_native_f32_fill_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageOpsColor> {
    if value.is_instance_of::<PyInt>() {
        return match value.extract::<i64>() {
            Ok(value) => Ok(pillow_rs::ImageOpsColor::Scalar(value)),
            Err(error) if error.is_instance_of::<PyOverflowError>(value.py()) => value
                .extract::<f64>()
                .map(pillow_rs::ImageOpsColor::FloatingScalar),
            Err(error) => Err(error),
        };
    }
    if value.is_instance_of::<PyFloat>() {
        return value
            .extract::<f64>()
            .map(pillow_rs::ImageOpsColor::FloatingScalar);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return tuple
            .extract::<Vec<f64>>()
            .map(pillow_rs::ImageOpsColor::FloatingComponents);
    }
    if value.cast::<PyList>().is_ok() {
        return Ok(pillow_rs::ImageOpsColor::InvalidList);
    }
    value
        .extract::<f64>()
        .map(pillow_rs::ImageOpsColor::FloatingScalar)
}

/// Adapts an Image.new fill using its exact destination Mode.
///
/// P-mode strings pass through ImageColor, then RGB(A) integer lists and tuples
/// use the temporary palette callback path. Other values retain the
/// destination-specific typed conversion.
pub(crate) fn imageops_fill_from_python(
    py: Python<'_>,
    value: Option<&Bound<'_, PyAny>>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::ImageOpsColor> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageOpsColor::None);
    };
    if value.is_none() {
        return Ok(pillow_rs::ImageOpsColor::None);
    }
    if mode == pillow_rs::Mode::ImagePalette8 && value.cast::<PyString>().is_ok() {
        let color = py
            .import("pillow_rs.imagecolor")?
            .getattr("getcolor")?
            .call1((value, "P"))?;
        return imageops_palette_fill_from_python(py, &color);
    }
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::ImageOpsColor::Name(name));
    }
    if mode == pillow_rs::Mode::ImageF32 {
        return imageops_float_fill_from_python(value);
    }
    if matches!(
        mode,
        pillow_rs::Mode::ImageRgb32F | pillow_rs::Mode::ImageRgba32F
    ) {
        return imageops_native_f32_fill_from_python(value);
    }
    if mode == pillow_rs::Mode::ImagePalette8 {
        return imageops_palette_fill_from_python(py, value);
    }

    if value.is_instance_of::<PyInt>() {
        return value.extract::<i64>().map(pillow_rs::ImageOpsColor::Scalar);
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

    if !imageops_tuple_arity_is_valid(tuple.len(), mode) {
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
    if tuple.len() == 1 && mode.band_count() == 1 && !tuple.get_item(0)?.is_instance_of::<PyInt>() {
        return Ok(pillow_rs::ImageOpsColor::TupleComponents(Vec::new()));
    }
    if tuple.len() == 1 && mode.band_count() >= 3 && !tuple.get_item(0)?.is_instance_of::<PyInt>() {
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

fn imageops_palette_fill_from_python(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageOpsColor> {
    if let Some(fill) = imageops_temporary_palette_fill_from_python(py, value)? {
        return Ok(fill);
    }
    if value.is_instance_of::<PyInt>() {
        return value.extract::<i64>().map(pillow_rs::ImageOpsColor::Scalar);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        if tuple.len() == 1 {
            let component = tuple.get_item(0)?;
            if component.is_instance_of::<PyInt>() {
                return component
                    .extract::<i64>()
                    .map(pillow_rs::ImageOpsColor::Scalar);
            }
        }
    }
    // Pillow's one-band P fill parser accepts only a PyLong or a one-element
    // tuple containing a PyLong once Image.new's temporary RGB route is over.
    Err(PyTypeError::new_err(
        "color must be int or single-element tuple",
    ))
}

fn imageops_temporary_palette_fill_from_python(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Option<pillow_rs::ImageOpsColor>> {
    if value.cast::<PyList>().is_err() && value.cast::<PyTuple>().is_err() {
        return Ok(None);
    }
    for component in value.try_iter()? {
        if !component?.is_instance_of::<PyInt>() {
            return Ok(None);
        }
    }
    let tuple = py.import("builtins")?.getattr("tuple")?.call1((value,))?;
    if !matches!(tuple.len()?, 3 | 4) {
        return Ok(None);
    }
    let palette_module = py.import("pillow_rs.imagepalette")?;
    let palette = palette_module.getattr("ImagePalette")?.call0()?;
    let index = palette
        .call_method1("getcolor", (&tuple,))?
        .extract::<u8>()?;
    let palette = palette.getattr("palette")?.extract::<Vec<u8>>()?;
    Ok(Some(pillow_rs::ImageOpsColor::PaletteFill {
        index,
        palette,
    }))
}

fn imageops_tuple_arity_is_valid(len: usize, mode: pillow_rs::Mode) -> bool {
    match mode.band_count() {
        1 => len == 1,
        2 => matches!(len, 1 | 2),
        _ => matches!(len, 1 | 3 | 4),
    }
}

fn imageops_float_fill_from_python(value: &Bound<'_, PyAny>) -> PyResult<pillow_rs::ImageOpsColor> {
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
    let magnitude = int_type.getattr("__abs__").ok()?.call1((component,)).ok()?;
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

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn convert_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ConvertModeInput> {
    match value {
        None => Ok(pillow_rs::ConvertModeInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::Mode::from_pillow_name(&value).map_or_else(
                || pillow_rs::ConvertModeInput::InvalidName(value),
                pillow_rs::ConvertModeInput::Mode,
            )),
            Err(_) => Ok(pillow_rs::ConvertModeInput::InvalidType(
                value.get_type().name()?.to_string(),
            )),
        },
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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

fn long_long_from_python(value: &Bound<'_, PyAny>) -> PyResult<i64> {
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
        return long_long_from_python(&value).map(pillow_rs::TransformFill::Scalar);
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
        return long_long_from_python(value).map(pillow_rs::TransformFill::Scalar);
    }
    if let Ok(value) = value.extract::<String>() {
        return Ok(pillow_rs::TransformFill::Name(value));
    }
    if let Ok(values) = value.cast::<PyTuple>() {
        if let Some(fill) = palette_transform_integer_color_from_python(values.iter(), values.len())
        {
            return Ok(fill);
        }
        return palette_transform_sequence_from_python(values.iter(), values.len(), true);
    }
    if let Ok(values) = value.cast::<PyList>() {
        if let Some(fill) = palette_transform_integer_color_from_python(values.iter(), values.len())
        {
            return Ok(fill);
        }
        return palette_transform_sequence_from_python(values.iter(), values.len(), false);
    }
    Err(palette_transform_fill_type_error())
}

/// Preserves wide integer lanes for Image.new's temporary P RGB(A) palette.
fn palette_transform_integer_color_from_python<'py>(
    values: impl IntoIterator<Item = Bound<'py, PyAny>>,
    len: usize,
) -> Option<pillow_rs::TransformFill> {
    if !matches!(len, 3 | 4) {
        return None;
    }
    let components = values
        .into_iter()
        .map(|value| {
            value
                .is_instance_of::<PyInt>()
                .then(|| palette_integer_component_from_python(&value))
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(pillow_rs::TransformFill::PaletteComponents(components))
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
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return transform_tuple_fill_from_python(tuple, mode).map(Some);
    }
    if value.cast::<PyList>().is_ok() {
        return Ok(Some(pillow_rs::TransformFill::InvalidList));
    }
    transform_scalar_fill_from_python(value, mode, true).map(Some)
}

fn transform_tuple_fill_from_python(
    tuple: &Bound<'_, PyTuple>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::TransformFill> {
    // Pillow's getink unwraps singleton tuples before selecting its scalar or
    // tuple parser, including the resulting mode-specific errors.
    if tuple.len() == 1 {
        return transform_singleton_tuple_fill_from_python(&tuple.get_item(0)?, mode);
    }
    if mode == pillow_rs::Mode::ImageF32 {
        return Ok(pillow_rs::TransformFill::FloatingComponents(Vec::new()));
    }
    if !transform_tuple_arity_is_valid(tuple.len(), mode) {
        return Ok(pillow_rs::TransformFill::Components(Vec::new()));
    }

    // Pillow parses its first component as C long long and later components
    // as C int; keep the same left-to-right overflow and __index__ behavior.
    let mut components = Vec::with_capacity(tuple.len());
    for (index, value) in tuple.iter().enumerate() {
        let component = if index == 0 {
            long_long_from_python(&value)?
        } else {
            i64::from(c_int_from_python(&value)?)
        };
        components.push(component);
    }
    Ok(pillow_rs::TransformFill::Components(components))
}

fn transform_singleton_tuple_fill_from_python(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::TransformFill> {
    if mode == pillow_rs::Mode::ImageF32 {
        return transform_scalar_fill_from_python(value, mode, false);
    }
    if value.is_instance_of::<PyInt>() {
        return long_long_from_python(value).map(pillow_rs::TransformFill::Scalar);
    }
    match mode.band_count() {
        1 => Ok(pillow_rs::TransformFill::Invalid),
        2 => Err(pyo3::exceptions::PySystemError::new_err(
            "new style getargs format but argument is not a tuple",
        )),
        _ => Ok(pillow_rs::TransformFill::Components(Vec::new())),
    }
}

fn transform_tuple_arity_is_valid(len: usize, mode: pillow_rs::Mode) -> bool {
    match mode.band_count() {
        1 => false,
        2 => len == 2,
        _ => matches!(len, 3 | 4),
    }
}

fn transform_scalar_fill_from_python(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
    allow_name: bool,
) -> PyResult<pillow_rs::TransformFill> {
    if allow_name {
        if let Ok(value) = value.extract::<String>() {
            return Ok(pillow_rs::TransformFill::Name(value));
        }
    }
    if mode == pillow_rs::Mode::ImageF32 {
        return value
            .extract::<f64>()
            .map(pillow_rs::TransformFill::FloatingScalar);
    }
    if value.is_instance_of::<PyInt>() {
        return long_long_from_python(value).map(pillow_rs::TransformFill::Scalar);
    }
    if value.is_instance_of::<PyFloat>() {
        return value
            .extract::<f64>()
            .map(pillow_rs::TransformFill::FloatingScalar);
    }
    Ok(pillow_rs::TransformFill::Invalid)
}

enum ReduceSequence {
    NotSequence,
    WrongLength { length: usize },
    Items(Vec<i64>),
}

fn reduce_sequence_items(
    value: &Bound<'_, PyAny>,
    expected_length: usize,
    argument_index: usize,
) -> PyResult<ReduceSequence> {
    let Ok(sequence) = value.cast::<PySequence>() else {
        return Ok(ReduceSequence::NotSequence);
    };
    if value.cast::<PyBytes>().is_ok() {
        return Ok(ReduceSequence::NotSequence);
    }

    let length = sequence.len()?;
    if length != expected_length {
        return Ok(ReduceSequence::WrongLength { length });
    }

    let mut items = Vec::with_capacity(expected_length);
    for index in 0..expected_length {
        // Pillow's low-level Reduce parser checks length, then gets and
        // converts each item in order. It clears a failed item lookup and
        // reports the indexed retrieval TypeError.
        let item = match sequence.get_item(index) {
            Ok(item) => item,
            Err(_sequence_error) => {
                return Err(PyTypeError::new_err(format!(
                    "argument {argument_index}, item {index} is not retrievable"
                )));
            }
        };
        items.push(c_int_from_python(&item).map(i64::from)?);
    }
    Ok(ReduceSequence::Items(items))
}

pub(crate) fn reduce_factor_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ReduceFactor> {
    match reduce_sequence_items(value, 2, 1)? {
        ReduceSequence::WrongLength { length } => Err(PyTypeError::new_err(format!(
            "argument 1 must be sequence of length 2, not {length}"
        ))),
        ReduceSequence::Items(items) => Ok(pillow_rs::ReduceFactor::Sequence(items)),
        ReduceSequence::NotSequence => Ok(pillow_rs::ReduceFactor::Scalar(i64::from(
            c_int_from_python(value)?,
        ))),
    }
}

pub(crate) fn reduce_box_from_python(value: &Bound<'_, PyAny>) -> PyResult<pillow_rs::ReduceBox> {
    match reduce_sequence_items(value, 4, 2)? {
        ReduceSequence::WrongLength { length } => {
            Ok(pillow_rs::ReduceBox::SequenceLength { length })
        }
        ReduceSequence::Items(items) => Ok(pillow_rs::ReduceBox::Sequence(items)),
        ReduceSequence::NotSequence => Ok(pillow_rs::ReduceBox::InvalidType(
            value.get_type().name()?.to_string(),
        )),
    }
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
