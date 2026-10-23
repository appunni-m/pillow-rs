use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

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
    let interface = interface.cast::<PyDict>()?;
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
fn imaging_core_to_bytes(py: Python<'_>, values: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
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

pub(super) fn image_info_value_to_python(
    py: Python<'_>,
    value: pillow_rs::ImageInfoValue,
) -> PyResult<Py<PyAny>> {
    match value {
        pillow_rs::ImageInfoValue::Integer(value) => Ok(value.into_py_any(py)?),
        pillow_rs::ImageInfoValue::Float(value) => Ok(value.into_py_any(py)?),
        pillow_rs::ImageInfoValue::String(value) => Ok(value.into_py_any(py)?),
        pillow_rs::ImageInfoValue::Bytes(value) => Ok(PyBytes::new(py, &value).into()),
        pillow_rs::ImageInfoValue::IntegerList(value) => Ok(value.into_py_any(py)?),
        pillow_rs::ImageInfoValue::FloatList(value) => Ok(value.into_py_any(py)?),
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

pub(super) fn image_info_to_python(
    py: Python<'_>,
    fields: Vec<(String, pillow_rs::ImageInfoValue)>,
) -> PyResult<Py<PyAny>> {
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
) -> PyResult<Py<PyAny>> {
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

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(align_row_to_32, m)?)?;
    m.add_function(wrap_pyfunction!(fromarray, m)?)?;
    m.add_function(wrap_pyfunction!(imaging_core_to_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(exif_compat_fields, m)?)?;
    Ok(())
}
