// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::redundant_clone)]

use pillow_rs::Image as RsImage;
use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyUserWarning, PyValueError};
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pymodule;
use pyo3::types::PyAnyMethods;
use pyo3::types::PyBool;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyCapsule;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFloat;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyModuleMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use pyo3::types::PyType;
use pyo3::types::PyTypeMethods;
use std::borrow::Cow;
use std::path::PathBuf;

mod backend;
mod draw;
mod errors;
mod font;
mod getcolors;
mod gpu_batch;
mod image;
mod image_chops;
mod image_color;
mod image_filter;
mod image_ops;
mod putdata;
mod save_options;
mod telemetry;
pub(crate) use errors::map_error;
#[cfg(test)]
use font::PilFontLoadCache;
pub(crate) use image::PyImage;
pub(crate) use image_chops::chops_offset_from_python;

// Pillow's custom exception for images exceeding its decompression-bomb limit.
#[cfg(not(feature = "parallel-wheel"))]
pyo3::create_exception!(_core, DecompressionBombError, pyo3::exceptions::PyException);
#[cfg(feature = "parallel-wheel")]
pyo3::create_exception!(
    _core_parallel,
    DecompressionBombError,
    pyo3::exceptions::PyException
);

fn filter_size_from_python(size: i64, negative_is_identity: bool) -> PyResult<u32> {
    if size < 0 {
        if negative_is_identity {
            return Ok(0);
        }
        return Err(PyValueError::new_err("bad kernel size"));
    }
    u32::try_from(size)
        .map_err(|_| PyOverflowError::new_err("Python int too large to convert to C ssize_t"))
}

fn filter_rank_from_python(rank: i64) -> PyResult<u32> {
    if rank < 0 {
        return Err(PyValueError::new_err("bad rank value"));
    }
    u32::try_from(rank)
        .map_err(|_| PyOverflowError::new_err("Python int too large to convert to C ssize_t"))
}

fn blur_radius_pair_from_python(
    radius: Option<&Bound<'_, PyAny>>,
    default: f64,
) -> PyResult<(f64, f64)> {
    match radius {
        None => Ok((default, default)),
        Some(value) => {
            if let Ok(radius) = value.extract::<f64>() {
                return Ok((radius, radius));
            }
            let values = value.extract::<Vec<f64>>().map_err(|_| {
                PyTypeError::new_err("radius must be a number or a sequence of two numbers")
            })?;
            if values.len() != 2 {
                return Err(PyTypeError::new_err(format!(
                    "argument 1 must be sequence of length 2, not {}",
                    values.len()
                )));
            }
            Ok((values[0], values[1]))
        }
    }
}

fn host_path_from_python(value: &Bound<'_, PyAny>) -> PyResult<Option<PathBuf>> {
    if let Ok(path) = value.extract::<PathBuf>() {
        return Ok(Some(path));
    }
    let Ok(bytes) = value.cast::<PyBytes>() else {
        return Ok(None);
    };
    if bytes.as_bytes().contains(&0) {
        return Err(PyValueError::new_err("embedded null byte"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(Some(std::ffi::OsStr::from_bytes(bytes.as_bytes()).into()))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.as_bytes().to_vec())
            .map(PathBuf::from)
            .map(Some)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }
}

fn map_open_path_error(
    py: Python<'_>,
    original: &Bound<'_, PyAny>,
    path: &std::path::Path,
    error: std::io::Error,
) -> PyErr {
    if error.kind() == std::io::ErrorKind::NotFound {
        // Pillow keeps a bytes path as a bytes object in the public OSError
        // tuple. Preserve that host representation while Rust owns the actual
        // filesystem lookup.
        let filename: Py<PyAny> = if let Ok(bytes) = original.cast::<PyBytes>() {
            PyBytes::new(py, bytes.as_bytes()).into()
        } else {
            PyString::new(py, &path.to_string_lossy()).into()
        };
        return pyo3::exceptions::PyFileNotFoundError::new_err((
            error.raw_os_error().unwrap_or(2),
            "No such file or directory",
            filename,
        ));
    }
    if error.kind() == std::io::ErrorKind::IsADirectory {
        return pyo3::exceptions::PyIsADirectoryError::new_err((
            error.raw_os_error().unwrap_or(21),
            "Is a directory",
            path.to_string_lossy().to_string(),
        ));
    }
    map_error(error.into())
}

fn resample_input_from_python(
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

fn rotate_resample_input_from_python(
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

fn rotate_expand_input_from_python(
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

fn rotate_point_input_from_python(
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

fn imageops_color_from_python(value: Option<&Bound<'_, PyAny>>) -> pillow_rs::ImageOpsColor {
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

fn convert_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PythonConvertModeInput> {
    match value {
        None => Ok(pillow_rs::PythonConvertModeInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::PythonConvertModeInput::Name(value)),
            Err(_) => Ok(pillow_rs::PythonConvertModeInput::Invalid(
                value.get_type().name()?.to_string(),
            )),
        },
    }
}

fn convert_palette_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PythonConvertPaletteInput> {
    match value {
        None => Ok(pillow_rs::PythonConvertPaletteInput::None),
        Some(value) => match value.extract::<String>() {
            Ok(value) => Ok(pillow_rs::PythonConvertPaletteInput::Name(value)),
            Err(_) => Ok(pillow_rs::PythonConvertPaletteInput::Invalid(
                value.get_type().name()?.to_string(),
            )),
        },
    }
}

fn transform_data_from_python(
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

fn transform_fill_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<pillow_rs::TransformFill>> {
    let Some(value) = value else {
        return Ok(None);
    };
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

fn reduce_factor_from_python(value: &Bound<'_, PyAny>) -> PyResult<pillow_rs::ReduceFactor> {
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

fn reduce_box_from_python(value: &Bound<'_, PyAny>) -> PyResult<pillow_rs::ReduceBox> {
    let value = match value.extract::<Vec<i64>>() {
        Ok(value) => pillow_rs::ReduceBox::Sequence(value),
        Err(_) => pillow_rs::ReduceBox::InvalidType(value.get_type().name()?.to_string()),
    };
    Ok(value)
}

fn centering_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::CenteringInput {
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::CenteringInput::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return pillow_rs::CenteringInput::Values(values);
    }
    pillow_rs::CenteringInput::Invalid
}

fn image_from_python(value: &Bound<'_, PyAny>) -> Option<RsImage> {
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
fn merge_inputs_from_python(values: &Bound<'_, PyAny>) -> PyResult<Vec<pillow_rs::MergeInput>> {
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

fn paste_source_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::PythonPasteSource {
    if let Some(image) = image_from_python(value) {
        return pillow_rs::PythonPasteSource::Image(Box::new(image));
    }
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::PythonPasteSource::Scalar(value);
    }
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::PythonPasteSource::Float(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::PythonPasteSource::Components(values);
    }
    if let Ok(value) = value.extract::<String>() {
        return pillow_rs::PythonPasteSource::String(value);
    }
    pillow_rs::PythonPasteSource::Invalid
}

fn paste_box_from_python(value: Option<&Bound<'_, PyAny>>) -> PyResult<pillow_rs::PythonPasteBox> {
    let Some(value) = value else {
        return Ok(pillow_rs::PythonPasteBox::None);
    };
    if let Some(image) = image_from_python(value) {
        return Ok(pillow_rs::PythonPasteBox::Image(Box::new(image)));
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return Ok(pillow_rs::PythonPasteBox::Values(values));
    }
    Ok(pillow_rs::PythonPasteBox::Invalid {
        length: value.len().ok(),
        type_name: value.get_type().name()?.to_string(),
    })
}

fn paste_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PythonPasteMask> {
    let Some(value) = value else {
        return Ok(pillow_rs::PythonPasteMask::None);
    };
    if let Some(image) = image_from_python(value) {
        return Ok(pillow_rs::PythonPasteMask::Image(Box::new(image)));
    }
    Ok(pillow_rs::PythonPasteMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

fn open_mode_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PythonOpenModeInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::PythonOpenModeInput::None);
    };
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::PythonOpenModeInput::Name(name));
    }
    Ok(pillow_rs::PythonOpenModeInput::Invalid(
        value.str()?.to_string(),
    ))
}

fn open_formats_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::PythonOpenFormatsInput> {
    let Some(value) = value else {
        return Ok(pillow_rs::PythonOpenFormatsInput::None);
    };
    if value.cast::<PyList>().is_err() && value.cast::<PyTuple>().is_err() {
        return Ok(pillow_rs::PythonOpenFormatsInput::Invalid(
            value.get_type().name()?.to_string(),
        ));
    }
    match value.extract::<Vec<String>>() {
        Ok(names) => Ok(pillow_rs::PythonOpenFormatsInput::Names(names)),
        Err(_) => Ok(pillow_rs::PythonOpenFormatsInput::Invalid(
            value.get_type().name()?.to_string(),
        )),
    }
}

fn imageops_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ImageOpsMask> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageOpsMask::None);
    };
    if let Some(mask) = image_from_python(value) {
        return Ok(pillow_rs::ImageOpsMask::Image(Box::new(mask)));
    }
    Ok(pillow_rs::ImageOpsMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

fn image_analysis_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ImageAnalysisMask> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageAnalysisMask::None);
    };
    if !value.is_truthy()? {
        return Ok(pillow_rs::ImageAnalysisMask::None);
    };
    if let Some(mask) = image_from_python(value) {
        return Ok(pillow_rs::ImageAnalysisMask::Image(Box::new(mask)));
    }
    Ok(pillow_rs::ImageAnalysisMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

fn stat_result_to_python(result: &pillow_rs::StatResult) -> PyResult<Py<PyAny>> {
    use pillow_rs::StatValue;

    Python::attach(|py| {
        let dict = pyo3::types::PyDict::new(py);
        macro_rules! set {
            ($key:expr, $field:ident) => {
                let value = match &result.$field {
                    StatValue::Int(value) => value.into_py_any(py)?,
                    StatValue::Float(value) => value.into_py_any(py)?,
                    StatValue::IntList(value) => value.into_py_any(py)?,
                    StatValue::FloatList(value) => value.into_py_any(py)?,
                    StatValue::ExtremaSingle(value) => value.into_py_any(py)?,
                    StatValue::ExtremaList(value) => value.into_py_any(py)?,
                };
                dict.set_item($key, value)?;
            };
        }
        set!("count", count);
        set!("sum", sum);
        set!("sum2", sum2);
        set!("mean", mean);
        set!("median", median);
        set!("rms", rms);
        set!("var", var);
        set!("stddev", stddev);
        set!("extrema", extrema);
        Ok(dict.into_py_any(py)?)
    })
}

fn putpixel_value_from_python(
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
    let single_band = matches!(mode, "1" | "L" | "P" | "I") || mode.starts_with("I;16");
    // Only P/PA's public palette wrapper accepts RGB(A) lists. Ordinary
    // getink accepts tuples, and unwraps a singleton before numeric coercion.
    let palette_list = matches!(mode, "P" | "PA")
        && value.is_instance_of::<PyList>()
        && matches!(value.len()?, 3 | 4);
    let length = if palette_list {
        Some(value.len()?)
    } else {
        tuple.map(|t| t.len())
    };
    let palette_color = matches!(mode, "P" | "PA") && matches!(length, Some(3 | 4));
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

fn exact_rgb_tuple_from_python(value: &Bound<'_, PyAny>) -> Option<[u8; 3]> {
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

fn new_color_input_from_python(color: Option<&Bound<'_, PyAny>>) -> pillow_rs::PythonNewColorInput {
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
    pillow_rs::PythonNewColorInput::from_parts(
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

fn putpixel_coordinates_from_python(xy: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
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

fn transpose_input_from_python(method: &Bound<'_, PyAny>) -> PyResult<pillow_rs::TransposeInput> {
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

#[pymodule]
// Only the companion wheel uses a new init symbol; `--features parallel`
// source/benchmark builds deliberately retain the ordinary `_core` name.
#[cfg_attr(feature = "parallel-wheel", pyo3(name = "_core_parallel"))]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add(
        "DecompressionBombError",
        m.py().get_type::<DecompressionBombError>(),
    )?;
    image::register_image_class(m)?;
    gpu_batch::register(m)?;
    image::register_adapter_classes(m)?;
    draw::register_classes(m)?;
    font::register_classes(m)?;
    font::register_functions(m)?;
    image_ops::register(m)?;
    image_chops::register(m)?;
    image_color::register(m)?;
    image::register_analysis(m)?;
    image::register_functions(m)?;
    backend::register(m)?;
    image_filter::register(m)?;
    image::register_utilities(m)?;
    Ok(())
}

#[cfg(test)]
mod pilfont_load_cache_tests {
    use super::PilFontLoadCache;

    #[test]
    fn cache_requires_exact_metrics_and_bitmap_bytes() {
        let metrics = b"pilfont metrics";
        let bitmap = b"glyph bitmap";
        let font = pillow_rs::PilFont::load_default().expect("embedded default font is valid");
        let mut cache = PilFontLoadCache::default();

        cache.insert(metrics, bitmap, &font);

        assert!(cache.get(metrics, bitmap).is_some());
        assert!(cache.get(b"changed metrics", bitmap).is_none());
        assert!(cache.get(metrics, b"changed bitmap").is_none());
    }
}
