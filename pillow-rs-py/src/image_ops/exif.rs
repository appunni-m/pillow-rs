use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyBytes, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};

#[pyfunction]
pub(super) fn ops_exif_transpose_xmp_value(
    py: Python<'_>,
    regex_module: &Bound<'_, PyAny>,
    original_sub: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let sub = regex_module.getattr("sub")?;
    if !sub.is(original_sub) {
        return python_xmp_substitutions(py, regex_module, value);
    }

    if value.is_instance_of::<PyString>() {
        let codepoints = xmp_text_codepoints(py, value)?;
        let value = match pillow_rs::exif_remove_xmp_orientation(
            pillow_rs::ImageOpsExifXmpValue::TextCodepoints(codepoints),
        ) {
            pillow_rs::ImageOpsExifXmpValue::TextCodepoints(value) => value,
            _ => unreachable!("text XMP strategy changed its value type"),
        };
        return xmp_codepoints_to_python(py, &value);
    }

    if let Ok(tuple) = value.cast::<PyTuple>() {
        let mut items = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            let Some(bytes) = xmp_buffer_bytes(py, &item)? else {
                // Delegate the original tuple to Python so regex errors and
                // any overridden `re.sub` behavior retain their call order.
                return python_xmp_substitutions(py, regex_module, value);
            };
            items.push(bytes);
        }
        let values = match pillow_rs::exif_remove_xmp_orientation(
            pillow_rs::ImageOpsExifXmpValue::BytesTuple(items),
        ) {
            pillow_rs::ImageOpsExifXmpValue::BytesTuple(values) => values,
            _ => unreachable!("tuple XMP strategy changed its value type"),
        };
        let values = values
            .iter()
            .map(|value| PyBytes::new(py, value).into_any())
            .collect::<Vec<_>>();
        return PyTuple::new(py, values).map(|tuple| tuple.into_any().unbind());
    }

    let Some(bytes) = xmp_buffer_bytes(py, value)? else {
        return python_xmp_substitutions(py, regex_module, value);
    };
    let value =
        match pillow_rs::exif_remove_xmp_orientation(pillow_rs::ImageOpsExifXmpValue::Bytes(bytes))
        {
            pillow_rs::ImageOpsExifXmpValue::Bytes(value) => value,
            _ => unreachable!("byte XMP strategy changed its value type"),
        };
    Ok(PyBytes::new(py, &value).into_any().unbind())
}

fn xmp_text_codepoints(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Vec<u32>> {
    let encode = py.get_type::<PyString>().getattr("encode")?;
    let bytes = encode
        .call1((value, "utf-32-le", "surrogatepass"))?
        .extract::<Vec<u8>>()?;
    Ok(bytes
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect())
}

fn xmp_codepoints_to_python(py: Python<'_>, codepoints: &[u32]) -> PyResult<Py<PyAny>> {
    let mut bytes = Vec::with_capacity(codepoints.len() * 4);
    for codepoint in codepoints {
        bytes.extend_from_slice(&codepoint.to_le_bytes());
    }
    let decode = py.get_type::<PyBytes>().getattr("decode")?;
    decode
        .call1((PyBytes::new(py, &bytes), "utf-32-le", "surrogatepass"))
        .map(|value| value.unbind())
}

fn xmp_buffer_bytes(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Option<Vec<u8>>> {
    let memoryview = py.import("builtins")?.getattr("memoryview")?;
    match memoryview.call1((value,)) {
        Ok(memoryview) => memoryview
            .call_method0("tobytes")?
            .extract::<Vec<u8>>()
            .map(Some),
        Err(error) if is_missing_python_buffer(py, &error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn is_missing_python_buffer(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<PyTypeError>(py)
}

fn python_xmp_substitutions(
    py: Python<'_>,
    regex_module: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    if value.is_instance_of::<PyString>() {
        return python_xmp_replace_text(py, regex_module, value);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        let mut first = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            first.push(python_xmp_replace_pattern(
                py,
                regex_module,
                &item,
                true,
                false,
            )?);
        }
        let first = PyTuple::new(py, first)?;
        let mut results = Vec::with_capacity(first.len());
        for item in first.iter() {
            results.push(python_xmp_replace_pattern(
                py,
                regex_module,
                &item,
                false,
                false,
            )?);
        }
        return PyTuple::new(py, results).map(|tuple| tuple.into_any().unbind());
    }
    python_xmp_replace_one(py, regex_module, value)
}

fn python_xmp_replace_pattern(
    py: Python<'_>,
    regex_module: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
    attribute: bool,
    text_pattern: bool,
) -> PyResult<Py<PyAny>> {
    let sub = regex_module.getattr("sub")?;
    let (prefix, suffix) = if attribute {
        (
            pillow_rs::EXIF_XMP_ATTRIBUTE_PREFIX,
            pillow_rs::EXIF_XMP_ATTRIBUTE_SUFFIX,
        )
    } else {
        (
            pillow_rs::EXIF_XMP_ELEMENT_PREFIX,
            pillow_rs::EXIF_XMP_ELEMENT_SUFFIX,
        )
    };
    let pattern = format!("{prefix}([0-9]){suffix}");
    if text_pattern {
        let pattern = PyString::new(py, &pattern);
        let empty = PyString::new(py, "");
        return sub
            .call1((pattern, empty, value))
            .map(|value| value.unbind());
    }
    let pattern = PyBytes::new(py, pattern.as_bytes());
    let empty = PyBytes::new(py, b"");
    sub.call1((pattern, empty, value))
        .map(|value| value.unbind())
}

fn python_xmp_replace_text(
    py: Python<'_>,
    regex_module: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let first = python_xmp_replace_pattern(py, regex_module, value, true, true)?;
    let first = first.bind(py);
    python_xmp_replace_pattern(py, regex_module, first, false, true)
}

fn python_xmp_replace_one(
    py: Python<'_>,
    regex_module: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let first = python_xmp_replace_pattern(py, regex_module, value, true, false)?;
    let first = first.bind(py);
    python_xmp_replace_pattern(py, regex_module, first, false, false)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_exif_transpose_xmp_value, module)?)?;
    Ok(())
}
