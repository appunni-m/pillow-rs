// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::redundant_clone)]

use pillow_rs::PilError;
use pillow_rs::{Image as RsImage, PutDataValue};
use pyo3::ToPyObject;
use pyo3::exceptions::{PySystemError, PyTypeError, PyValueError};
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyObject;
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
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFloat;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyModuleMethods;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use pyo3::types::PyType;
use pyo3::types::PyTypeMethods;
use pyo3::wrap_pyfunction;
use std::path::PathBuf;

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
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

fn map_open_path_error(path: &std::path::Path, error: std::io::Error) -> PyErr {
    if error.kind() == std::io::ErrorKind::NotFound {
        return pyo3::exceptions::PyFileNotFoundError::new_err((
            error.raw_os_error().unwrap_or(2),
            "No such file or directory",
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
) -> pillow_rs::RotateResampleInput {
    match value {
        None => pillow_rs::RotateResampleInput::None,
        Some(value) => value
            .extract::<String>()
            .map(pillow_rs::RotateResampleInput::Name)
            .unwrap_or(pillow_rs::RotateResampleInput::Other),
    }
}

fn rotate_expand_input_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> pillow_rs::RotateExpandInput {
    match value {
        Some(value) if value.is_instance_of::<PyBool>() => value
            .extract::<bool>()
            .map(pillow_rs::RotateExpandInput::Boolean)
            .unwrap_or(pillow_rs::RotateExpandInput::Invalid),
        None => pillow_rs::RotateExpandInput::Boolean(false),
        Some(_) => pillow_rs::RotateExpandInput::Invalid,
    }
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
        let mesh = mesh
            .into_iter()
            .map(|item| {
                if item.len() == 2 {
                    Ok((item[0].clone(), item[1].clone()))
                } else {
                    Err(PyTypeError::new_err(
                        "mesh entries must contain a bbox and quad",
                    ))
                }
            })
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(Some(pillow_rs::TransformData::Mesh(mesh)));
    }
    Err(PyTypeError::new_err("transform data must be a sequence"))
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
    Ok(Some(pillow_rs::TransformFill::Invalid))
}

fn reduce_factor_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::ReduceFactor {
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::ReduceFactor::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::ReduceFactor::Sequence(values);
    }
    pillow_rs::ReduceFactor::Invalid
}

fn reduce_box_from_python(value: Option<&Bound<'_, PyAny>>) -> pillow_rs::ReduceBox {
    let Some(value) = value else {
        return pillow_rs::ReduceBox::Invalid;
    };
    value
        .extract::<Vec<i64>>()
        .map(pillow_rs::ReduceBox::Sequence)
        .unwrap_or(pillow_rs::ReduceBox::Invalid)
}

fn centering_from_python(value: Option<&Bound<'_, PyAny>>) -> pillow_rs::CenteringInput {
    let Some(value) = value else {
        return pillow_rs::CenteringInput::Default;
    };
    if let Ok(value) = value.extract::<f64>() {
        return pillow_rs::CenteringInput::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<f64>>() {
        if values == [0.5, 0.5] {
            return pillow_rs::CenteringInput::Default;
        }
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

fn paste_source_from_python(value: &Bound<'_, PyAny>) -> pillow_rs::PythonPasteSource {
    if let Some(image) = image_from_python(value) {
        return pillow_rs::PythonPasteSource::Image(image);
    }
    if let Ok(value) = value.extract::<i64>() {
        return pillow_rs::PythonPasteSource::Scalar(value);
    }
    if let Ok(values) = value.extract::<Vec<i64>>() {
        return pillow_rs::PythonPasteSource::Components(values);
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

fn imageops_mask_from_python(value: Option<&Bound<'_, PyAny>>) -> pillow_rs::ImageOpsMask {
    let Some(value) = value else {
        return pillow_rs::ImageOpsMask::None;
    };
    if let Some(mask) = image_from_python(value) {
        return pillow_rs::ImageOpsMask::Image(mask);
    }
    pillow_rs::ImageOpsMask::Invalid
}

fn image_analysis_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> pillow_rs::ImageAnalysisMask {
    let Some(value) = value else {
        return pillow_rs::ImageAnalysisMask::None;
    };
    if let Some(mask) = image_from_python(value) {
        return pillow_rs::ImageAnalysisMask::Image(mask);
    }
    pillow_rs::ImageAnalysisMask::Invalid
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

#[allow(unsafe_code)]
fn python_is_sequence(value: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `Bound` guarantees a non-null, GIL-bound borrowed pointer for
    // this call. `PySequence_Check` only inspects the object's type slots and
    // neither steals a reference nor stores the pointer.
    unsafe { pyo3::ffi::PySequence_Check(value.as_ptr()) != 0 }
}

fn putdata_value_from_python(value: &Bound<'_, PyAny>, mode: &str) -> PyResult<PutDataValue> {
    if matches!(
        mode,
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
    ) {
        if python_is_sequence(value) {
            // Preserve the shape distinction for the core's canonical
            // "sequence must be flattened" error instead of terminating in
            // the binding before putdata_bytes sees the value.
            return Ok(PutDataValue::Components(Vec::new()));
        }
        // Pillow's numeric `_putdata` path deliberately clears conversion
        // errors after writing the sentinel returned by PyFloat_AsDouble.
        return Ok(PutDataValue::Number(value.extract::<f64>().unwrap_or(-1.0)));
    }

    if value.is_instance_of::<PyInt>() {
        return value.extract::<i64>().map(PutDataValue::Packed);
    }

    // Multiband Pillow putdata rejects scalar floats through the same shape
    // validation as other non-tuple values. Preserve the numeric distinction
    // for the core so it owns the public error contract instead of the
    // binding short-circuiting that path.
    if value.is_instance_of::<PyFloat>() {
        return value.extract::<f64>().map(PutDataValue::Number);
    }

    let Ok(tuple) = value.downcast::<PyTuple>() else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let tuple_len = tuple.len();
    if tuple_len == 1 {
        let packed = tuple.get_item(0)?;
        if packed.is_instance_of::<PyInt>() {
            return packed.extract::<i64>().map(PutDataValue::Packed);
        }
        if matches!(mode, "LA" | "PA") {
            return Err(PySystemError::new_err(
                "new style getargs format but argument is not a tuple",
            ));
        }
        return Err(PyTypeError::new_err(
            "color must be int, or tuple of one, three or four elements",
        ));
    }

    // Keep tuple extraction in the binding, but let the core own arity
    // validation.  That preserves one canonical error path for every backend
    // and lets public parity inputs exercise the same shape checks as the
    // Rust API.
    let mut components = Vec::with_capacity(tuple_len);
    for (index, item) in tuple.iter().enumerate() {
        let component = if index == 0 {
            item.extract::<i64>()? as i128
        } else {
            item.extract::<i32>()? as i128
        };
        components.push(component);
    }
    Ok(PutDataValue::Components(components))
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

    #[classmethod]
    #[pyo3(signature = (fp, formats=None))]
    fn open(
        _cls: &Bound<'_, PyType>,
        fp: &Bound<'_, PyAny>,
        formats: Option<Vec<String>>,
    ) -> PyResult<Self> {
        let format_refs = formats
            .as_deref()
            .map(|formats| formats.iter().map(String::as_str).collect::<Vec<_>>());
        if let Some(path) = host_path_from_python(fp)? {
            let bytes = std::fs::read(&path).map_err(|error| map_open_path_error(&path, error))?;
            let img = RsImage::open_bytes_with_formats(bytes, format_refs.as_deref())
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        } else {
            let bytes = fp.call_method0("read")?.extract::<Vec<u8>>()?;
            let img = RsImage::open_bytes_with_formats(bytes, format_refs.as_deref())
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        }
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

    #[classmethod]
    fn validate_open_source(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(bytes) = fp.downcast::<PyBytes>() {
            pillow_rs::validate_python_open_source_bytes(bytes.as_bytes()).map_err(map_error)?;
        }
        Ok(())
    }

    fn save(&mut self, fp: &Bound<'_, PyAny>, format: Option<String>) -> PyResult<()> {
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
        let encoded = self.inner.encode(&format).map_err(map_error)?;
        if let Some(path) = path {
            std::fs::write(path, encoded).map_err(|error| map_error(error.into()))
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
    ) -> PyResult<PyImage> {
        let resample = resample_input_from_python(resample)?;
        let rs = self
            .inner
            .resize(size, resample, box_coords)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: Option<(i32, i32, i32, i32)>) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(map_error)?;
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
    ) -> PyResult<PyImage> {
        let _ = (center, translate, fillcolor);
        let rs = self
            .inner
            .rotate_with_input(
                angle,
                rotate_resample_input_from_python(resample),
                rotate_expand_input_from_python(expand),
                None,
            )
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &Bound<'_, PyAny>) -> PyResult<PyImage> {
        let input = if let Ok(value) = method.extract::<i64>() {
            pillow_rs::TransposeInput::Index(value)
        } else if let Ok(value) = method.extract::<String>() {
            pillow_rs::TransposeInput::Name(value)
        } else {
            pillow_rs::TransposeInput::Invalid(method.get_type().name()?.to_string())
        };
        let rs = self.inner.transpose_with_input(input).map_err(map_error)?;
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
        let dither = pillow_rs::normalize_python_convert_dither(dither).map_err(map_error)?;
        let rs = self
            .inner
            .convert_with_input(
                convert_mode_input_from_python(mode)?,
                matrix,
                dither.as_deref(),
                convert_palette_input_from_python(palette)?,
                colors,
            )
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        self.inner
            .paste_with_input(
                paste_source_from_python(im),
                paste_box_from_python(box_coords)?,
                paste_mask_from_python(mask)?,
            )
            .map_err(map_error)
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self.inner.filter(filter_type).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn validate_filter(&self, filter_name: &str) -> PyResult<()> {
        self.inner.validate_filter(filter_name).map_err(map_error)
    }

    fn kernel_filter(
        &self,
        kernel: Vec<f32>,
        scale: f32,
        offset: i32,
        size: u32,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .kernel_filter(&kernel, scale, offset, size)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes(&self) -> PyResult<Vec<u8>> {
        self.inner.tobytes().map_err(map_error)
    }

    fn tobytes_unpacked(&self) -> PyResult<Vec<u8>> {
        self.inner.tobytes_unpacked().map_err(map_error)
    }

    fn tobytes_formatted(&self, mode: &str) -> PyResult<Vec<u8>> {
        self.inner.tobytes_formatted(mode).map_err(map_error)
    }
    fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: Vec<String>,
    ) -> PyResult<Vec<u8>> {
        self.inner
            .tobytes_encoded(mode, encoder_name, &args)
            .map_err(map_error)
    }

    /// Return palette data (RGB triples) for P-mode quantized images.
    fn palette(&self) -> Option<Vec<u8>> {
        self.inner.palette()
    }
    /// Return palette trimmed of trailing zero triples, matching PIL's getpalette().
    fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_trimmed()
    }

    fn getpalette_rgba(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_rgba()
    }

    fn getpalette_rawmode(&self, rawmode: &str) -> PyResult<Option<Vec<u8>>> {
        self.inner.getpalette_rawmode(rawmode).map_err(map_error)
    }

    #[pyo3(signature = (rawmode=None))]
    fn getpalette_with_input(&self, rawmode: Option<String>) -> PyResult<Option<Vec<u8>>> {
        self.inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(map_error)
    }

    fn indexed_color_table(&self, mode: &str) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table(mode).map_err(map_error)
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    fn pending_transparency_index(&self) -> Option<u8> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Index(index)) => Some(index),
            _ => None,
        }
    }

    fn pending_transparency_table(&self) -> Option<Vec<u8>> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Table(alpha)) => Some(alpha),
            _ => None,
        }
    }

    fn converted_palette_transparency(&self, mode: &str) -> Option<Vec<u8>> {
        self.inner.converted_palette_transparency(mode)
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    fn apply_transparency(&mut self) -> PyResult<()> {
        self.inner.apply_transparency().map_err(map_error)
    }

    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(&mut self, data: Vec<u8>, rawmode: &str) -> PyResult<()> {
        self.inner.putpalette(&data, rawmode).map_err(map_error)
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

    fn getim(&self) -> PyResult<String> {
        // Pillow exposes an `Imaging` pointer in a named PyCapsule. A pointer to
        // this Rust wrapper is not ABI-compatible with `Imaging` and could make
        // capsule consumers dereference an invalid layout, so keep this endpoint
        // visibly unsupported until a genuine compatibility layer exists.
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "getim requires a Pillow Imaging-compatible capsule",
        ))
    }

    #[pyo3(signature = (size, resample=None))]
    fn thumbnail(&mut self, size: (i64, i64), resample: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let resample = resample_input_from_python(resample)?;
        self.inner.thumbnail(size, resample).map_err(map_error)
    }

    #[pyo3(signature = (colors=None, method=None, kmeans=None, dither=None, palette=None))]
    fn quantize(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        dither: Option<bool>,
        palette: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let palette = match palette {
            None => pillow_rs::QuantizePalette::None,
            Some(value) => image_from_python(value)
                .map(pillow_rs::QuantizePalette::Image)
                .unwrap_or(pillow_rs::QuantizePalette::Other),
        };
        let rs = self
            .inner
            .quantize_with_input(colors, method, kmeans, palette, dither)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getbbox(&self, alpha_only: Option<bool>) -> PyResult<Option<(u32, u32, u32, u32)>> {
        self.inner
            .getbbox(alpha_only.unwrap_or(true))
            .map_err(map_error)
    }

    fn getextrema(&self) -> PyResult<Vec<(u8, u8)>> {
        self.inner.getextrema().map_err(map_error)
    }
    /// Return extrema formatted as PIL expects.
    fn getextrema_formatted(&self) -> PyResult<PyObject> {
        let formatted = self.inner.getextrema_formatted().map_err(map_error)?;
        Python::with_gil(|py| match formatted {
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

    fn stat(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner.stat().map_err(map_error)
    }

    #[pyo3(signature = (mask=None))]
    fn stat_formatted(&self, mask: Option<&Bound<'_, PyAny>>) -> PyResult<PyObject> {
        let mask = imageops_mask_from_python(mask);
        let result = self
            .inner
            .stat_formatted_with_mask(mask)
            .map_err(map_error)?;
        stat_result_to_python(&result)
    }

    fn histogram(&self) -> PyResult<Vec<u32>> {
        self.inner.histogram().map_err(map_error)
    }

    #[pyo3(signature = (mask=None))]
    fn histogram_with_input(&self, mask: Option<&Bound<'_, PyAny>>) -> PyResult<Vec<u32>> {
        self.inner
            .histogram_with_input(image_analysis_mask_from_python(mask))
            .map_err(map_error)
    }

    fn histogram_with_mask(&self, mask: Option<&Bound<'_, PyImage>>) -> PyResult<Vec<u32>> {
        let mask_inner = mask.map(|m| m.borrow().inner.clone());
        self.inner
            .histogram_with_mask(mask_inner.as_ref())
            .map_err(map_error)
    }

    fn gaussian_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .gaussian_blur(radius.unwrap_or(2.0) as f32)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(
        &self,
        radius: Option<f64>,
        percent: Option<i32>,
        threshold: Option<u8>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .unsharp_mask(
                radius.unwrap_or(2.0) as f32,
                percent.unwrap_or(150),
                threshold.unwrap_or(3),
            )
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .max_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .min_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .median_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn box_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .box_blur(radius.unwrap_or(2.0) as f32)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn mode_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .mode_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn rank_filter(&self, size: Option<u32>, rank: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .rank_filter(size.unwrap_or(3), rank.unwrap_or(0))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color3dlut(
        &self,
        size: (u32, u32, u32),
        table: Vec<f64>,
        channels: Option<u32>,
        target_mode: Option<&str>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .color3dlut(size, table, channels.unwrap_or(3), target_mode)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: &Bound<'_, PyAny>) -> PyResult<PyImage> {
        let selector = if let Ok(channel) = channel.extract::<i32>() {
            pillow_rs::ChannelSelector::Index(channel)
        } else if let Ok(channel) = channel.extract::<String>() {
            pillow_rs::ChannelSelector::Name(channel)
        } else {
            pillow_rs::ChannelSelector::Invalid
        };
        let rs = self
            .inner
            .getchannel_selector(selector)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn load(&mut self) -> PyResult<()> {
        self.inner.load().map_err(map_error)
    }

    fn putalpha(&mut self, alpha: u8) -> PyResult<()> {
        self.inner.putalpha(alpha).map_err(map_error)
    }

    fn putalpha_input(&mut self, alpha: &Bound<'_, PyAny>) -> PyResult<()> {
        let input = if let Ok(mask) = alpha.downcast::<PyImage>() {
            pillow_rs::PutAlphaInput::Image(mask.borrow().inner.clone())
        } else if let Ok(value) = alpha.extract::<i64>() {
            pillow_rs::PutAlphaInput::Integer(value)
        } else {
            pillow_rs::PutAlphaInput::Invalid(alpha.get_type().name()?.to_string())
        };
        self.inner.putalpha_with_input(input).map_err(map_error)
    }

    fn putalpha_data(&mut self, mask: &Bound<'_, PyImage>) -> PyResult<()> {
        let mask_inner = mask.borrow().inner.clone();
        self.inner.putalpha_data(&mask_inner).map_err(map_error)
    }

    fn reduce(
        &self,
        factor: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let factor = reduce_factor_from_python(factor);
        let box_coords = box_coords.map(|value| reduce_box_from_python(Some(value)));
        let rs = self
            .inner
            .reduce_public(factor, box_coords)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, dest=None, source=None))]
    fn alpha_composite(
        &mut self,
        im: &Bound<'_, PyImage>,
        dest: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
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
        self.inner
            .alpha_composite_public(&source_image, dest, source_box)
            .map_err(map_error)
    }

    fn getcolors(&mut self, maxcolors: Option<u32>) -> PyResult<Option<Vec<(u32, Vec<u8>)>>> {
        self.inner
            .getcolors(maxcolors.unwrap_or(256))
            .map_err(map_error)
    }
    /// Return getcolors formatted as PIL expects.
    fn getcolors_formatted(&mut self, maxcolors: Option<u32>) -> PyResult<Option<PyObject>> {
        let formatted = self
            .inner
            .getcolors_formatted(maxcolors.unwrap_or(256))
            .map_err(map_error)?;
        Python::with_gil(|py| match formatted {
            None => Ok(None),
            Some(results) => {
                let out = pyo3::types::PyList::empty(py);
                for (count, color) in results {
                    let color_value = match color {
                        pillow_rs::FormattedPixelValue::Scalar(value) => value.to_object(py),
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

    fn getdata(&mut self, band: Option<i32>) -> PyResult<Vec<u8>> {
        self.inner.getdata(band).map_err(map_error)
    }
    /// Return getdata formatted as PIL expects.
    fn getdata_formatted(&mut self, band: Option<i32>) -> PyResult<PyObject> {
        let formatted = self.inner.getdata_formatted(band).map_err(map_error)?;
        Python::with_gil(|py| match formatted {
            pillow_rs::FormattedImageData::Scalars(values) if band.is_some() => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(value)?;
                }
                Ok(out.to_object(py))
            }
            pillow_rs::FormattedImageData::Scalars(values) => Ok(values.to_object(py)),
            pillow_rs::FormattedImageData::Components(values) => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(PyTuple::new(py, value)?)?;
                }
                Ok(out.to_object(py))
            }
        })
    }

    fn getprojection(&mut self) -> PyResult<(Vec<u32>, Vec<u32>)> {
        self.inner.getprojection().map_err(map_error)
    }

    fn entropy(&mut self) -> PyResult<f64> {
        self.inner.entropy().map_err(map_error)
    }

    #[pyo3(signature = (mask=None))]
    fn entropy_with_input(&mut self, mask: Option<&Bound<'_, PyAny>>) -> PyResult<f64> {
        self.inner
            .entropy_with_input(image_analysis_mask_from_python(mask))
            .map_err(map_error)
    }

    fn entropy_with_mask(&mut self, mask: Option<&Bound<'_, PyImage>>) -> PyResult<f64> {
        let mask_inner = mask.map(|m| m.borrow().inner.clone());
        self.inner
            .entropy_with_mask(mask_inner.as_ref())
            .map_err(map_error)
    }

    fn seek(&mut self, frame: u32) -> PyResult<()> {
        self.inner.seek(frame).map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    /// Pre-built LUT: must have exactly 256 * n_bands entries (PIL requirement).
    fn point(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    /// point() with band replication derived from the Rust image mode.
    fn point_replicated(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval_replicated_for_image(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    /// Validate pre-built LUT length before calling point.
    /// PIL: LUT must have exactly 256 * n_bands entries.
    fn point_validated(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval_validated(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn effect_spread(&self, distance: u32) -> PyResult<PyImage> {
        pillow_rs::image_effect_spread(&self.inner, distance)
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
    ) -> PyResult<PyImage> {
        let data = transform_data_from_python(data)?;
        let fillcolor = transform_fill_from_python(fillcolor)?;
        self.inner
            .transform_public(
                size,
                method,
                data,
                resample.unwrap_or(0),
                fill.unwrap_or(1),
                fillcolor,
            )
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
    ) -> PyResult<PyImage> {
        pillow_rs::image_frombytes(mode, size, &data, decoder_name)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[pyo3(signature = (dest_map, source_palette=None))]
    fn remap_palette(
        &mut self,
        dest_map: Vec<u8>,
        source_palette: Option<Vec<u8>>,
    ) -> PyResult<PyImage> {
        let remapped = match source_palette.as_deref() {
            None => self.inner.remap_palette(&dest_map),
            Some(source_palette) => self
                .inner
                .remap_palette_with_source(&dest_map, Some(source_palette)),
        }
        .map(|i| PyImage { inner: i })
        .map_err(map_error)?;
        Ok(remapped)
    }

    fn tobitmap(&mut self) -> PyResult<Vec<u8>> {
        self.inner.tobitmap().map_err(map_error)
    }

    fn effect_noise(&self, sigma: Option<f64>) -> PyResult<PyImage> {
        pillow_rs::image_effect_noise(&self.inner, sigma.unwrap_or(10.0))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn blend(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        alpha: f64,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        pillow_rs::image_blend(&im1.inner, &im2.inner, alpha)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn composite(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        mask: &Bound<'_, PyImage>,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        let m = mask.borrow();
        pillow_rs::image_composite(&im1.inner, &im2.inner, &m.inner)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn merge(_cls: &Bound<'_, PyType>, mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
        let mut images = Vec::new();
        for item in bands.iter()? {
            let obj = item?;
            let py_img = obj.downcast::<PyImage>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("bands must be a sequence of Image objects")
            })?;
            images.push(py_img.borrow().inner.clone());
        }
        pillow_rs::image_merge(mode, &images)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self) -> PyResult<()> {
        self.inner.verify().map_err(map_error)
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

    fn getpixel(&mut self, xy: (u32, u32)) -> PyResult<(u8, u8, u8, u8)> {
        self.inner.getpixel(xy.0, xy.1).map_err(map_error)
    }

    fn getpixel_formatted(&mut self, xy: (u32, u32), mode: &str) -> PyResult<PyObject> {
        let (r, g, b, a) = self.inner.getpixel(xy.0, xy.1).map_err(map_error)?;
        Python::with_gil(|py| {
            Ok(match mode {
                "L" | "1" => r.to_object(py),
                "LA" | "PA" => (r, a).to_object(py),
                "RGB" => (r, g, b).to_object(py),
                "RGBA" | "CMYK" => (r, g, b, a).to_object(py),
                "P" => r.to_object(py), // P mode stored as RGB; r is the palette index proxy
                _ => (r, g, b).to_object(py),
            })
        })
    }

    fn putdata(slf: &Bound<'_, Self>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::putdata_formatted(slf, data, 1.0, 0.0)
    }

    #[pyo3(signature = (data, scale=1.0, offset=0.0))]
    fn putdata_formatted(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
    ) -> PyResult<()> {
        if !python_is_sequence(data) {
            return Err(PyTypeError::new_err("argument must be a sequence"));
        }
        let entry_count = data
            .len()
            .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;

        let (width, height, mode) = {
            let image = slf.try_borrow()?;
            let (width, height) = image.inner.size().map_err(map_error)?;
            let mode = image.inner.mode().map_err(map_error)?;
            (width, height, mode)
        };
        let pixel_count = u64::from(width) * u64::from(height);
        if entry_count as u128 > u128::from(pixel_count) {
            return Err(PyTypeError::new_err("too many data entries"));
        }

        // Pillow's I;16 bytes fast path copies the supplied bytes into the
        // raw two-byte sample buffer. It does not coerce each byte into a
        // separate numeric sample as the generic sequence path does.
        if matches!(mode.as_str(), "I;16" | "I;16L" | "I;16B" | "I;16N") {
            if let Ok(bytes) = data.downcast::<PyBytes>() {
                return slf
                    .try_borrow_mut()?
                    .inner
                    .putdata_l16_bytes(bytes.as_bytes())
                    .map_err(map_error);
            }
        }

        // Pillow's image8 fast path reads the underlying bytes directly, even
        // for a bytes subclass that overrides Python iteration.
        if matches!(mode.as_str(), "1" | "L" | "P") {
            if let Ok(bytes) = data.downcast::<PyBytes>() {
                let values = bytes
                    .as_bytes()
                    .iter()
                    .copied()
                    // Pillow reads the terminating NUL when a bytes subtype
                    // reports one more item than its physical payload. Extend
                    // that behavior safely for every missing item instead of
                    // following its unchecked C pointer read out of bounds.
                    .chain(std::iter::repeat(0))
                    .take(entry_count)
                    .map(|value| PutDataValue::Number(f64::from(value)))
                    .collect::<Vec<_>>();
                return slf
                    .try_borrow_mut()?
                    .inner
                    .putdata_values(&values, scale, offset)
                    .map_err(map_error);
            }
        }

        let write_item = |pixel_index, item: Bound<'_, PyAny>| -> PyResult<()> {
            // No PyImage borrow may span coercion: __index__ and __float__ can
            // re-enter this same public image and must observe earlier writes.
            let value = putdata_value_from_python(&item, &mode)?;
            slf.try_borrow_mut()?
                .inner
                .putdata_value_at(pixel_index, &value, scale, offset)
                .map_err(map_error)
        };

        // CPython's PySequence_Fast retains exact lists and tuples instead of
        // copying them. Read each exact-list item only when its pixel is due so
        // coercing an earlier item can replace a later one, as Pillow exposes.
        if let Ok(list) = data.downcast_exact::<PyList>() {
            for pixel_index in 0..entry_count {
                write_item(pixel_index, list.get_item(pixel_index)?)?;
            }
            return Ok(());
        }
        if let Ok(tuple) = data.downcast_exact::<PyTuple>() {
            for pixel_index in 0..entry_count {
                write_item(pixel_index, tuple.get_item(pixel_index)?)?;
            }
            return Ok(());
        }

        // Pillow calls PySequence_Fast before coercing any pixel. Materialize
        // generic sequence items first, map iteration failures to its fixed
        // error, then process only the count reported by the original sequence.
        let iterator = data
            .iter()
            .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;
        let mut items = Vec::new();
        for item in iterator {
            items.push(item.map_err(|_| PyTypeError::new_err("argument must be a sequence"))?);
        }

        for (pixel_index, item) in items.into_iter().take(entry_count).enumerate() {
            write_item(pixel_index, item)?;
        }
        Ok(())
    }

    fn putpixel(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let (r, g, b, a) = if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
            (r, g, b, 255)
        } else if let Ok((r, g, b, a)) = value.extract::<(u8, u8, u8, u8)>() {
            (r, g, b, a)
        } else if let Ok(val) = value.extract::<u8>() {
            (val, val, val, 255)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "value must be int, RGB tuple, or RGBA tuple",
            ));
        };
        self.inner
            .putpixel(xy.0, xy.1, r, g, b, a)
            .map_err(map_error)
    }
    /// Mode-aware putpixel: expands values according to PIL's per-mode semantics.
    fn putpixel_mode(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = if let Ok(value) = value.extract::<i64>() {
            pillow_rs::PutPixelValue::Integer(value)
        } else if let Ok(value) = value.extract::<Vec<u8>>() {
            pillow_rs::PutPixelValue::Components(value)
        } else if let Ok(value) = value.extract::<f64>() {
            pillow_rs::PutPixelValue::Float(value)
        } else {
            pillow_rs::PutPixelValue::Invalid
        };
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

    fn explicit_mode(&self) -> PyResult<Option<String>> {
        Ok(self.inner.explicit_mode().map(|s| s.to_string()))
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

#[pyfunction]
fn resolve_array_layout(
    shape: Vec<usize>,
    typestr: &str,
    mode: Option<&str>,
) -> PyResult<(String, String, usize, usize, usize, bool)> {
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode).map_err(map_error)?;
    Ok((
        layout.mode,
        layout.raw_mode,
        layout.width,
        layout.height,
        layout.dimensions,
        layout.mode_reinterprets_dtype,
    ))
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

// --- Utility functions (moved from Python to satisfy "thin wrapper" rule) ---

/// Align each scanline to a 4-byte boundary (Qt/BMP compatibility).
/// Matches PIL's `ImageQt._toqclass_helper` align8to32 padding logic.
#[pyfunction]
#[pyo3(signature = (data, width, bits_per_pixel=8))]
fn align_row_to_32(data: Vec<u8>, width: u32, bits_per_pixel: u8) -> PyResult<Vec<u8>> {
    pillow_rs::align_row_to_32(&data, width, bits_per_pixel).map_err(map_error)
}

/// Create an Image from a flat or nested list of integer pixel values.
///
/// Python extraction is kept here at the ABI boundary; mode arity, range,
/// width, and raw-image construction are owned by the Rust core.
#[pyfunction]
fn fromarray_pixel_list(data: &Bound<'_, PyAny>, mode: Option<&str>) -> PyResult<PyImage> {
    let flat: Vec<i32> = if let Ok(v) = data.extract::<Vec<i32>>() {
        v
    } else if let Ok(nested) = data.extract::<Vec<Vec<i32>>>() {
        nested.into_iter().flatten().collect()
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "fromarray_pixel_list: expected list of ints or nested list of ints",
        ));
    };

    pillow_rs::from_array_pixel_values(&flat, mode)
        .map(|img| PyImage { inner: img })
        .map_err(map_error)
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

/// Create an image from a Python array-interface or list object.
///
/// The ABI layer only marshals Python protocols into plain Rust values. Dtype,
/// shape, mode, and byte-layout policy are implemented by `pillow-rs` so the
/// Python and JavaScript bindings cannot grow divergent `fromarray` logic.
#[pyfunction]
fn fromarray(data: &Bound<'_, PyAny>, mode: Option<&str>) -> PyResult<PyImage> {
    if let Ok(bytes) = data.downcast::<PyBytes>() {
        return pillow_rs::from_array_bytes(bytes.as_bytes(), mode)
            .map(|img| PyImage { inner: img })
            .map_err(map_error);
    }

    if data.hasattr("__array_interface__")? {
        let (shape, typestr) = array_interface_descriptor(data)?;
        // Resolve dimensions before touching the Python buffer. Pillow reports
        // malformed shape/mode combinations before it asks the object for a
        // byte buffer.
        pillow_rs::resolve_array_layout(&shape, &typestr, mode).map_err(map_error)?;
        let bytes = array_interface_bytes(data, mode)?;
        return pillow_rs::from_array_interface(&shape, &typestr, mode, &bytes)
            .map(|img| PyImage { inner: img })
            .map_err(map_error);
    }

    if data.hasattr("tobytes")? {
        let shape = data
            .getattr("shape")
            .and_then(|shape| shape.extract::<Vec<usize>>())?;
        let bytes = data.call_method0("tobytes")?.extract::<Vec<u8>>()?;
        let inferred_typestr = "|u1";
        pillow_rs::resolve_array_layout(&shape, inferred_typestr, mode).map_err(map_error)?;
        return pillow_rs::from_array_interface(&shape, inferred_typestr, mode, &bytes)
            .map(|img| PyImage { inner: img })
            .map_err(map_error);
    }

    if let Ok(flat) = data.extract::<Vec<i32>>() {
        return pillow_rs::from_array_pixel_values(&flat, mode)
            .map(|img| PyImage { inner: img })
            .map_err(map_error);
    }
    if let Ok(nested) = data.extract::<Vec<Vec<i32>>>() {
        let flat = nested.into_iter().flatten().collect::<Vec<_>>();
        return pillow_rs::from_array_pixel_values(&flat, mode)
            .map(|img| PyImage { inner: img })
            .map_err(map_error);
    }

    Err(pyo3::exceptions::PyNotImplementedError::new_err(format!(
        "fromarray: unsupported object type ({})",
        data.get_type().name()?
    )))
}

/// Flatten mesh transform data (list of (bbox, quad) tuples) into a flat f64 Vec.
///
/// Accepts `[(bbox, quad), …]` where each bbox is `[x0, y0, x1, y1]` and each
/// quad is `[x0, y0, …, x3, y3]` (8 coords).  Returns `[b0,b1,b2,b3, q0,…,q7, …]`.
#[pyfunction]
fn mesh_flatten(items: Vec<(Vec<f64>, Vec<f64>)>) -> PyResult<Vec<f64>> {
    let mut flat = Vec::with_capacity(items.len() * 12);
    for (bbox, quad) in &items {
        if bbox.len() != 4 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mesh_flatten: each bbox must have exactly 4 values [x0, y0, x1, y1]",
            ));
        }
        if quad.len() != 8 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mesh_flatten: each quad must have exactly 8 values [x0, y0, …, x3, y3]",
            ));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
}

/// Apply a Python callable to the range 0..255 to produce a LUT, then
/// replicate for `n_bands` channels.  Used by `Image.eval` and `Image.point`.
#[pyfunction]
fn make_lut(func: &Bound<'_, PyAny>, n_bands: u32) -> PyResult<Vec<u8>> {
    let mut table = Vec::with_capacity(256);
    for i in 0..256u32 {
        let result = func.call1((i,)).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("LUT function failed: {}", e))
        })?;
        let v = result.extract::<i32>().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err("LUT function must return an integer")
        })?;
        // Pillow 12.2.0 `_imaging.c::_point` maps each function output through
        // CLIP8 (ImagingUtils.h), saturating to [0, 255]; wrapping with a mask
        // would diverge for out-of-range function values.
        table.push(if v <= 0 {
            0
        } else if v < 256 {
            v as u8
        } else {
            255
        });
    }
    if n_bands > 1 {
        table = table.repeat(n_bands as usize);
    }
    Ok(table)
}

#[pyfunction]
fn eval_validate_input(value: &Bound<'_, PyAny>) -> PyResult<()> {
    let kind = if value.extract::<String>().is_ok() {
        pillow_rs::EvalInputKind::String
    } else {
        pillow_rs::EvalInputKind::Other
    };
    pillow_rs::validate_eval_input(kind).map_err(map_error)
}

#[pyfunction]
fn imaging_core_to_bytes(py: Python<'_>, values: &Bound<'_, PyAny>) -> PyResult<PyObject> {
    let input = values
        .extract::<Vec<i64>>()
        .map(pillow_rs::ImagingCoreBytesInput::Scalars)
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

    m.add_class::<PyImage>()?;
    m.add_class::<PyDraw>()?;
    m.add_class::<PyOutline>()?;
    m.add_class::<PyFont>()?;
    m.add_class::<PyPilFont>()?;
    m.add_function(wrap_pyfunction!(transposed_font_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(validate_transposed_font_length, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_array_layout, m)?)?;

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
    m.add_function(wrap_pyfunction!(palette_search, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_append, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_validate, m)?)?;
    m.add_function(wrap_pyfunction!(palette_save_to_file, m)?)?;
    m.add_function(wrap_pyfunction!(palette_to_text, m)?)?;

    // ImageDraw helpers
    m.add_function(wrap_pyfunction!(outline_curve, m)?)?;

    // ImageStat module helpers
    m.add_function(wrap_pyfunction!(stat_from_list, m)?)?;
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

    // ImageFilter helper functions
    m.add_function(wrap_pyfunction!(color3dlut_check_size, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_new, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_generate, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_transform, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_repr, m)?)?;
    m.add_function(wrap_pyfunction!(stat_from_list, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_prepare, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_validate_coefficients, m)?)?;

    // Utility functions (moved from Python)
    m.add_function(wrap_pyfunction!(align_row_to_32, m)?)?;
    m.add_function(wrap_pyfunction!(fromarray, m)?)?;
    m.add_function(wrap_pyfunction!(fromarray_pixel_list, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_flatten, m)?)?;
    m.add_function(wrap_pyfunction!(make_lut, m)?)?;
    m.add_function(wrap_pyfunction!(eval_validate_input, m)?)?;
    m.add_function(wrap_pyfunction!(imaging_core_to_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(exif_compat_fields, m)?)?;
    m.add_function(wrap_pyfunction!(imagefont_normalize_bbox, m)?)?;

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
        fp: &str,
        size: f64,
        index: usize,
        encoding: &str,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let data = std::fs::read(fp).map_err(|e| {
            pyo3::exceptions::PyOSError::new_err(format!("Cannot read font file: {}", e))
        })?;
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
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(f32, f32, f32, f32)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getbbox_with_options(&self.inner, text, &options).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn getbbox_bytes_with_options(
        &self,
        text: Vec<u8>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(f32, f32, f32, f32)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getbbox_bytes_with_options(&self.inner, &text, &options)
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
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(u32, u32, Vec<u8>)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ink,
            start,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getmask_with_options(&self.inner, text, &options).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None))]
    fn getmask_alpha_bytes_with_options(
        &self,
        text: Vec<u8>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(u32, u32, Vec<u8>)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ink,
            start,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getmask_bytes_with_options(&self.inner, &text, &options)
            .map_err(map_error)
    }

    fn get_transposed_mask_image(
        &self,
        text: &str,
        orientation: Option<&str>,
    ) -> PyResult<PyImage> {
        let (width, height, pixels) =
            pillow_rs::imagefont_get_transposed_mask(&self.inner, text, orientation)
                .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
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
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
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
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None, stroke_filled=false, has_args=false, has_kwargs=false))]
    fn getmask2_image_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            ink,
            start,
            has_args,
            has_kwargs,
        };
        let (width, height, pixels, offset) =
            pillow_rs::imagefont_getmask2_with_options(&self.inner, text, &options)
                .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None, stroke_filled=false, has_args=false, has_kwargs=false))]
    fn getmask2_image_bytes_with_options(
        &self,
        text: Vec<u8>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            ink,
            start,
            has_args,
            has_kwargs,
        };
        let (width, height, pixels, offset) =
            pillow_rs::imagefont_getmask2_bytes_with_options(&self.inner, &text, &options)
                .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok((PyImage { inner }, offset))
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
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        stroke_filled: bool,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(u32, u32, Vec<u8>, (i32, i32))> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            ink,
            start,
            has_args: false,
            has_kwargs: false,
        };
        pillow_rs::imagefont_native_render(&self.inner, text, &options).map_err(map_error)
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

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None))]
    fn getlength_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
    ) -> PyResult<f32> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getlength_with_options(&self.inner, text, &options).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None))]
    fn getlength_bytes_with_options(
        &self,
        text: Vec<u8>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
    ) -> PyResult<f32> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getlength_bytes_with_options(&self.inner, &text, &options)
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

    fn set_variation_by_name(&mut self, name: Vec<u8>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_name(&mut self.inner, &name).map_err(map_error)
    }

    fn setvarname(&mut self, instance_index: i64) -> PyResult<()> {
        pillow_rs::imagefont_native_setvarname(&mut self.inner, instance_index).map_err(map_error)
    }

    fn set_variation_by_axes(&mut self, axes: Vec<f32>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_axes(&mut self.inner, &axes).map_err(map_error)
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

    fn getsize(&self, text: Vec<u8>) -> PyResult<(i32, i32)> {
        self.inner.getsize(&text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=""))]
    fn getmask(&self, text: Vec<u8>, mode: &str) -> PyResult<PyImage> {
        let _ = mode;
        let image = self
            .inner
            .getmask(&text)
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

    #[getter]
    fn _points(&self) -> Vec<(i32, i32)> {
        self.points.clone()
    }
}

#[pyclass(name = "ImageDraw")]
pub struct PyDraw {
    draw: pillow_rs::Draw,
}

fn draw_points_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawPointsInput {
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawPointsInput::Nested(points);
    }
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawPointsInput::Flat(values);
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

fn extract_draw_points(xy: &Bound<'_, PyAny>) -> PyResult<Vec<(i32, i32)>> {
    if let Ok(points) = xy.extract::<Vec<(i32, i32)>>() {
        return Ok(points);
    }
    let flat = xy.extract::<Vec<i32>>()?;
    if flat.len() < 2 || flat.len() % 2 != 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "wrong number of coordinates",
        ));
    }
    Ok(flat
        .chunks_exact(2)
        .map(|point| (point[0], point[1]))
        .collect())
}

#[pymethods]
impl PyDraw {
    #[new]
    #[pyo3(signature = (image, mode=None))]
    fn new(image: &Bound<'_, PyImage>, mode: Option<String>) -> PyResult<Self> {
        let borrowed = image.borrow();
        let draw = pillow_rs::Draw::new(borrowed.inner.clone(), mode);
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
        let w = width.map_or(1, |w| if w > 0 { w } else { 1 });
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
                width.unwrap_or(1),
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
                width.unwrap_or(1),
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
                width.unwrap_or(1),
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
                width.unwrap_or(1),
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
            extract_draw_points(shape)
                .map_err(|_| pyo3::exceptions::PyTypeError::new_err("Unsupported shape format"))?
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
                width.unwrap_or(1),
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
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
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
                width.unwrap_or(1),
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
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
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
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius, fill=None, outline=None, width=1))]
    fn circle(
        &mut self,
        xy: (f64, f64),
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
        self.draw
            .circle(xy.0 as i32, xy.1 as i32, radius, fc, oc, width.unwrap_or(1))
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
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
        self.draw
            .rounded_rectangle_with_input(
                draw_box_input_from_python(xy),
                radius,
                fc,
                oc,
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn text(
        &mut self,
        xy: (f64, f64),
        text: String,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        if let Some(pyfont) = font {
            let borrowed = pyfont.borrow();
            let options = pillow_rs::ImageFontTextOptions {
                direction,
                features,
                language,
                stroke_width,
                anchor,
                ..pillow_rs::ImageFontTextOptions::default()
            };
            self.draw
                .text_with_options(
                    xy.0 as i32,
                    xy.1 as i32,
                    &text,
                    &borrowed.inner,
                    color,
                    &options,
                )
                .map_err(map_error)
        } else {
            Err(pyo3::exceptions::PyNotImplementedError::new_err(
                "text() requires a font",
            ))
        }
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, spacing=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn multiline_text(
        &mut self,
        xy: (f64, f64),
        text: &str,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: Option<i32>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let Some(pyfont) = font else {
            return Err(pyo3::exceptions::PyNotImplementedError::new_err(
                "text() requires a font",
            ));
        };
        let borrowed = pyfont.borrow();
        self.draw
            .multiline_text_with_options(
                xy.0,
                xy.1,
                text,
                &borrowed.inner,
                color,
                f64::from(spacing.unwrap_or(4)),
                &options,
            )
            .map_err(map_error)
    }

    /// Compute text bounding box. Loads default FreeType font if font is None.
    /// Returns (left, top, right, bottom).
    #[pyo3(signature = (xy, text, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn textbbox(
        &mut self,
        xy: (i32, i32),
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let bbox = match font {
            Some(f) => {
                let bbox =
                    pillow_rs::imagefont_getbbox_with_options(&f.borrow().inner, text, &options)
                        .map_err(map_error)?;
                (bbox.0 as i32, bbox.1 as i32, bbox.2 as i32, bbox.3 as i32)
            }
            None => {
                let font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
                let bbox = pillow_rs::imagefont_getbbox_with_options(&font, text, &options)
                    .map_err(map_error)?;
                (bbox.0 as i32, bbox.1 as i32, bbox.2 as i32, bbox.3 as i32)
            }
        };
        Ok((xy.0 + bbox.0, xy.1 + bbox.1, xy.0 + bbox.2, xy.1 + bbox.3))
    }

    /// Compute text length in pixels. Loads default FreeType font if font is None.
    #[pyo3(signature = (text, font=None, direction=None, features=None, language=None))]
    fn textlength(
        &mut self,
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
    ) -> PyResult<f64> {
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let w = match font {
            Some(f) => {
                pillow_rs::imagefont_getlength_with_options(&f.borrow().inner, text, &options)
                    .map_err(map_error)?
            }
            None => {
                let font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
                pillow_rs::imagefont_getlength_with_options(&font, text, &options)
                    .map_err(map_error)?
            }
        };
        Ok(w as f64)
    }

    /// Compute bounding box for multiline text. Matches PIL's exact algorithm.
    #[pyo3(signature = (xy, text, font=None, spacing=4, align="left", direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn multiline_textbbox(
        &mut self,
        xy: (i32, i32),
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        spacing: i32,
        align: &str,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let default_font;
        let f: &pillow_rs::FreeTypeFont = if let Some(f) = font {
            &f.borrow().inner
        } else {
            default_font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
            &default_font
        };
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_multiline_textbbox(f, xy, text, spacing, align, &options)
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
    let mask = imageops_mask_from_python(mask);
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_autocontrast_with_mask(&inner, c, mask))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>, mask: Option<&Bound<'_, PyAny>>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let mask = imageops_mask_from_python(mask);
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
    let color = match color {
        None => pillow_rs::ImageOpsColor::None,
        Some(color) => {
            if let Ok(value) = color.extract::<String>() {
                pillow_rs::ImageOpsColor::Name(value)
            } else if let Ok(value) = color.extract::<i64>() {
                pillow_rs::ImageOpsColor::Scalar(value)
            } else if let Ok(values) = color.extract::<Vec<i64>>() {
                pillow_rs::ImageOpsColor::Components(values)
            } else {
                pillow_rs::ImageOpsColor::Invalid
            }
        }
    };

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
fn ops_scale(image: &Bound<'_, PyImage>, factor: f64, filter: Option<String>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_scale(&inner, factor, filter.as_deref()))
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

    // Resolve fill: int -> (v, 0, 0, 0), 3-tuple -> (v0, v1, v2, 0), 4-tuple as-is
    let fill_val: (u8, u8, u8, u8) = if let Ok(i) = fill.extract::<u8>() {
        (i, 0, 0, 0)
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
    let mut band_images: Vec<pillow_rs::Image> = Vec::new();
    let mut invalid_band = false;
    for item in bands.iter()? {
        let obj = item?;
        if let Some(image) = image_from_python(&obj) {
            band_images.push(image);
        } else {
            invalid_band = true;
        }
    }
    if invalid_band {
        // Preserve Pillow's core-owned mode/arity error ordering. The host
        // adapter records only that extraction failed; Rust decides whether
        // the public result is a mode or band-count error.
        band_images.clear();
    }
    let rs = pillow_rs::image_merge(mode, &band_images).map_err(map_error)?;
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

#[pyfunction]
fn palette_search(palette: Vec<u8>, r: u8, g: u8, b: u8) -> Option<usize> {
    pillow_rs::palette_getcolor(&palette, r, g, b)
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

/// Compute cubic Bezier curve subdivision points for Outline.
/// Accepts flat list of 8 control points [x0,y0,x1,y1,x2,y2,x3,y3] and steps.
/// Returns flat list of [x,y] int pairs for the curve.
#[pyfunction]
fn outline_curve(points: Vec<f64>, steps: u32) -> Vec<Vec<i32>> {
    let result = pillow_rs::outline_curve_points(&points, steps);
    result.into_iter().map(|(x, y)| vec![x, y]).collect()
}

/// Compute basic statistics from a list of values (PIL's ImageStat fallback).
/// Returns a dict with count, sum, mean, min, max.
#[pyfunction]
fn stat_from_list(data: Vec<f64>) -> PyObject {
    let (count, sum, mean, min_val, max_val) = pillow_rs::stat_from_list(&data);
    Python::with_gil(|py| {
        let dict = pyo3::types::PyDict::new(py);
        let _ = dict.set_item("count", count as i64);
        let _ = dict.set_item("sum", sum);
        let _ = dict.set_item("mean", mean);
        let _ = dict.set_item("min", min_val);
        let _ = dict.set_item("max", max_val);
        dict.to_object(py)
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
    pillow_rs::color3dlut_transform_table(
        &table,
        size,
        channels_in,
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

/// Prepare kernel parameters for image convolution.
/// - If `kernel` is None, creates a default kernel of all 1.0s
/// - If `scale` is None, computes it as the sum of kernel values
/// - Converts `offset` to i32
/// - Validates that size[0] == size[1] and size[0] in {3, 5}
/// Returns (kernel, scale, offset_i32, size_x).
#[pyfunction]
#[pyo3(signature = (kernel, scale=None, offset=0.0, size=(3, 3)))]
fn kernel_prepare(
    kernel: Option<Vec<f64>>,
    scale: Option<f64>,
    offset: f64,
    size: (u32, u32),
) -> PyResult<(Vec<f64>, f64, i32, u32)> {
    pillow_rs::prepare_kernel(kernel, scale, offset, size).map_err(map_error)
}

#[pyfunction]
fn kernel_validate_coefficients(kernel: Option<Vec<f64>>, size: (u32, u32)) -> PyResult<()> {
    pillow_rs::validate_kernel_coefficients(kernel.as_deref(), size).map_err(map_error)
}
