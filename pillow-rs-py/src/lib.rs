// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::redundant_clone)]

use pillow_rs::Image as RsImage;
use pillow_rs::PilError;
use pyo3::ToPyObject;
use pyo3::exceptions::{PyTypeError, PyUserWarning, PyValueError};
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyObject;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pyfunction;
use pyo3::pymethods;
use pyo3::pymodule;
use pyo3::types::PyAnyMethods;
use pyo3::types::PyBool;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyCapsule;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyModuleMethods;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyType;
use pyo3::types::PyTypeMethods;
use pyo3::wrap_pyfunction;
use std::ffi::CString;
use std::path::PathBuf;

mod putdata;

// Pillow's custom exception for images exceeding its decompression-bomb limit.
pyo3::create_exception!(_core, DecompressionBombError, pyo3::exceptions::PyException);

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
}

/// Thin host handle for the Rust-owned ImageSequence iterator state.
#[pyclass(name = "Iterator", unsendable)]
pub struct PyImageSequenceIterator {
    image: PyObject,
    state: pillow_rs::ImageSequenceIterator,
}

#[pymethods]
impl PyImageSequenceIterator {
    #[new]
    fn new(im: PyObject, py: Python<'_>) -> PyResult<Self> {
        let bound = im.bind(py);
        if !bound.hasattr("seek")? {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                "im must have seek method",
            ));
        }
        let min_frame = match bound.getattr("_min_frame") {
            Ok(value) => value.extract::<u32>()?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => 0,
            Err(error) => return Err(error),
        };
        Ok(Self {
            image: im,
            state: pillow_rs::ImageSequenceIterator::new(min_frame),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        let frame = self.state.position();
        match self.image.bind(py).call_method1("seek", (frame,)) {
            Ok(_) => {
                self.state.advance();
                Ok(self.image.clone_ref(py))
            }
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyEOFError>(py) => Err(
                pyo3::exceptions::PyStopIteration::new_err("end of sequence"),
            ),
            Err(error) => Err(error),
        }
    }
}

fn host_path_from_python(value: &Bound<'_, PyAny>) -> PyResult<Option<PathBuf>> {
    if let Ok(path) = value.extract::<PathBuf>() {
        return Ok(Some(path));
    }
    let Ok(bytes) = value.downcast::<PyBytes>() else {
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
        let filename: PyObject = if let Ok(bytes) = original.downcast::<PyBytes>() {
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
            if value.is_none() {
                return Ok(pillow_rs::RotateResampleInput::None);
            }
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
    if value.is_none() {
        return Ok(pillow_rs::RotatePointInput::Default);
    }
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
        Some(value) if value.downcast::<PyImage>().is_ok() => {
            Ok(pillow_rs::PythonConvertPaletteInput::Image)
        }
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

fn reduce_box_from_python(value: Option<&Bound<'_, PyAny>>) -> PyResult<pillow_rs::ReduceBox> {
    let Some(value) = value else {
        return Ok(pillow_rs::ReduceBox::Invalid);
    };
    let value = match value.extract::<Vec<i64>>() {
        Ok(value) => pillow_rs::ReduceBox::Sequence(value),
        Err(_) => pillow_rs::ReduceBox::InvalidType(value.get_type().name()?.to_string()),
    };
    Ok(value)
}

fn centering_from_python(value: Option<&Bound<'_, PyAny>>) -> pillow_rs::CenteringInput {
    let Some(value) = value else {
        return pillow_rs::CenteringInput::Default;
    };
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::CenteringInput::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return pillow_rs::CenteringInput::Values(values);
    }
    pillow_rs::CenteringInput::Invalid
}

fn image_from_python(value: &Bound<'_, PyAny>) -> Option<RsImage> {
    if let Ok(image) = value.downcast::<PyImage>() {
        return Some(image.borrow().inner.clone());
    }
    value.getattr("_rust_image").ok().and_then(|inner| {
        inner
            .downcast::<PyImage>()
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
        .iter()?
        .map(|item| {
            let obj = item?;
            Ok(match image_from_python(&obj) {
                Some(image) => pillow_rs::MergeInput::Image(image),
                None => pillow_rs::MergeInput::Invalid(obj.get_type().name()?.to_string()),
            })
        })
        .collect()
}

fn paste_source_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::PythonPasteSource {
    if let Some(image) = image_from_python(value) {
        return pillow_rs::PythonPasteSource::Image(image);
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
        return Ok(pillow_rs::PythonPasteBox::Image(image));
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
        return Ok(pillow_rs::PythonPasteMask::Image(image));
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
    if value.downcast::<PyList>().is_err() && value.downcast::<PyTuple>().is_err() {
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
        return Ok(pillow_rs::ImageOpsMask::Image(mask));
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
        return Ok(pillow_rs::ImageAnalysisMask::Image(mask));
    }
    Ok(pillow_rs::ImageAnalysisMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

#[pyfunction]
fn ops_validate_deform_resample(value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let value = resample_input_from_python(value)?;
    pillow_rs::imageops_validate_deform_resample(value).map_err(map_error)
}

fn stat_result_to_python(result: &pillow_rs::StatResult) -> PyResult<PyObject> {
    use pillow_rs::StatValue;

    Python::with_gil(|py| {
        let dict = pyo3::types::PyDict::new(py);
        macro_rules! set {
            ($key:expr, $field:ident) => {
                let value = match &result.$field {
                    StatValue::Int(value) => value.to_object(py),
                    StatValue::Float(value) => value.to_object(py),
                    StatValue::IntList(value) => value.to_object(py),
                    StatValue::FloatList(value) => value.to_object(py),
                    StatValue::ExtremaSingle(value) => value.to_object(py),
                    StatValue::ExtremaList(value) => value.to_object(py),
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
        Ok(dict.to_object(py))
    })
}

fn putpixel_value_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::PutPixelValue {
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::PutPixelValue::Integer(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::PutPixelValue::Components(values);
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        return pillow_rs::PutPixelValue::FloatComponents(values);
    }
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::PutPixelValue::Float(value);
    }
    pillow_rs::PutPixelValue::Invalid
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        // Default 1x1 RGB image for compatibility
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0)).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        size: (u32, u32),
        color: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let (hex, single, rgb, rgba, la, int32_val, float_val) = if let Some(val) = color {
            (
                val.extract::<String>().ok(),
                val.extract::<u8>().ok(),
                val.extract::<(u8, u8, u8)>().ok(),
                val.extract::<(u8, u8, u8, u8)>().ok(),
                val.extract::<(u8, u8)>().ok(),
                val.extract::<i32>().ok(),
                val.extract::<f64>().ok(),
            )
        } else {
            (None, None, None, None, None, None, None)
        };
        let input = pillow_rs::PythonNewColorInput::from_parts(
            hex,
            single,
            rgb,
            rgba,
            la,
            int32_val,
            float_val,
            color.is_some(),
        );
        let img = RsImage::new_with_input(size.0, size.1, mode, input).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[staticmethod]
    #[pyo3(name = "effect_noise_from_size")]
    fn effect_noise_from_size(size: (u32, u32), sigma: f64) -> PyResult<Self> {
        let image = pillow_rs::image_effect_noise_from_size(size, sigma).map_err(map_error)?;
        Ok(PyImage { inner: image })
    }

    #[classmethod]
    #[pyo3(signature = (fp, mode=None, formats=None))]
    fn open(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        fp: &Bound<'_, PyAny>,
        mode: Option<&Bound<'_, PyAny>>,
        formats: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mode = open_mode_input_from_python(mode)?;
        let formats = open_formats_input_from_python(formats)?;
        pillow_rs::validate_python_open_inputs(mode, formats.clone()).map_err(map_error)?;
        let format_names = match formats {
            pillow_rs::PythonOpenFormatsInput::None => None,
            pillow_rs::PythonOpenFormatsInput::Names(names) => Some(names),
            pillow_rs::PythonOpenFormatsInput::Invalid(_) => {
                unreachable!("validated Image.open formats cannot remain invalid")
            }
        };
        let format_refs = format_names
            .as_deref()
            .map(|names| names.iter().map(String::as_str).collect::<Vec<_>>());
        if let Some(path) = host_path_from_python(fp)? {
            let bytes = py
                .allow_threads(|| std::fs::read(&path))
                .map_err(|error| map_open_path_error(py, fp, &path, error))?;
            let img = py
                .allow_threads(|| RsImage::open_bytes_with_formats(bytes, format_refs.as_deref()))
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        } else {
            let bytes = fp.call_method0("read")?.extract::<Vec<u8>>()?;
            let img = py
                .allow_threads(|| RsImage::open_bytes_with_formats(bytes, format_refs.as_deref()))
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        }
    }

    #[classmethod]
    fn validate_open_source(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(bytes) = fp.downcast::<PyBytes>() {
            pillow_rs::validate_python_open_source_bytes(bytes.as_bytes()).map_err(map_error)?;
        }
        Ok(())
    }

    #[classmethod]
    #[pyo3(signature = (mode=None, formats=None))]
    fn validate_open_inputs(
        _cls: &Bound<'_, PyType>,
        mode: Option<&Bound<'_, PyAny>>,
        formats: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        pillow_rs::validate_python_open_inputs(
            open_mode_input_from_python(mode)?,
            open_formats_input_from_python(formats)?,
        )
        .map_err(map_error)
    }

    fn save(
        &mut self,
        fp: &Bound<'_, PyAny>,
        format: Option<String>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let path = host_path_from_python(fp)?;
        if let Some(path) = path.as_deref() {
            if path.is_dir() {
                return Err(pyo3::exceptions::PyIsADirectoryError::new_err((
                    21,
                    "Is a directory",
                    path.to_string_lossy().to_string(),
                )));
            }
            if path.parent().is_some_and(|parent| !parent.exists()) {
                return Err(pyo3::exceptions::PyFileNotFoundError::new_err((
                    2,
                    "No such file or directory",
                    path.to_string_lossy().to_string(),
                )));
            }
        }
        let extension = path
            .as_deref()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str());
        let format = pillow_rs::Image::resolve_save_format(format.as_deref(), extension)
            .map_err(map_error)?;
        let encoded = py
            .allow_threads(|| self.inner.encode(&format))
            .map_err(map_error)?;
        if let Some(path) = path {
            py.allow_threads(|| std::fs::write(path, &encoded))
                .map_err(|error| map_error(error.into()))
        } else {
            fp.call_method1("write", (PyBytes::new(fp.py(), &encoded),))?;
            Ok(())
        }
    }

    #[pyo3(signature = (size, resample=None, box_coords=None))]
    fn resize(
        &self,
        size: (i64, i64),
        resample: Option<&Bound<'_, PyAny>>,
        box_coords: Option<(i32, i32, i32, i32)>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let resample = resample_input_from_python(resample)?;
        let rs = py
            .allow_threads(|| self.inner.resize(size, resample, box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: Option<(f64, f64, f64, f64)>, py: Python<'_>) -> PyResult<PyImage> {
        let rs = py
            .allow_threads(|| self.inner.crop_float(box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, resample=None, expand=None, center=None, translate=None, fillcolor=None))]
    fn rotate(
        &self,
        angle: f64,
        resample: Option<&Bound<'_, PyAny>>,
        expand: Option<&Bound<'_, PyAny>>,
        center: Option<&Bound<'_, PyAny>>,
        translate: Option<&Bound<'_, PyAny>>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let center = rotate_point_input_from_python(center)?;
        let translate = rotate_point_input_from_python(translate)?;
        let expand = rotate_expand_input_from_python(expand)?;
        let resample = rotate_resample_input_from_python(resample)?;
        let fillcolor = imageops_color_from_python(fillcolor);
        let rs = py
            .allow_threads(|| {
                self.inner
                    .rotate_with_input(angle, resample, expand, center, translate, fillcolor)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let input = transpose_input_from_python(method)?;
        let rs = py
            .allow_threads(|| self.inner.transpose_with_input(input))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode=None, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        &self,
        mode: Option<&Bound<'_, PyAny>>,
        matrix: Option<Vec<f64>>,
        dither: Option<&Bound<'_, PyAny>>,
        palette: Option<&Bound<'_, PyAny>>,
        colors: Option<u32>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let dither = match dither {
            None => pillow_rs::PythonDitherInput::None,
            Some(value) => {
                if let Ok(int_value) = value.extract::<u32>() {
                    pillow_rs::PythonDitherInput::Integer(int_value)
                } else if let Ok(string_value) = value.extract::<String>() {
                    pillow_rs::PythonDitherInput::Name(string_value)
                } else {
                    pillow_rs::PythonDitherInput::Invalid(value.get_type().name()?.to_string())
                }
            }
        };
        let mode = convert_mode_input_from_python(mode)?;
        let palette = convert_palette_input_from_python(palette)?;
        let rs = py
            .allow_threads(|| {
                self.inner
                    .convert_with_input(mode, matrix, dither, palette, colors)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let source = paste_source_from_python(im);
        let box_coords = paste_box_from_python(box_coords)?;
        let mask = paste_mask_from_python(mask)?;
        py.allow_threads(|| self.inner.paste_with_input(source, box_coords, mask))
            .map_err(map_error)
    }

    fn split(&self, py: Python<'_>) -> PyResult<Vec<PyImage>> {
        let bands = py.allow_threads(|| self.inner.split()).map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let filter = if filter_type.is_callable() {
            filter_type.call0()?
        } else {
            filter_type.clone()
        };
        let type_name = filter.get_type().name()?.to_string();
        let validation_name = filter
            .getattr("name")
            .ok()
            .and_then(|value| value.extract::<String>().ok())
            .unwrap_or_else(|| type_name.clone());
        self.inner
            .validate_filter(&validation_name)
            .map_err(map_error)?;
        if !filter.hasattr("_apply")? {
            return Err(PyTypeError::new_err(
                "filter argument should be ImageFilter.Filter instance or class",
            ));
        }
        let image = Py::new(
            py,
            PyImage {
                inner: self.inner.clone(),
            },
        )?;
        let result = filter.call_method1("_apply", (image,))?;
        let result = result.extract::<PyRef<'_, PyImage>>()?;
        Ok(PyImage {
            inner: result.inner.clone(),
        })
    }

    fn filter_name(&self, filter_type: &str, py: Python<'_>) -> PyResult<PyImage> {
        let filter_type = filter_type.to_owned();
        py.allow_threads(|| self.inner.filter(&filter_type))
            .map(|inner| PyImage { inner })
            .map_err(map_error)
    }

    fn validate_filter(&self, filter_name: &str) -> PyResult<()> {
        self.inner.validate_filter(filter_name).map_err(map_error)
    }

    fn kernel_filter(
        &self,
        kernel: Option<Vec<f64>>,
        scale: Option<f64>,
        offset: f64,
        size: (u32, u32),
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let rs = py
            .allow_threads(|| self.inner.kernel_filter(kernel, scale, offset, size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes_unpacked(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        py.allow_threads(|| self.inner.tobytes_unpacked())
            .map_err(map_error)
    }

    fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: Vec<String>,
        py: Python<'_>,
    ) -> PyResult<Vec<u8>> {
        let mode = mode.to_owned();
        let encoder_name = encoder_name.to_owned();
        py.allow_threads(|| self.inner.tobytes_encoded(&mode, &encoder_name, &args))
            .map_err(map_error)
    }

    /// Lock a lazy image pipeline to the sole active compute backend.
    ///
    /// This keeps an explicitly selected SIMD or GPU process from silently
    /// benchmarking CPU fallback. Ordinary multi-backend routing is unchanged.
    fn lock_active_backend(&self) -> PyResult<PyImage> {
        let active = pillow_rs::active_backends().map_err(map_error)?;
        let inner = if active.len() == 1 {
            self.inner.clone().use_backend(active[0])
        } else {
            self.inner.clone()
        };
        Ok(PyImage { inner })
    }

    #[pyo3(signature = (rawmode=None))]
    fn getpalette_with_input(&self, rawmode: Option<String>, py: Python<'_>) -> PyResult<PyObject> {
        let palette = self
            .inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(map_error)?;
        match palette {
            Some(values) => Ok(PyList::new(py, values)?.to_object(py)),
            None => Ok(py.None()),
        }
    }

    fn indexed_color_table(&self, mode: &str) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table(mode).map_err(map_error)
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    fn apply_transparency(&mut self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.apply_transparency())
            .map_err(map_error)
    }

    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(&mut self, data: Vec<u8>, rawmode: &str, py: Python<'_>) -> PyResult<()> {
        let rawmode = rawmode.to_owned();
        py.allow_threads(|| self.inner.putpalette(&data, &rawmode))
            .map_err(map_error)
    }

    fn get_child_images(&self) -> Vec<PyImage> {
        self.inner
            .get_child_images()
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect()
    }

    fn getexif(&self) -> Vec<u8> {
        self.inner.getexif()
    }

    fn getxmp(&self) -> std::collections::HashMap<String, String> {
        self.inner.getxmp()
    }

    fn getim(&self, py: Python<'_>) -> PyResult<PyObject> {
        // The pure-Rust core owns the unsupported-handle decision. The binding
        // only creates a named capsule so the Python result keeps Pillow's
        // observable shape; its payload is deliberately not an Imaging pointer.
        let _ = self.inner.getim();
        let name = CString::new("PIL Imaging")
            .map_err(|_| pyo3::exceptions::PySystemError::new_err("invalid capsule name"))?;
        Ok(PyCapsule::new(py, 0u8, Some(name))?.into_any().unbind())
    }

    #[pyo3(signature = (size, resample=None))]
    fn thumbnail(
        &mut self,
        size: (i64, i64),
        resample: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let resample = resample_input_from_python(resample)?;
        py.allow_threads(|| self.inner.thumbnail(size, resample))
            .map_err(map_error)
    }

    #[pyo3(signature = (colors=None, method=None, kmeans=None, dither=None, palette=None))]
    fn quantize(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        dither: Option<bool>,
        palette: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let palette = match palette {
            None => pillow_rs::QuantizePalette::None,
            Some(value) => image_from_python(value)
                .map(pillow_rs::QuantizePalette::Image)
                .unwrap_or(pillow_rs::QuantizePalette::Other),
        };
        let rs = py
            .allow_threads(|| {
                self.inner
                    .quantize_with_input(colors, method, kmeans, palette, dither)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getbbox(
        &self,
        alpha_only: Option<bool>,
        py: Python<'_>,
    ) -> PyResult<Option<(u32, u32, u32, u32)>> {
        let alpha_only = alpha_only.unwrap_or(true);
        py.allow_threads(|| self.inner.getbbox(alpha_only))
            .map_err(map_error)
    }

    /// Return extrema formatted as PIL expects.
    fn getextrema_formatted(&self, py: Python<'_>) -> PyResult<PyObject> {
        let formatted = py
            .allow_threads(|| self.inner.getextrema_formatted())
            .map_err(map_error)?;
        Python::with_gil(|py| match formatted {
            pillow_rs::FormattedExtrema::Empty => Ok(py.None()),
            pillow_rs::FormattedExtrema::EmptyMultiple(bands) => {
                let values: Vec<PyObject> = (0..bands).map(|_| py.None()).collect();
                Ok(PyTuple::new(py, values)?.to_object(py))
            }
            pillow_rs::FormattedExtrema::Single((minimum, maximum)) => {
                Ok((minimum, maximum).to_object(py))
            }
            pillow_rs::FormattedExtrema::Multiple(values) => {
                let tuples: Vec<PyObject> = values
                    .into_iter()
                    .map(|(minimum, maximum)| (minimum, maximum).to_object(py))
                    .collect();
                Ok(PyTuple::new(py, tuples)?.to_object(py))
            }
            pillow_rs::FormattedExtrema::Integer((minimum, maximum)) => {
                Ok((minimum, maximum).to_object(py))
            }
            pillow_rs::FormattedExtrema::Float((minimum, maximum)) => {
                Ok((minimum, maximum).to_object(py))
            }
        })
    }
    /// Band names for the active image, delegated to the Rust core.
    fn getbands(&self) -> PyResult<PyObject> {
        let bands = self.inner.getbands().map_err(map_error)?;
        Python::with_gil(|py| {
            let objs: Vec<PyObject> = bands.iter().map(|band| band.to_object(py)).collect();
            Ok(PyTuple::new(py, objs)?.to_object(py))
        })
    }

    #[pyo3(signature = (mask=None))]
    fn stat_formatted(
        &self,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyObject> {
        let mask = imageops_mask_from_python(mask)?;
        let result = py
            .allow_threads(|| self.inner.stat_formatted_with_mask(mask))
            .map_err(map_error)?;
        stat_result_to_python(&result)
    }

    #[pyo3(signature = (mask=None))]
    fn histogram_with_input(
        &self,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Vec<u32>> {
        let mask = image_analysis_mask_from_python(mask)?;
        py.allow_threads(|| self.inner.histogram_with_input(mask))
            .map_err(map_error)
    }

    fn gaussian_blur(&self, radius: Option<f64>, py: Python<'_>) -> PyResult<PyImage> {
        let radius = radius.unwrap_or(2.0) as f32;
        let rs = py
            .allow_threads(|| self.inner.gaussian_blur(radius))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(
        &self,
        radius: Option<f64>,
        percent: Option<i32>,
        threshold: Option<u8>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let radius = radius.unwrap_or(2.0) as f32;
        let percent = percent.unwrap_or(150);
        let threshold = threshold.unwrap_or(3);
        let rs = py
            .allow_threads(|| self.inner.unsharp_mask(radius, percent, threshold))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<u32>, py: Python<'_>) -> PyResult<PyImage> {
        let size = size.unwrap_or(3);
        let rs = py
            .allow_threads(|| self.inner.max_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<u32>, py: Python<'_>) -> PyResult<PyImage> {
        let size = size.unwrap_or(3);
        let rs = py
            .allow_threads(|| self.inner.min_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<u32>, py: Python<'_>) -> PyResult<PyImage> {
        let size = size.unwrap_or(3);
        let rs = py
            .allow_threads(|| self.inner.median_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn box_blur(&self, radius: Option<f64>, py: Python<'_>) -> PyResult<PyImage> {
        let radius = radius.unwrap_or(2.0) as f32;
        let rs = py
            .allow_threads(|| self.inner.box_blur(radius))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn mode_filter(&self, size: Option<u32>, py: Python<'_>) -> PyResult<PyImage> {
        let size = size.unwrap_or(3);
        let rs = py
            .allow_threads(|| self.inner.mode_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn rank_filter(
        &self,
        size: Option<u32>,
        rank: Option<u32>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = size.unwrap_or(3);
        let rank = rank.unwrap_or(0);
        let rs = py
            .allow_threads(|| self.inner.rank_filter(size, rank))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color3dlut(
        &self,
        size: (u32, u32, u32),
        table: Vec<f64>,
        channels: Option<u32>,
        target_mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let input =
            pillow_rs::prepare_color3dlut(table, size, channels.unwrap_or(3)).map_err(map_error)?;
        let target_mode = target_mode.map(str::to_owned);
        let rs = py
            .allow_threads(|| self.inner.color3dlut(input, target_mode.as_deref()))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let selector = if let Ok(channel) = channel.extract::<i32>() {
            pillow_rs::ChannelSelector::Index(channel)
        } else if let Ok(channel) = channel.extract::<String>() {
            pillow_rs::ChannelSelector::Name(channel)
        } else {
            pillow_rs::ChannelSelector::Invalid(channel.get_type().name()?.to_string())
        };
        let rs = py
            .allow_threads(|| self.inner.getchannel_selector(selector))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn load(&mut self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.load()).map_err(map_error)
    }

    fn putalpha_input(&mut self, alpha: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let input = if let Some(mask) = image_from_python(alpha) {
            pillow_rs::PutAlphaInput::Image(mask)
        } else if let Ok(value) = alpha.extract::<i64>() {
            pillow_rs::PutAlphaInput::Integer(value)
        } else {
            pillow_rs::PutAlphaInput::Invalid(alpha.get_type().name()?.to_string())
        };
        py.allow_threads(|| self.inner.putalpha_with_input(input))
            .map_err(map_error)
    }

    fn reduce(
        &self,
        factor: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let factor = reduce_factor_from_python(factor)?;
        let box_coords = box_coords
            .map(|value| reduce_box_from_python(Some(value)))
            .transpose()?;
        let rs = py
            .allow_threads(|| self.inner.reduce_public(factor, box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, dest=None, source=None))]
    fn alpha_composite(
        &mut self,
        im: &Bound<'_, PyImage>,
        dest: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let source_image = im.borrow().inner.clone();
        let dest = dest.map_or_else(
            || pillow_rs::AlphaCompositeBox::Values(vec![0, 0]),
            |value| {
                value
                    .extract::<Vec<i64>>()
                    .map(pillow_rs::AlphaCompositeBox::Values)
                    .unwrap_or(pillow_rs::AlphaCompositeBox::Invalid)
            },
        );
        let source_box = source.map_or_else(
            || pillow_rs::AlphaCompositeBox::Values(vec![0, 0]),
            |value| {
                value
                    .extract::<Vec<i64>>()
                    .map(pillow_rs::AlphaCompositeBox::Values)
                    .unwrap_or(pillow_rs::AlphaCompositeBox::Invalid)
            },
        );
        py.allow_threads(|| {
            self.inner
                .alpha_composite_public(&source_image, dest, source_box)
        })
        .map_err(map_error)
    }

    /// Return getcolors formatted as PIL expects.
    fn getcolors_formatted(
        &mut self,
        maxcolors: Option<u32>,
        py: Python<'_>,
    ) -> PyResult<Option<PyObject>> {
        let maxcolors = maxcolors.unwrap_or(256);
        let formatted = py
            .allow_threads(|| self.inner.getcolors_formatted(maxcolors))
            .map_err(map_error)?;
        Python::with_gil(|py| match formatted {
            None => Ok(None),
            Some(results) => {
                let out = pyo3::types::PyList::empty(py);
                for (count, color) in results {
                    let color_value = match color {
                        pillow_rs::FormattedPixelValue::Scalar(value) => value.to_object(py),
                        pillow_rs::FormattedPixelValue::Integer(value) => value.to_object(py),
                        pillow_rs::FormattedPixelValue::Float(value) => value.to_object(py),
                        pillow_rs::FormattedPixelValue::Components(values) => {
                            PyTuple::new(py, values)?.to_object(py)
                        }
                    };
                    let entry = PyTuple::new(py, [count.to_object(py), color_value])?;
                    out.append(entry)?;
                }
                Ok(Some(out.to_object(py)))
            }
        })
    }

    /// Return getdata formatted as PIL expects.
    fn getdata_formatted(&mut self, band: Option<i32>, py: Python<'_>) -> PyResult<PyObject> {
        let formatted = py
            .allow_threads(|| self.inner.getdata_formatted(band))
            .map_err(map_error)?;
        Python::with_gil(|py| match formatted {
            pillow_rs::FormattedImageData::Scalars(values) if band.is_some() => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(value)?;
                }
                Ok(out.to_object(py))
            }
            pillow_rs::FormattedImageData::Scalars(values) => Ok(values.to_object(py)),
            pillow_rs::FormattedImageData::IntegerScalars(values) => Ok(values.to_object(py)),
            pillow_rs::FormattedImageData::FloatScalars(values) => Ok(values.to_object(py)),
            pillow_rs::FormattedImageData::Components(values) => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(PyTuple::new(py, value)?)?;
                }
                Ok(out.to_object(py))
            }
        })
    }

    fn getprojection(&mut self, py: Python<'_>) -> PyResult<(Vec<u32>, Vec<u32>)> {
        py.allow_threads(|| self.inner.getprojection())
            .map_err(map_error)
    }

    #[pyo3(signature = (mask=None))]
    fn entropy_with_input(
        &mut self,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<f64> {
        let mask = image_analysis_mask_from_python(mask)?;
        py.allow_threads(|| self.inner.entropy_with_input(mask))
            .map_err(map_error)
    }

    fn seek(&mut self, frame: u32, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.seek(frame))
            .map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    /// Applies a sequence or callable LUT through the Rust-owned public path.
    fn point(&self, input: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let input_kind = if input.is_instance_of::<PyString>() {
            pillow_rs::EvalInputKind::String
        } else {
            pillow_rs::EvalInputKind::Other
        };
        pillow_rs::validate_eval_input(input_kind).map_err(map_error)?;
        if input.is_callable() {
            return pillow_rs::image_eval_callable(&self.inner, |sample| {
                let result = input.call1((sample,)).map_err(|error| {
                    pillow_rs::PilError::ValueError(format!("LUT function failed: {error}"))
                })?;
                result.extract::<i32>().map_err(|_| {
                    pillow_rs::PilError::ValueError(
                        "LUT function must return an integer".to_owned(),
                    )
                })
            })
            .map(|i| PyImage { inner: i })
            .map_err(map_error);
        }

        let lut = input.extract::<Vec<u8>>()?;
        py.allow_threads(|| pillow_rs::image_eval_validated(&self.inner, &lut))
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn effect_spread(&self, distance: u32, py: Python<'_>) -> PyResult<PyImage> {
        py.allow_threads(|| pillow_rs::image_effect_spread(&self.inner, distance))
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    #[pyo3(signature = (size, method, data=None, resample=0, fill=1, fillcolor=None))]
    fn transform(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<&Bound<'_, PyAny>>,
        resample: Option<i32>,
        fill: Option<i32>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let data = transform_data_from_python(data)?;
        let fillcolor = transform_fill_from_python(fillcolor)?;
        let resample = resample.unwrap_or(0);
        let fill = fill.unwrap_or(1);
        py.allow_threads(|| {
            self.inner
                .transform_public(size, method, data, resample, fill, fillcolor)
        })
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
    }

    #[staticmethod]
    #[pyo3(signature = (mode, size, data, decoder_name="raw"))]
    fn frombytes(
        mode: &str,
        size: (u32, u32),
        data: Vec<u8>,
        decoder_name: &str,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let mode = mode.to_owned();
        let decoder_name = decoder_name.to_owned();
        py.allow_threads(|| pillow_rs::image_frombytes(&mode, size, &data, &decoder_name))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[pyo3(signature = (dest_map, source_palette=None))]
    fn remap_palette(
        &mut self,
        dest_map: Vec<u8>,
        source_palette: Option<Vec<u8>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let remapped = py
            .allow_threads(|| match source_palette.as_deref() {
                None => self.inner.remap_palette(&dest_map),
                Some(source_palette) => self
                    .inner
                    .remap_palette_with_source(&dest_map, Some(source_palette)),
            })
            .map(|i| PyImage { inner: i })
            .map_err(map_error)?;
        Ok(remapped)
    }

    fn tobitmap(&mut self, py: Python<'_>) -> PyResult<Vec<u8>> {
        py.allow_threads(|| self.inner.tobitmap())
            .map_err(map_error)
    }

    #[classmethod]
    fn blend(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        alpha: f64,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow().inner.clone();
        let im2 = image2.borrow().inner.clone();
        py.allow_threads(|| pillow_rs::image_blend(&im1, &im2, alpha))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn composite(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        mask: &Bound<'_, PyImage>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow().inner.clone();
        let im2 = image2.borrow().inner.clone();
        let m = mask.borrow().inner.clone();
        py.allow_threads(|| pillow_rs::image_composite(&im1, &im2, &m))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn merge(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        bands: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let inputs = merge_inputs_from_python(bands)?;
        let mode = mode.to_owned();
        py.allow_threads(|| pillow_rs::image_merge_inputs(&mode, &inputs))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| self.inner.verify()).map_err(map_error)
    }

    fn enhance_brightness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_brightness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_contrast(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_contrast(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_color(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_sharpness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_sharpness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getpixel_formatted(&mut self, xy: (u32, u32), py: Python<'_>) -> PyResult<PyObject> {
        let value = py
            .allow_threads(|| self.inner.getpixel_formatted(xy.0, xy.1))
            .map_err(map_error)?;
        Python::with_gil(|py| match value {
            pillow_rs::FormattedPixelValue::Scalar(value) => Ok(value.to_object(py)),
            pillow_rs::FormattedPixelValue::Integer(value) => Ok(value.to_object(py)),
            pillow_rs::FormattedPixelValue::Float(value) => Ok(value.to_object(py)),
            pillow_rs::FormattedPixelValue::Components(values) => {
                Ok(PyTuple::new(py, values)?.to_object(py))
            }
        })
    }

    #[pyo3(signature = (data, scale=1.0, offset=0.0))]
    fn putdata_formatted(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
    ) -> PyResult<()> {
        putdata::putdata_formatted(slf, data, scale, offset)
    }

    /// Mode-aware putpixel: expands values according to PIL's per-mode semantics.
    fn putpixel_mode(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = putpixel_value_from_python(value);
        self.inner
            .putpixel_value(xy.0, xy.1, value)
            .map_err(map_error)
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(map_error)
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(map_error)?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(map_error)?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(map_error)
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn compatibility_info(&self, py: Python<'_>) -> PyResult<PyObject> {
        image_info_to_python(py, self.inner.compatibility_info())
    }

    fn converted_compatibility_info(
        &self,
        py: Python<'_>,
        target_mode: &str,
    ) -> PyResult<PyObject> {
        image_info_to_python(py, self.inner.converted_compatibility_info(target_mode))
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self.inner.format_name().unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}

fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::OsError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::AssertionError(msg) => pyo3::exceptions::PyAssertionError::new_err(msg),
        PilError::IndexError(msg) => pyo3::exceptions::PyIndexError::new_err(msg),
        PilError::KeyError(msg) => pyo3::exceptions::PyKeyError::new_err(msg),
        PilError::KeyErrorInt(key) => pyo3::exceptions::PyKeyError::new_err(key),
        PilError::AttributeError(msg) => pyo3::exceptions::PyAttributeError::new_err(msg),
        PilError::EOFError(msg) => pyo3::exceptions::PyEOFError::new_err(msg),
        PilError::UnsupportedLibraqm => {
            let message = PilError::UnsupportedLibraqm.to_string();
            pyo3::exceptions::PyKeyError::new_err(
                message
                    .strip_prefix('\'')
                    .and_then(|inner| inner.strip_suffix('\''))
                    .unwrap_or(&message)
                    .to_owned(),
            )
        }
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::OverflowError(msg) => pyo3::exceptions::PyOverflowError::new_err(msg),
        PilError::DecompressionBombError(msg) => DecompressionBombError::new_err(msg),
        PilError::UnicodeEncodeError {
            encoding,
            object,
            start,
            end,
            reason,
            ..
        } => {
            pyo3::exceptions::PyUnicodeEncodeError::new_err((encoding, object, start, end, reason))
        }
        PilError::SyntaxError(msg) => pyo3::exceptions::PySyntaxError::new_err(msg),
        PilError::SystemError(msg) => pyo3::exceptions::PySystemError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ZeroDivisionError(msg) => pyo3::exceptions::PyZeroDivisionError::new_err(msg),
        // Pillow reports deferred decoder failures (for example, a valid PNG
        // header whose image payload is missing) as OSError from Image.load.
        // Keep the Rust codec error message while preserving that public
        // exception category at the binding boundary.
        PilError::ImageError(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => pyo3::exceptions::PyNotImplementedError::new_err(msg),
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => {
            pyo3::exceptions::PyFileNotFoundError::new_err(err.to_string())
        }
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
        // AS PER DESIGN — DO NOT REMOVE: New error variants from Fix 9
        PilError::PaletteError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::InternalError(msg) => pyo3::exceptions::PyException::new_err(msg),
        PilError::DimensionError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
    }
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

#[pyfunction]
fn transposed_font_orientation(orientation: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if orientation.is_none() {
        return Ok(None);
    }
    Ok(pillow_rs::normalize_transposed_font_input(
        transpose_input_from_python(orientation)?,
    ))
}

#[pyfunction]
fn transposed_font_bbox(
    bbox: (i32, i32, i32, i32),
    orientation: Option<&str>,
) -> (i32, i32, i32, i32) {
    pillow_rs::transposed_bbox(bbox, orientation)
}

#[pyfunction]
fn validate_transposed_font_length(orientation: Option<&str>) -> PyResult<()> {
    pillow_rs::validate_transposed_length(orientation).map_err(map_error)
}

/// Activate a compute backend. Returns true if the backend exists on this machine.
#[pyfunction]
fn enable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::enable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// Deactivate a compute backend. Returns true if it was active.
#[pyfunction]
fn disable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::disable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// List backends that exist on this machine.
#[pyfunction]
fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// List currently active backends (priority order).
#[pyfunction]
fn active_backends() -> PyResult<Vec<String>> {
    Ok(pillow_rs::active_backends()
        .map_err(map_error)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[pyfunction]
fn backend_enabled(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::backend_enabled(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// Enable or disable bounded image-pipeline execution telemetry.
#[pyfunction]
fn set_pipeline_telemetry(enabled: bool) -> bool {
    pillow_rs::Backend::set_pipeline_telemetry_enabled(enabled)
}

/// Take the most recent completed image-pipeline telemetry sample for this
/// thread, or return ``None`` when no sample is available.
#[pyfunction]
fn take_pipeline_telemetry(py: Python<'_>) -> PyResult<Option<PyObject>> {
    let Some((
        requested_backend,
        actual_backend,
        operation_count,
        route_ns,
        validation_ns,
        backend_ns,
        dispatch_count,
        fallback_reason,
        resource,
        resize_coeff_cache_hits,
        resize_coeff_cache_misses,
    )) = pillow_rs::Backend::take_pipeline_telemetry()
    else {
        return Ok(None);
    };

    let result = PyDict::new(py);
    result.set_item(
        "requested_backend",
        requested_backend.map(|backend| format!("{:?}", backend).to_lowercase()),
    )?;
    result.set_item(
        "actual_backend",
        format!("{:?}", actual_backend).to_lowercase(),
    )?;
    result.set_item("operation_count", operation_count)?;
    result.set_item("route_ns", route_ns)?;
    result.set_item("validation_ns", validation_ns)?;
    result.set_item("backend_ns", backend_ns)?;
    result.set_item("dispatch_count", dispatch_count)?;
    result.set_item("fallback_reason", fallback_reason)?;
    result.set_item("resize_coeff_cache_hits", resize_coeff_cache_hits)?;
    result.set_item("resize_coeff_cache_misses", resize_coeff_cache_misses)?;
    if let Some(resource) = resource {
        let resource_dict = PyDict::new(py);
        resource_dict.set_item("upload_bytes", resource.upload_bytes)?;
        resource_dict.set_item("readback_bytes", resource.readback_bytes)?;
        resource_dict.set_item("auxiliary_bytes", resource.auxiliary_bytes)?;
        resource_dict.set_item("parameter_bytes", resource.parameter_bytes)?;
        resource_dict.set_item("retained_cache_bytes", resource.retained_cache_bytes)?;
        resource_dict.set_item("full_frame_copy_count", resource.full_frame_copy_count)?;
        resource_dict.set_item("mode_conversion_count", resource.mode_conversion_count)?;
        resource_dict.set_item("host_buffer_count", resource.host_buffer_count)?;
        resource_dict.set_item("host_buffer_bytes", resource.host_buffer_bytes)?;
        resource_dict.set_item("peak_live_host_bytes", resource.peak_live_host_bytes)?;
        resource_dict.set_item("fused_operation_count", resource.fused_operation_count)?;
        result.set_item("resource", resource_dict)?;
    } else {
        result.set_item("resource", py.None())?;
    }
    Ok(Some(result.into()))
}

// --- Utility functions (moved from Python to satisfy "thin wrapper" rule) ---

/// Align each scanline to a 4-byte boundary (Qt/BMP compatibility).
/// Matches PIL's `ImageQt._toqclass_helper` align8to32 padding logic.
#[pyfunction]
#[pyo3(signature = (data, width, bits_per_pixel=8))]
fn align_row_to_32(data: Vec<u8>, width: u32, bits_per_pixel: u8) -> PyResult<Vec<u8>> {
    pillow_rs::align_row_to_32(&data, width, bits_per_pixel).map_err(map_error)
}

fn array_interface_bytes(value: &Bound<'_, PyAny>, mode: Option<&str>) -> PyResult<Vec<u8>> {
    let memoryview = value
        .py()
        .import("builtins")?
        .getattr("memoryview")?
        .call1((value,));
    match memoryview {
        Ok(memoryview) => memoryview.call_method0("tobytes")?.extract::<Vec<u8>>(),
        Err(_) => {
            let type_name = value.get_type().name()?;
            let message = if mode == Some("RGBA") {
                "expected string or buffer".to_owned()
            } else {
                format!("a bytes-like object is required, not '{type_name}'")
            };
            Err(PyTypeError::new_err(message))
        }
    }
}

fn array_interface_descriptor(value: &Bound<'_, PyAny>) -> PyResult<(Vec<usize>, String)> {
    let interface = value.getattr("__array_interface__")?;
    let interface = interface.downcast::<PyDict>()?;
    let shape = interface
        .get_item("shape")?
        .ok_or_else(|| PyValueError::new_err("__array_interface__ has no shape"))?
        .extract::<Vec<usize>>()?;
    let typestr = interface
        .get_item("typestr")?
        .ok_or_else(|| PyValueError::new_err("__array_interface__ has no typestr"))?
        .extract::<String>()?;
    Ok((shape, typestr))
}

/// Create an image from a Python object implementing Pillow's array interface.
///
/// The ABI layer only marshals Python protocols into plain Rust values. Dtype,
/// shape, mode, and byte-layout policy are implemented by `pillow-rs` so the
/// Python and JavaScript bindings cannot grow divergent `fromarray` logic.
#[pyfunction]
fn fromarray(data: &Bound<'_, PyAny>, mode: Option<&str>) -> PyResult<PyImage> {
    let (shape, typestr) = array_interface_descriptor(data)?;
    // Resolve dimensions before touching the Python buffer. Pillow reports
    // malformed shape/mode combinations before it asks the object for a
    // byte buffer.
    let layout = pillow_rs::resolve_array_layout(&shape, &typestr, mode).map_err(map_error)?;
    let bytes = array_interface_bytes(data, mode)?;
    pillow_rs::from_resolved_array_interface(&layout, &bytes)
        .map(|img| PyImage { inner: img })
        .map_err(map_error)
}

#[pyfunction]
fn imaging_core_to_bytes(py: Python<'_>, values: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let input = values
        .extract::<Vec<i64>>()
        .map(pillow_rs::ImagingCoreBytesInput::Scalars)
        .or_else(|_| {
            values
                .extract::<Vec<f64>>()
                .map(pillow_rs::ImagingCoreBytesInput::Floats)
        })
        .unwrap_or(pillow_rs::ImagingCoreBytesInput::Multiband);
    let bytes = pillow_rs::imaging_core_to_bytes(input).map_err(map_error)?;
    Ok(PyBytes::new(py, &bytes).into())
}

fn image_info_value_to_python(
    py: Python<'_>,
    value: pillow_rs::ImageInfoValue,
) -> PyResult<PyObject> {
    match value {
        pillow_rs::ImageInfoValue::Integer(value) => Ok(value.to_object(py)),
        pillow_rs::ImageInfoValue::Float(value) => Ok(value.to_object(py)),
        pillow_rs::ImageInfoValue::String(value) => Ok(value.to_object(py)),
        pillow_rs::ImageInfoValue::Bytes(value) => Ok(PyBytes::new(py, &value).into()),
        pillow_rs::ImageInfoValue::IntegerList(value) => Ok(value.to_object(py)),
        pillow_rs::ImageInfoValue::FloatList(value) => Ok(value.to_object(py)),
        pillow_rs::ImageInfoValue::IntegerTuple(value) => Ok(PyTuple::new(py, value)?.into()),
        pillow_rs::ImageInfoValue::Object(fields) => {
            let result = PyDict::new(py);
            for (key, value) in fields {
                result.set_item(key, image_info_value_to_python(py, value)?)?;
            }
            Ok(result.into())
        }
    }
}

fn image_info_to_python(
    py: Python<'_>,
    fields: Vec<(String, pillow_rs::ImageInfoValue)>,
) -> PyResult<PyObject> {
    let result = PyDict::new(py);
    for (key, value) in fields {
        result.set_item(key, image_info_value_to_python(py, value)?)?;
    }
    Ok(result.into())
}

#[pyfunction]
fn exif_compat_fields(
    py: Python<'_>,
    raw: Option<Vec<u8>>,
    loaded_exif: bool,
) -> PyResult<PyObject> {
    let fields = pillow_rs::prepare_exif_compat(raw.as_deref(), loaded_exif);
    let result = PyDict::new(py);
    result.set_item("_loaded_exif", fields.loaded_exif)?;
    result.set_item("_loaded", true)?;
    if fields.has_source {
        result.set_item("fp", py.None())?;
        result.set_item("head", fields.head.unwrap_or_default())?;
        if let Some(endian) = fields.endian {
            result.set_item("endian", endian)?;
        }
        if fields.bigtiff {
            result.set_item("bigtiff", false)?;
        }
    }
    Ok(result.into())
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    m.add(
        "DecompressionBombError",
        m.py().get_type::<DecompressionBombError>(),
    )?;
    m.add_class::<PyImage>()?;
    m.add_class::<PyImageSequenceIterator>()?;
    m.add_class::<PyDraw>()?;
    m.add_class::<PyOutline>()?;
    m.add_class::<PyFont>()?;
    m.add_class::<PyPilFont>()?;
    m.add_function(wrap_pyfunction!(transposed_font_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(validate_transposed_font_length, m)?)?;
    m.add_function(wrap_pyfunction!(transposed_font_orientation, m)?)?;

    // ImageOps functions
    m.add_function(wrap_pyfunction!(ops_autocontrast, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_validate_deform_resample, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_colorize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_contain, m)?)?;
    m.add_function(wrap_pyfunction!(ops_cover, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit, m)?)?;
    m.add_function(wrap_pyfunction!(ops_pad, m)?)?;
    m.add_function(wrap_pyfunction!(ops_scale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_border, m)?)?;
    m.add_function(wrap_pyfunction!(ops_exif_transpose, m)?)?;

    // ImageChops functions
    m.add_function(wrap_pyfunction!(chops_add, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract, m)?)?;
    m.add_function(wrap_pyfunction!(chops_multiply, m)?)?;
    m.add_function(wrap_pyfunction!(chops_screen, m)?)?;
    m.add_function(wrap_pyfunction!(chops_darker, m)?)?;
    m.add_function(wrap_pyfunction!(chops_lighter, m)?)?;
    m.add_function(wrap_pyfunction!(chops_difference, m)?)?;
    m.add_function(wrap_pyfunction!(chops_duplicate, m)?)?;
    m.add_function(wrap_pyfunction!(chops_invert, m)?)?;

    // More ImageChops
    m.add_function(wrap_pyfunction!(chops_add_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_constant, m)?)?;
    m.add_function(wrap_pyfunction!(chops_hard_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_soft_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_overlay, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_and, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_or, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_xor, m)?)?;
    m.add_function(wrap_pyfunction!(chops_offset, m)?)?;

    // ImageColor
    m.add_function(wrap_pyfunction!(getrgb, m)?)?;
    m.add_function(wrap_pyfunction!(getcolor, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_append, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_validate, m)?)?;
    m.add_function(wrap_pyfunction!(palette_save_to_file, m)?)?;
    m.add_function(wrap_pyfunction!(palette_to_text, m)?)?;

    // ImageStat module helpers
    m.add_function(wrap_pyfunction!(stat_from_histogram, m)?)?;

    // Image module functions
    m.add_function(wrap_pyfunction!(image_merge, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_alpha_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_linear_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_radial_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_effect_mandelbrot, m)?)?;

    // GPU functions
    m.add_function(wrap_pyfunction!(enable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(disable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(available_backends, m)?)?;
    m.add_function(wrap_pyfunction!(active_backends, m)?)?;
    m.add_function(wrap_pyfunction!(backend_enabled, m)?)?;
    m.add_function(wrap_pyfunction!(set_pipeline_telemetry, m)?)?;
    m.add_function(wrap_pyfunction!(take_pipeline_telemetry, m)?)?;

    // ImageFilter helper functions
    m.add_function(wrap_pyfunction!(color3dlut_check_size, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_new, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_generate, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_transform, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_repr, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_validate_coefficients, m)?)?;

    // Utility functions (moved from Python)
    m.add_function(wrap_pyfunction!(align_row_to_32, m)?)?;
    m.add_function(wrap_pyfunction!(fromarray, m)?)?;
    m.add_function(wrap_pyfunction!(imaging_core_to_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(exif_compat_fields, m)?)?;
    m.add_function(wrap_pyfunction!(imagefont_normalize_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(imagefont_normalize_layout_engine, m)?)?;
    m.add_function(wrap_pyfunction!(transpose_from_int, m)?)?;

    Ok(())
}

// --- ImageFont ---

#[pyclass(name = "ImageFont", unsendable)]
pub struct PyFont {
    inner: pillow_rs::FreeTypeFont,
}

fn font_bbox_value_to_python(py: Python<'_>, value: pillow_rs::ImageFontBBoxValue) -> PyObject {
    match value {
        pillow_rs::ImageFontBBoxValue::Integer(value) => value.to_object(py),
        pillow_rs::ImageFontBBoxValue::Float(value) => value.to_object(py),
    }
}

#[pyfunction]
fn imagefont_normalize_bbox(
    py: Python<'_>,
    bbox: (f64, f64, f64, f64),
) -> (PyObject, PyObject, PyObject, PyObject) {
    let values = pillow_rs::normalize_font_bbox(bbox);
    (
        font_bbox_value_to_python(py, values[0]),
        font_bbox_value_to_python(py, values[1]),
        font_bbox_value_to_python(py, values[2]),
        font_bbox_value_to_python(py, values[3]),
    )
}

#[pyfunction]
fn imagefont_normalize_layout_engine(
    py: Python<'_>,
    layout_engine: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let value = if layout_engine.is_none() {
        None
    } else {
        layout_engine.extract::<i64>().ok()
    };
    let (name, requested_raqm) = pillow_rs::normalize_layout_engine(value);
    if requested_raqm {
        PyErr::warn(
            py,
            &py.get_type::<PyUserWarning>(),
            pyo3::ffi::c_str!(
                "Raqm layout was requested, but Raqm is not available. Falling back to basic layout."
            ),
            3,
        )?;
    }
    Ok(name.to_owned())
}

#[pyfunction]
fn transpose_from_int(value: i64) -> &'static str {
    pillow_rs::transpose_name_from_int(value)
}

/// Marshal Python's path/file-like font source into core-owned input.
/// Filesystem access and Python protocol calls stay at this host boundary;
/// source validation and font loading remain in `pillow-rs`.
fn imagefont_source_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontSourceInput> {
    if let Some(path) = host_path_from_python(value)? {
        let data = std::fs::read(&path).map_err(|error| {
            pyo3::exceptions::PyOSError::new_err(format!("Cannot read font file: {}", error))
        })?;
        return Ok(pillow_rs::ImageFontSourceInput::Bytes(data));
    }
    if value.hasattr("read")? {
        let data = value.call_method0("read")?.extract::<Vec<u8>>()?;
        return Ok(pillow_rs::ImageFontSourceInput::Bytes(data));
    }
    Ok(pillow_rs::ImageFontSourceInput::Invalid)
}

/// Classify a Python font text argument before handing it to the core.
///
/// This is deliberately limited to host-object conversion: the core owns the
/// byte-text interpretation and every font operation consumes the same input
/// enum.
fn imagefont_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontTextInput> {
    if let Ok(bytes) = value.downcast::<PyBytes>() {
        return Ok(pillow_rs::ImageFontTextInput::Bytes(
            bytes.as_bytes().to_vec(),
        ));
    }
    Ok(pillow_rs::ImageFontTextInput::Text(
        value.str()?.to_string(),
    ))
}

fn imagefont_variation_name_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontVariationNameInput> {
    if let Ok(bytes) = value.downcast::<PyBytes>() {
        return Ok(pillow_rs::ImageFontVariationNameInput::Bytes(
            bytes.as_bytes().to_vec(),
        ));
    }
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::ImageFontVariationNameInput::Text(name));
    }
    Ok(pillow_rs::ImageFontVariationNameInput::InvalidType(
        value.get_type().name()?.to_string(),
    ))
}

fn imagefont_start_from_python(value: Option<&Bound<'_, PyAny>>) -> (Option<(f64, f64)>, bool) {
    let Some(value) = value else {
        return (None, false);
    };
    match value.extract::<(f64, f64)>() {
        Ok(start) => (Some(start), false),
        Err(_) => (None, true),
    }
}

fn pilfont_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::PilFontTextInput> {
    if let Ok(text) = value.extract::<String>() {
        return Ok(pillow_rs::PilFontTextInput::Text(text));
    }
    value
        .extract::<Vec<u8>>()
        .map(pillow_rs::PilFontTextInput::Bytes)
}

fn variation_axes_to_python(
    py: Python<'_>,
    axes: Vec<pillow_rs::ImageFontVariationAxis>,
) -> PyResult<Vec<PyObject>> {
    axes.into_iter()
        .map(|axis| {
            let dict = PyDict::new(py);
            dict.set_item("minimum", axis.minimum)?;
            dict.set_item("default", axis.default)?;
            dict.set_item("maximum", axis.maximum)?;
            dict.set_item("name", PyBytes::new(py, &axis.name))?;
            Ok(dict.into())
        })
        .collect()
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    #[pyo3(signature = (fp, size, index=0, encoding="", layout_engine=None))]
    fn truetype(
        fp: &Bound<'_, PyAny>,
        size: f64,
        index: usize,
        encoding: &str,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let options = pillow_rs::ImageFontLoadOptions {
            index: Some(index),
            encoding: (!encoding.is_empty()).then(|| encoding.to_owned()),
            layout_engine,
        };
        let source = imagefont_source_from_python(fp)?;
        let font =
            pillow_rs::imagefont_from_source(source, size as f32, &options).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    #[pyo3(signature = (data, size, index=0, encoding="", layout_engine=None))]
    fn truetype_from_bytes(
        data: Vec<u8>,
        size: f64,
        index: usize,
        encoding: &str,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let options = pillow_rs::ImageFontLoadOptions {
            index: Some(index),
            encoding: (!encoding.is_empty()).then(|| encoding.to_owned()),
            layout_engine,
        };
        let font = pillow_rs::imagefont_from_bytes_with_options(data, size as f32, &options)
            .map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    #[pyo3(signature = (size=None))]
    fn load_default(size: Option<f32>) -> PyResult<Self> {
        let sz = size.unwrap_or(10.0);
        let font = pillow_rs::imagefont_load_default(sz).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    fn getbbox(&self, text: &str) -> PyResult<(i32, i32, i32, i32)> {
        pillow_rs::imagefont_getbbox(&self.inner, text).map_err(map_error)
    }

    fn getbbox_bytes(&self, text: Vec<u8>) -> PyResult<(i32, i32, i32, i32)> {
        pillow_rs::imagefont_getbbox_bytes(&self.inner, &text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn getbbox_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(f32, f32, f32, f32)> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getbbox_input_with_options(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)
    }

    fn getmask_alpha(&self, text: &str) -> PyResult<(u32, u32, Vec<u8>)> {
        pillow_rs::imagefont_getmask(&self.inner, text).map_err(map_error)
    }

    fn getmask_alpha_bytes(&self, text: Vec<u8>) -> PyResult<(u32, u32, Vec<u8>)> {
        pillow_rs::imagefont_getmask_bytes(&self.inner, &text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None))]
    fn getmask_alpha_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let (features, features_invalid) = draw_features_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            ink,
            start,
            start_invalid,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let (width, height, pixels) = pillow_rs::imagefont_getmask_input_with_options(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)?;
        let inner =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok(PyImage { inner })
    }

    fn get_transposed_mask_image(
        &self,
        text: &Bound<'_, PyAny>,
        orientation: Option<&str>,
    ) -> PyResult<PyImage> {
        let (width, height, pixels) = pillow_rs::imagefont_get_transposed_mask_input(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            orientation,
        )
        .map_err(map_error)?;
        let inner = pillow_rs::imagefont_mask_image(
            width,
            height,
            pixels,
            &pillow_rs::ImageFontTextOptions::default(),
        )
        .map_err(map_error)?;
        Ok(PyImage { inner })
    }

    #[pyo3(signature = (text, start=None))]
    fn getmask2_image(
        &self,
        text: &str,
        start: Option<(f64, f64)>,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let (width, height, pixels, offset) = match start {
            None => pillow_rs::imagefont_getmask2(&self.inner, text),
            Some(start) => pillow_rs::imagefont_getmask2_with_start(&self.inner, text, start),
        }
        .map_err(map_error)?;
        let inner = pillow_rs::imagefont_mask_image(
            width,
            height,
            pixels,
            &pillow_rs::ImageFontTextOptions::default(),
        )
        .map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    #[pyo3(signature = (text, start=None))]
    fn getmask2_image_bytes(
        &self,
        text: Vec<u8>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let (width, height, pixels, offset) = match start {
            None => pillow_rs::imagefont_getmask2_bytes(&self.inner, &text),
            Some(start) => {
                pillow_rs::imagefont_getmask2_bytes_with_start(&self.inner, &text, start)
            }
        }
        .map_err(map_error)?;
        let inner = pillow_rs::imagefont_mask_image(
            width,
            height,
            pixels,
            &pillow_rs::ImageFontTextOptions::default(),
        )
        .map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None, stroke_filled=false, has_args=false, has_kwargs=false))]
    fn getmask2_image_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let (features, features_invalid) = draw_features_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features,
            features_invalid,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            anchor_invalid_length_error: false,
            ink,
            start,
            start_invalid,
            has_args,
            has_kwargs,
        };
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_input_with_options(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)?;
        let image =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok((PyImage { inner: image }, offset))
    }

    fn getlength(&self, text: &str) -> PyResult<i32> {
        pillow_rs::imagefont_native_getlength_26dot6(&self.inner, text).map_err(map_error)
    }

    fn getlength_alpha(&self, text: &str) -> PyResult<f32> {
        pillow_rs::imagefont_getlength(&self.inner, text).map_err(map_error)
    }

    fn getlength_bytes(&self, text: Vec<u8>) -> PyResult<f32> {
        pillow_rs::imagefont_getlength_bytes(&self.inner, &text).map_err(map_error)
    }

    fn getsize(&self, text: &str) -> PyResult<((i32, i32), (i32, i32))> {
        pillow_rs::imagefont_native_getsize(&self.inner, text).map_err(map_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, stroke_filled=false, anchor=None, ink=None, start=None))]
    fn render_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        stroke_filled: bool,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let (features, features_invalid) = draw_features_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features,
            features_invalid,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            anchor_invalid_length_error: false,
            ink,
            start,
            start_invalid,
            has_args: false,
            has_kwargs: false,
        };
        let (width, height, pixels, offset) = pillow_rs::imagefont_native_render_input(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)?;
        let image =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok((PyImage { inner: image }, offset))
    }

    #[getter]
    fn family(&self) -> Option<String> {
        pillow_rs::imagefont_native_face_attrs(&self.inner)
            .0
            .map(str::to_owned)
    }

    #[getter]
    fn style(&self) -> Option<String> {
        pillow_rs::imagefont_native_face_attrs(&self.inner)
            .1
            .map(str::to_owned)
    }

    #[getter]
    fn ascent(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).2
    }

    #[getter]
    fn descent(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).3
    }

    #[getter]
    fn height(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).4
    }

    #[getter]
    fn x_ppem(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).5
    }

    #[getter]
    fn y_ppem(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).6
    }

    #[getter]
    fn glyphs(&self) -> i64 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).7
    }

    #[getter]
    fn font_format(&self) -> &'static str {
        pillow_rs::imagefont_font_format(&self.inner)
    }

    #[getter]
    fn is_scalable(&self) -> bool {
        pillow_rs::imagefont_is_scalable(&self.inner)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None))]
    fn getlength_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
    ) -> PyResult<f32> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            features_invalid,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getlength_input_with_options(
            &self.inner,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)
    }

    fn getmetrics(&self) -> (u32, u32) {
        pillow_rs::imagefont_getmetrics(&self.inner)
    }

    fn has_variations(&self) -> bool {
        pillow_rs::imagefont_has_variations(&self.inner)
    }

    fn get_variation_axes(&self) -> PyResult<Vec<PyObject>> {
        Python::with_gil(|py| {
            pillow_rs::imagefont_get_variation_axes(&self.inner)
                .map_err(map_error)
                .and_then(|axes| variation_axes_to_python(py, axes))
        })
    }

    fn get_variation_names(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_get_variation_names(&self.inner).map_err(map_error)
    }

    fn getvarnames(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_native_getvarnames(&self.inner).map_err(map_error)
    }

    fn getvaraxes(&self) -> PyResult<Vec<PyObject>> {
        Python::with_gil(|py| {
            pillow_rs::imagefont_native_getvaraxes(&self.inner)
                .map_err(map_error)
                .and_then(|axes| variation_axes_to_python(py, axes))
        })
    }

    fn set_variation_by_name(&mut self, name: &Bound<'_, PyAny>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_name_input(
            &mut self.inner,
            imagefont_variation_name_input_from_python(name)?,
        )
        .map_err(map_error)
    }

    fn setvarname(&mut self, instance_index: i64) -> PyResult<()> {
        pillow_rs::imagefont_native_setvarname(&mut self.inner, instance_index).map_err(map_error)
    }

    fn set_variation_by_axes(&mut self, axes: &Bound<'_, PyAny>) -> PyResult<()> {
        let input = if axes.downcast::<PyList>().is_ok() {
            axes.extract::<Vec<f32>>()
                .map(pillow_rs::ImageFontVariationAxesInput::Values)
                .unwrap_or(pillow_rs::ImageFontVariationAxesInput::Invalid)
        } else {
            pillow_rs::ImageFontVariationAxesInput::Invalid
        };
        pillow_rs::imagefont_set_variation_by_axes_input(&mut self.inner, input).map_err(map_error)
    }

    fn setvaraxes(&mut self, axes: Vec<f32>) -> PyResult<()> {
        pillow_rs::imagefont_native_setvaraxes(&mut self.inner, &axes).map_err(map_error)
    }

    #[pyo3(signature = (size=None))]
    fn font_variant(&self, size: Option<f32>) -> PyResult<Self> {
        pillow_rs::imagefont_variant(&self.inner, size)
            .map(|inner| PyFont { inner })
            .map_err(map_error)
    }

    #[pyo3(signature = (font_bytes=None, size=None, index=None, encoding=None, layout_engine=None))]
    fn font_variant_with_options(
        &self,
        font_bytes: Option<Vec<u8>>,
        size: Option<f32>,
        index: Option<usize>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let options = pillow_rs::ImageFontVariantOptions {
            font_bytes,
            size,
            index,
            encoding,
            layout_engine,
        };
        pillow_rs::imagefont_variant_with_options(&self.inner, &options)
            .map(|inner| PyFont { inner })
            .map_err(map_error)
    }

    fn get_name(&self) -> (Option<String>, Option<String>) {
        let (family, style) = pillow_rs::imagefont_getname_optional(&self.inner);
        (family.map(ToOwned::to_owned), style.map(ToOwned::to_owned))
    }

    fn get_size(&self) -> f32 {
        pillow_rs::imagefont_size(&self.inner)
    }
}

#[pyclass(name = "PilFont", unsendable)]
pub struct PyPilFont {
    inner: pillow_rs::PilFont,
    file: Option<String>,
}

#[pymethods]
impl PyPilFont {
    #[staticmethod]
    fn load(filename: &str) -> PyResult<Self> {
        let (inner, file) =
            load_pilfont_from_path(std::path::Path::new(filename)).map_err(map_error)?;
        Ok(Self {
            inner,
            file: Some(file),
        })
    }

    #[staticmethod]
    fn load_path(py: Python<'_>, filename: &str) -> PyResult<Self> {
        let directories = py
            .import("sys")?
            .getattr("path")?
            .extract::<Vec<String>>()?;
        for directory in directories {
            let path = std::path::Path::new(&directory).join(filename);
            match load_pilfont_from_path(&path) {
                Ok((inner, file)) => {
                    return Ok(Self {
                        inner,
                        file: Some(file),
                    });
                }
                Err(error) if pilfont_load_path_catches(&error) => {}
                Err(error) => return Err(map_error(error)),
            }
        }

        let mut message = format!("cannot find font file \"{filename}\" in sys.path");
        if std::path::Path::new(filename).exists() {
            message.push_str(&format!(
                ", did you mean ImageFont.load(\"{filename}\") instead?"
            ));
        }
        Err(pyo3::exceptions::PyOSError::new_err(message))
    }

    #[staticmethod]
    fn load_default() -> PyResult<Self> {
        let inner = pillow_rs::PilFont::load_default().map_err(map_error)?;
        Ok(Self { inner, file: None })
    }

    #[getter]
    fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    #[getter]
    fn info(&self, py: Python<'_>) -> Vec<Py<PyBytes>> {
        self.inner
            .info()
            .iter()
            .map(|line| PyBytes::new(py, line).unbind())
            .collect()
    }

    fn getsize(&self, text: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
        self.inner
            .getsize_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    fn getbbox(&self, text: &Bound<'_, PyAny>) -> PyResult<(i32, i32, i32, i32)> {
        self.inner
            .getbbox_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    fn getlength(&self, text: &Bound<'_, PyAny>) -> PyResult<i32> {
        self.inner
            .getlength_input(pilfont_text_input_from_python(text)?)
            .map_err(map_error)
    }

    #[pyo3(signature = (text, mode=""))]
    fn getmask(&self, text: &Bound<'_, PyAny>, mode: &str) -> PyResult<PyImage> {
        let _ = mode;
        let image = self
            .inner
            .getmask_input(pilfont_text_input_from_python(text)?)
            .and_then(|mask| mask.to_image())
            .map_err(map_error)?;
        Ok(PyImage { inner: image })
    }
}

fn load_pilfont_from_path(
    path: &std::path::Path,
) -> Result<(pillow_rs::PilFont, String), PilError> {
    let metrics = std::fs::read(path).map_err(PilError::from)?;
    let root = path.with_extension("");

    for extension in [".png", ".gif", ".pbm"] {
        let mut candidate_name = root.as_os_str().to_os_string();
        candidate_name.push(extension);
        let candidate = std::path::PathBuf::from(candidate_name);
        let Ok(bitmap) = std::fs::read(&candidate) else {
            continue;
        };
        let Ok(image) = pillow_rs::PilFont::open_pilfont_glyph_image(bitmap) else {
            continue;
        };
        match pilfont_from_glyph_image(&metrics, image) {
            Ok(font) => return Ok((font, candidate.to_string_lossy().into_owned())),
            Err(PilError::TypeError(message)) if message == "invalid font image mode" => continue,
            Err(error) => return Err(error),
        }
    }

    Err(PilError::OsError(format!(
        "cannot find glyph data file {}.{{gif|pbm|png}}",
        root.display()
    )))
}

fn pilfont_from_glyph_image(
    metrics: &[u8],
    image: pillow_rs::PilFontGlyphImage,
) -> Result<pillow_rs::PilFont, PilError> {
    match image {
        pillow_rs::PilFontGlyphImage::Image(image) => {
            pillow_rs::PilFont::from_pilfont_data(metrics, image)
        }
        deferred @ pillow_rs::PilFontGlyphImage::DeferredRenderError { .. } => {
            pillow_rs::PilFont::from_pilfont_glyph_data(metrics, deferred)
        }
    }
}

fn pilfont_load_path_catches(error: &PilError) -> bool {
    matches!(
        error,
        PilError::IOError(_)
            | PilError::OsError(_)
            | PilError::UnidentifiedImageError(_)
            | PilError::ImageError(_)
            | PilError::Io(_)
    )
}

// --- ImageDraw ---

#[pyclass(name = "Outline")]
pub struct PyOutline {
    points: Vec<(i32, i32)>,
}

#[pymethods]
impl PyOutline {
    #[new]
    fn new() -> Self {
        Self { points: Vec::new() }
    }

    #[pyo3(name = "move")]
    fn move_to(&mut self, x: i32, y: i32) {
        self.points.clear();
        self.points.push((x, y));
    }

    fn line(&mut self, x: i32, y: i32) {
        self.points.push((x, y));
    }

    fn curve(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> PyResult<()> {
        let (x0, y0) = self.points.last().copied().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("outline has no current point")
        })?;
        self.points.extend(pillow_rs::outline_curve_points(
            &[x0 as f64, y0 as f64, x1, y1, x2, y2, x3, y3],
            32,
        ));
        Ok(())
    }

    fn close(&mut self) {
        if self.points.len() > 2 && self.points.first() != self.points.last() {
            self.points.push(self.points[0]);
        }
    }
}

#[pyclass(name = "ImageDraw")]
pub struct PyDraw {
    draw: pillow_rs::Draw,
}

fn draw_points_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawPointsInput {
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

fn draw_box_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawBoxInput {
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawBoxInput::Nested(points);
    }
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawBoxInput::Flat(values);
    }
    pillow_rs::DrawBoxInput::Invalid
}

fn draw_circle_center_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawCircleCenterInput {
    if xy.is_instance_of::<PyInt>() && !xy.is_instance_of::<PyBool>() {
        return pillow_rs::DrawCircleCenterInput::Integer;
    }
    if xy.is_instance_of::<PyDict>() {
        return pillow_rs::DrawCircleCenterInput::Mapping;
    }
    if xy.extract::<String>().is_ok() {
        return pillow_rs::DrawCircleCenterInput::Text;
    }
    xy.extract::<Vec<f64>>()
        .map(pillow_rs::DrawCircleCenterInput::Values)
        .unwrap_or(pillow_rs::DrawCircleCenterInput::Invalid)
}

fn draw_color_input_from_python(
    val: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::DrawColorInput> {
    let Some(val) = val else {
        return Ok(pillow_rs::DrawColorInput::None);
    };
    if let Ok(value) = val.extract::<String>() {
        return Ok(pillow_rs::DrawColorInput::String(value));
    }
    if let Ok(value) = val.extract::<i64>() {
        return Ok(pillow_rs::DrawColorInput::Integer(value));
    }
    if let Ok(value) = val.extract::<f64>() {
        return Ok(pillow_rs::DrawColorInput::Float(value));
    }
    if let Ok(value) = val.extract::<Vec<i64>>() {
        return Ok(pillow_rs::DrawColorInput::Components(value));
    }
    Ok(pillow_rs::DrawColorInput::Invalid)
}

fn draw_features_from_python(features: Option<&Bound<'_, PyAny>>) -> (Option<Vec<String>>, bool) {
    let Some(features) = features else {
        return (None, false);
    };
    match features.extract::<Vec<String>>() {
        Ok(values) => (Some(values), false),
        Err(_) => (None, true),
    }
}

#[pymethods]
impl PyDraw {
    #[new]
    #[pyo3(signature = (image, mode=None))]
    fn new(image: &Bound<'_, PyImage>, mode: Option<String>) -> PyResult<Self> {
        let borrowed = image.borrow();
        let draw = pillow_rs::Draw::new(borrowed.inner.clone(), mode);
        draw.validate_mode().map_err(map_error)?;
        Ok(PyDraw { draw })
    }

    fn line(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let points = draw_points_input_from_python(xy);
        let w = pillow_rs::normalize_draw_width(width);
        self.draw
            .polyline_with_input(points, color, w)
            .map_err(map_error)
    }

    fn rectangle(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .rectangle(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn ellipse(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .ellipse(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn bitmap(
        &mut self,
        xy: (f64, f64),
        bitmap: &Bound<'_, PyImage>,
        fill: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let fill_input = draw_color_input_from_python(fill)?;
        let bmp = bitmap.borrow();
        self.draw
            .bitmap_with_input(xy.0 as i32, xy.1 as i32, &bmp.inner, fill_input)
            .map_err(map_error)
    }

    fn regular_polygon(
        &mut self,
        bounding_circle: &Bound<'_, PyAny>,
        n_sides: &Bound<'_, PyAny>,
        rotation: Option<f64>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let circle = if let Ok((x, y, radius)) = bounding_circle.extract::<(f64, f64, f64)>() {
            pillow_rs::RegularPolygonCircle::Flat(x, y, radius)
        } else if let Ok(((x, y), radius)) = bounding_circle.extract::<((f64, f64), f64)>() {
            pillow_rs::RegularPolygonCircle::Nested(x, y, radius)
        } else {
            pillow_rs::RegularPolygonCircle::Invalid
        };
        let sides = n_sides
            .extract::<i64>()
            .map(pillow_rs::RegularPolygonSides::Value)
            .unwrap_or(pillow_rs::RegularPolygonSides::Invalid);
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        self.draw
            .regular_polygon(
                circle,
                sides,
                rotation.unwrap_or(0.0),
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn polygon(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        self.draw
            .polygon_with_input(
                draw_points_input_from_python(xy),
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn point(&mut self, xy: &Bound<'_, PyAny>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = self.color(fill)?;
        self.draw
            .point_with_input(draw_points_input_from_python(xy), color)
            .map_err(map_error)
    }

    fn shape(
        &mut self,
        shape: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let points = if let Ok(mut rust_outline) = shape.extract::<PyRefMut<'_, PyOutline>>() {
            rust_outline.close();
            rust_outline.points.clone()
        } else {
            let input = draw_points_input_from_python(shape);
            let fill = fill.map(|_| self.color(fill)).transpose()?;
            let outline = outline.map(|_| self.color(outline)).transpose()?;
            return self
                .draw
                .shape_with_input(input, fill, outline)
                .map_err(map_error);
        };
        let fill = fill.map(|_| self.color(fill)).transpose()?;
        let outline = outline.map(|_| self.color(outline)).transpose()?;
        self.draw.shape(&points, fill, outline).map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, width=1))]
    fn arc(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .arc(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn chord(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .chord(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn pieslice(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .pieslice(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius, fill=None, outline=None, width=1))]
    fn circle(
        &mut self,
        xy: &Bound<'_, PyAny>,
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        self.draw
            .circle_with_input(
                draw_circle_center_input_from_python(xy),
                radius,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius=0.0, fill=None, outline=None, width=1))]
    fn rounded_rectangle(
        &mut self,
        xy: &Bound<'_, PyAny>,
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        self.draw
            .rounded_rectangle_with_input(
                draw_box_input_from_python(xy),
                radius,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false))]
    fn text(
        &mut self,
        xy: (f64, f64),
        text: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
    ) -> PyResult<()> {
        let color = self.text_color(fill)?;
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let borrowed = font.map(|font| font.borrow());
        self.draw
            .text_with_optional_font_input(
                xy.0 as i32,
                xy.1 as i32,
                imagefont_text_input_from_python(text)?,
                borrowed.as_ref().map(|font| &font.inner),
                color,
                None,
                &options,
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, spacing=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    fn multiline_text(
        &mut self,
        xy: (f64, f64),
        text: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: Option<i32>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<()> {
        let color = self.text_color(fill)?;
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let borrowed = font.map(|font| font.borrow());
        self.draw
            .multiline_text_with_optional_font_input(
                xy.0,
                xy.1,
                imagefont_text_input_from_python(text)?,
                borrowed.as_ref().map(|font| &font.inner),
                color,
                f64::from(spacing.unwrap_or(4)),
                font_size,
                &options,
            )
            .map_err(map_error)
    }

    /// Compute text bounding box. Loads default FreeType font if font is None.
    /// Returns (left, top, right, bottom).
    #[pyo3(signature = (xy, text, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    fn textbbox(
        &mut self,
        xy: (i32, i32),
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_textbbox_at_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            xy,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)
    }

    /// Compute text length in pixels. Loads default FreeType font if font is None.
    #[pyo3(signature = (text, font=None, direction=None, features=None, language=None, embedded_color=false, font_size=None))]
    fn textlength(
        &mut self,
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<f64> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_getlength_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map(|width| width as f64)
        .map_err(map_error)
    }

    /// Compute bounding box for multiline text. Matches PIL's exact algorithm.
    #[pyo3(signature = (xy, text, font=None, spacing=4, align="left", direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    fn multiline_textbbox(
        &mut self,
        xy: (i32, i32),
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: i32,
        align: &str,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_multiline_textbbox_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            xy,
            imagefont_text_input_from_python(text)?,
            spacing,
            align,
            &options,
        )
        .map_err(map_error)
    }

    #[getter]
    fn image(&self) -> PyResult<PyImage> {
        // Return a copy of the current image state
        Ok(PyImage {
            inner: self.draw_get_image()?,
        })
    }
}

impl PyDraw {
    fn draw_get_image(&self) -> PyResult<pillow_rs::Image> {
        self.draw.image_clone().map_err(map_error)
    }

    /// Parse a draw color, using the image mode to determine byte representation.
    fn color(&self, val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
        self.draw
            .color_with_input(draw_color_input_from_python(val)?)
            .map_err(map_error)
    }

    /// Parse an ImageDraw text ink, including Pillow's mode-specific default.
    fn text_color(&self, val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
        self.draw
            .text_color_with_input(draw_color_input_from_python(val)?)
            .map_err(map_error)
    }
}

// --- ImageOps module-level functions ---

#[pyfunction]
fn ops_autocontrast(
    image: &Bound<'_, PyImage>,
    cutoff: Option<f64>,
    mask: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let c = cutoff.unwrap_or(0.0);
    let mask = imageops_mask_from_python(mask)?;
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_autocontrast_with_mask(&inner, c, mask))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>, mask: Option<&Bound<'_, PyAny>>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let mask = imageops_mask_from_python(mask)?;
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_equalize_with_mask(&inner, mask))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_invert(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_flip(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_flip(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_mirror(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_mirror(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_posterize(image: &Bound<'_, PyImage>, bits: u8) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs =
        Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_posterize(&inner, bits)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_solarize(image: &Bound<'_, PyImage>, threshold: Option<u8>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let t = threshold.unwrap_or(128);
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_solarize(&inner, t)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_grayscale(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_colorize(
    image: &Bound<'_, PyImage>,
    black: &Bound<'_, PyAny>,
    white: &Bound<'_, PyAny>,
    mid: Option<&Bound<'_, PyAny>>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    pillow_rs::imageops_validate_colorize_mode(&inner).map_err(map_error)?;
    let black = parse_colorize_color(black)?;
    let white = parse_colorize_color(white)?;
    let mid = mid.map(parse_colorize_color).transpose()?;
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| {
            pillow_rs::imageops_colorize(
                &inner, black, white, mid, blackpoint, midpoint, whitepoint,
            )
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

fn parse_colorize_color(value: &Bound<'_, PyAny>) -> PyResult<(u8, u8, u8)> {
    if let Ok(color) = value.extract::<String>() {
        let (r, g, b, _) = pillow_rs::parse_color_str(&color).map_err(map_error)?;
        return Ok((r, g, b));
    }
    if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
        return Ok((r, g, b));
    }
    if let Ok((r, g, b, _)) = value.extract::<(u8, u8, u8, u8)>() {
        return Ok((r, g, b));
    }
    if value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("'int' object is not subscriptable"));
    }
    Err(PyTypeError::new_err("color must be a color name or tuple"))
}

#[pyfunction]
fn ops_contain(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_contain_with_input(&inner, size.0, size.1, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_cover(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_cover_with_input(&inner, size.0, size.1, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_fit(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
    bleed: Option<f64>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let centering = centering_from_python(Some(centering));
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| {
            pillow_rs::imageops_fit_with_input(
                &inner,
                size.0,
                size.1,
                filter,
                bleed.unwrap_or(0.0),
                centering,
            )
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_pad(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
    color: Option<&Bound<'_, PyAny>>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let centering = centering_from_python(Some(centering));
    let color = imageops_color_from_python(color);

    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| {
            pillow_rs::imageops_pad_with_input(&inner, size.0, size.1, filter, color, centering)
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_scale(
    image: &Bound<'_, PyImage>,
    factor: f64,
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_scale_with_input(&inner, factor, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_expand(
    image: &Bound<'_, PyImage>,
    border: &Bound<'_, PyAny>,
    fill: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    // Resolve border: int -> use as u32, 4-tuple -> max
    let border_val: u32 = if let Ok(i) = border.extract::<u32>() {
        i
    } else if let Ok((t, r, b, l)) = border.extract::<(u32, u32, u32, u32)>() {
        t.max(r).max(b).max(l)
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "border must be int or 4-tuple",
        ));
    };

    // Resolve fill: int -> (v, 0, 0, 0), 2-tuple -> (v, v, v, alpha),
    // 3-tuple -> (v0, v1, v2, 0), 4-tuple as-is. The pair form is the
    // public LA/PA fill representation; the core selects the native bands.
    let fill_val: (u8, u8, u8, u8) = if let Ok(i) = fill.extract::<u8>() {
        (i, 0, 0, 0)
    } else if let Ok((value, alpha)) = fill.extract::<(u8, u8)>() {
        (value, value, value, alpha)
    } else if let Ok((r, g, b)) = fill.extract::<(u8, u8, u8)>() {
        (r, g, b, 0)
    } else if let Ok((r, g, b, a)) = fill.extract::<(u8, u8, u8, u8)>() {
        (r, g, b, a)
    } else {
        (0, 0, 0, 0)
    };

    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_expand(&inner, border_val, fill_val))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_crop_border(image: &Bound<'_, PyImage>, border: u32) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_crop(&inner, border)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
#[pyo3(signature = (image, in_place=false))]
fn ops_exif_transpose(image: &Bound<'_, PyImage>, in_place: bool) -> PyResult<Option<PyImage>> {
    let inner = image.borrow().inner.clone();
    let result = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_exif_transpose(&inner, in_place))
    })
    .map_err(map_error)?;

    if in_place {
        if let Some(transposed) = result {
            image.borrow_mut().inner = transposed;
        }
        Ok(None)
    } else {
        Ok(result.map(|inner| PyImage { inner }))
    }
}

// --- ImageChops module-level functions ---

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_add(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs =
        Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_add(&b1, &b2, scale, offset)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_subtract(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::chops_subtract(&b1, &b2, scale, offset))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_multiply(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_multiply(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_screen(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_screen(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_darker(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_darker(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_lighter(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_lighter(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_difference(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_difference(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_duplicate(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    Ok(PyImage {
        inner: pillow_rs::chops_duplicate(&borrowed.inner),
    })
}

#[pyfunction]
fn chops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs::chops_invert(&borrowed.inner).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_add_modulo(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_add_modulo(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_subtract_modulo(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_subtract_modulo(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_constant(image: &Bound<'_, PyImage>, value: u8) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs::chops_constant(&b.inner, value)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_hard_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_hard_light(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_soft_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_soft_light(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_overlay(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_overlay(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_and(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_and(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_or(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_or(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_xor(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_xor(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_offset(
    image: &Bound<'_, PyImage>,
    xoffset: i32,
    yoffset: Option<i32>,
) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs::chops_offset_with_default(&b.inner, xoffset, yoffset)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

// --- Image module functions ---

#[pyfunction]
fn image_merge(mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
    let inputs = merge_inputs_from_python(bands)?;
    let rs = pillow_rs::image_merge_inputs(mode, &inputs).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_blend(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    alpha: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs::image_blend(&b1.inner, &b2.inner, alpha).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_composite(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    mask: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let bm = mask.borrow();
    let rs = pillow_rs::image_composite(&b1.inner, &b2.inner, &bm.inner).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_alpha_composite(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::image_alpha_composite(&b1.inner, &b2.inner)
        .map(|image| PyImage { inner: image })
        .map_err(map_error)
}

/// Generate a 256×256 linear gradient image from black to white.
#[pyfunction]
fn image_linear_gradient(mode: &str) -> PyResult<PyImage> {
    let rs = pillow_rs::image_linear_gradient(mode).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a 256×256 radial gradient image from white (center) to black (edges).
#[pyfunction]
fn image_radial_gradient(mode: &str) -> PyResult<PyImage> {
    let rs = pillow_rs::image_radial_gradient(mode).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a Mandelbrot set image.
#[pyfunction]
fn image_effect_mandelbrot(
    size: (u32, u32),
    extent: &Bound<'_, PyAny>,
    quality: i32,
) -> PyResult<PyImage> {
    let extent_type = extent.get_type().name()?.to_string();
    let extent = extent.extract::<Vec<f64>>().ok();
    let rs = pillow_rs::image_effect_mandelbrot_with_extent(
        size,
        extent.as_deref(),
        &extent_type,
        quality,
    )
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

// --- ImageColor ---

#[pyfunction]
fn getrgb(color: &str) -> PyResult<PyObject> {
    let (r, g, b, a) = pillow_rs::parse_color_str_unclamped(color).map_err(map_error)?;
    Python::with_gil(|py| {
        if pillow_rs::color_has_explicit_alpha(color) {
            Ok((r, g, b, a).to_object(py))
        } else {
            Ok((r, g, b).to_object(py))
        }
    })
}

/// PIL-compatible getcolor: search palette for (r,g,b[,a]), append if new. Returns index.
#[pyfunction]
fn palette_getcolor_append(
    palette: Vec<u8>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mode: &str,
) -> PyResult<usize> {
    let mut pal = palette;
    pillow_rs::palette_getcolor_append(&mut pal, r, g, b, a, mode).map_err(PyValueError::new_err)
}

/// Format palette as PIL-compatible text (header + 256-entry table).
#[pyfunction]
fn palette_to_text(palette: Vec<u8>, mode: &str) -> String {
    pillow_rs::palette_to_text(&palette, mode)
}

#[pyfunction]
fn getcolor(color: &str, mode: &str) -> PyResult<PyObject> {
    let (r, g, b, a) = pillow_rs::parse_color_str_unclamped(color).map_err(map_error)?;
    let result = pillow_rs::getcolor(r, g, b, a, mode).map_err(map_error)?;
    Python::with_gil(|py| match result {
        pillow_rs::ColorValue::Gray(value) => Ok(value.to_object(py)),
        pillow_rs::ColorValue::GrayAlpha(gray, alpha) => Ok((gray, alpha).to_object(py)),
        pillow_rs::ColorValue::Rgb(r, g, b) => Ok((r, g, b).to_object(py)),
        pillow_rs::ColorValue::Rgba(r, g, b, a) => Ok((r, g, b, a).to_object(py)),
        pillow_rs::ColorValue::Hsv(h, s, v) => Ok((h, s, v).to_object(py)),
    })
}

/// Validate a color for palette mode, append it, and return the
/// updated palette and the color index.
/// Handles mode-specific logic: RGB mode rejects non-opaque RGBA,
/// RGBA mode auto-fills missing alpha to 255.
#[pyfunction]
fn palette_getcolor_validate(
    palette: Vec<u8>,
    color: &Bound<'_, PyAny>,
    mode: &str,
) -> PyResult<(Vec<u8>, usize)> {
    let mut pal = palette;
    let repr = color.repr()?.to_string();
    let input = if color.downcast::<PyTuple>().is_ok() || color.downcast::<PyList>().is_ok() {
        color
            .extract::<Vec<u8>>()
            .map(pillow_rs::PaletteColorInput::Components)
            .unwrap_or(pillow_rs::PaletteColorInput::Invalid(repr))
    } else {
        pillow_rs::PaletteColorInput::Invalid(repr)
    };
    let idx = pillow_rs::palette_getcolor_validate_input(&mut pal, input, mode)
        .map_err(PyValueError::new_err)?;
    Ok((pal, idx))
}

/// Save palette data to a text file.
#[pyfunction]
fn palette_save_to_file(palette: Vec<u8>, mode: &str, fp: &str) -> PyResult<()> {
    let text = pillow_rs::palette_to_text(&palette, mode);
    std::fs::write(fp, text).map_err(|error| {
        pyo3::exceptions::PyOSError::new_err(format!("Cannot write palette file: {error}"))
    })
}

/// Compute Pillow ImageStat values from a precomputed histogram.
#[pyfunction]
fn stat_from_histogram(data: Vec<f64>) -> PyResult<PyObject> {
    let result = pillow_rs::stat_from_histogram(&data);
    stat_result_to_python(&result)
}

// --- ImageFilter helper functions ---

/// Validate and normalize 3D LUT size. Accepts int (converted to 3-tuple)
/// or sequence of 3 ints. Each dimension must be in [2, 65].
#[pyfunction]
fn color3dlut_check_size(size: &Bound<'_, PyAny>) -> PyResult<(u32, u32, u32)> {
    let values = match size.extract::<Vec<f64>>() {
        Ok(values) => values,
        Err(_) => vec![size.extract::<f64>().map_err(|_| {
            PyValueError::new_err("Size should be either an integer or a tuple of three integers.")
        })?],
    };
    pillow_rs::color3dlut_check_size(&values).map_err(map_error)
}

/// Validate and flatten a 3D LUT table from Python object.
/// Handles flat list of floats and list of tuples.
/// Always returns a flat Vec<f64> of validated length.
#[pyfunction]
#[pyo3(signature = (table_obj, size, channels=3, copy_table=true))]
fn color3dlut_new(
    table_obj: &Bound<'_, PyAny>,
    size: (u32, u32, u32),
    channels: u32,
    copy_table: bool,
) -> PyResult<Vec<f64>> {
    let _ = copy_table;
    let table = if let Ok(table) = table_obj.extract::<Vec<Vec<f64>>>() {
        pillow_rs::Color3DLutTable::Nested(table)
    } else {
        pillow_rs::Color3DLutTable::Flat(table_obj.extract::<Vec<f64>>().map_err(|_| {
            PyValueError::new_err(
                "Table must be a sequence of floats or a sequence of tuples of floats.",
            )
        })?)
    };
    pillow_rs::color3dlut_prepare_table(table, size, channels).map_err(map_error)
}

/// Generate a 3D LUT table by calling a Python callback for each position.
/// The callback receives (r, g, b) normalized to [0, 1] and returns a tuple
/// of `channels` floats.
#[pyfunction]
fn color3dlut_generate(
    size: (u32, u32, u32),
    channels: u32,
    callback: PyObject,
    py: Python,
) -> PyResult<Vec<f64>> {
    pillow_rs::color3dlut_generate_table(
        size,
        channels,
        |args| {
            let point = PyTuple::new(py, args)?;
            callback.call(py, &point, None)?.extract(py)
        },
        map_error,
    )
}

/// Transform a 3D LUT table by calling a Python callback for each entry.
/// If `with_normals` is true, the callback receives (r_norm, g_norm, b_norm, *values).
/// Otherwise, receives (*values).
/// The callback returns a tuple of `channels_out` floats.
#[pyfunction]
fn color3dlut_transform(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels_in: u32,
    channels_out: Option<u32>,
    with_normals: bool,
    callback: PyObject,
    py: Python,
) -> PyResult<(Vec<f64>, u32)> {
    let input = pillow_rs::prepare_color3dlut(table, size, channels_in).map_err(map_error)?;
    pillow_rs::color3dlut_transform_table(
        &input,
        channels_out,
        with_normals,
        |args| {
            let point = PyTuple::new(py, args)?;
            callback.call(py, &point, None)?.extract(py)
        },
        map_error,
    )
}

#[pyfunction]
fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    pillow_rs::color3dlut_repr(table_type, size, channels, target_mode)
}

#[pyfunction]
fn kernel_validate_coefficients(kernel: Option<Vec<f64>>, size: (u32, u32)) -> PyResult<()> {
    pillow_rs::validate_kernel_coefficients(kernel.as_deref(), size).map_err(map_error)
}
