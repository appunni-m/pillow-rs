//! Format-specific option parsing for Python image saving.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyInt, PyString, PyStringMethods};

// The index is Pillow's public integer code for each JPEG sampling mode.
const JPEG_SAMPLING: [&str; 3] = ["4:4:4", "4:2:2", "4:2:0"];

fn jpeg_subsampling(
    value: &Bound<'_, PyAny>,
    source_sampling: Option<i8>,
) -> PyResult<Option<String>> {
    let index = if let Ok(value) = value.cast::<PyString>() {
        let name = value.to_cow()?;
        if JPEG_SAMPLING.contains(&name.as_ref()) {
            return Ok(Some(name.into_owned()));
        }
        match name.as_ref() {
            // Pillow versions before 4.3 called 4:2:0 "4:1:1".
            "4:1:1" => 2,
            "keep" => i32::from(source_sampling.ok_or_else(|| {
                PyValueError::new_err("Cannot use 'keep' when original image is not a JPEG")
            })?),
            _ => {
                return Err(PyTypeError::new_err(format!(
                    "Unsupported JPEG subsampling: {name:?}"
                )));
            }
        }
    } else {
        value.extract::<i32>()?
    };
    // Pillow passes other integers (including -1) through to libjpeg's default.
    let sampling = usize::try_from(index)
        .ok()
        .and_then(|i| JPEG_SAMPLING.get(i));
    Ok(sampling.map(|&name| name.to_owned()))
}

pub(super) fn encoder_options(
    (format, codec): (&str, &str),
    compress_level: Option<i64>,
    params: Option<&Bound<'_, PyDict>>,
    source_sampling: Option<i8>,
) -> PyResult<Vec<(String, String)>> {
    let mut options = Vec::new();
    if format.eq_ignore_ascii_case("PNG")
        && let Some(level) = compress_level
    {
        if !(0..=9).contains(&level) {
            return Err(PyValueError::new_err("Invalid compression level"));
        }
        options.push(("compression".to_owned(), level.to_string()));
    }
    if let Some(params) = params
        && codec == "JPEG"
    {
        if let Some(value) = params.get_item("quality")? {
            let quality = value
                .cast::<PyInt>()
                .map_err(|_| PyValueError::new_err("Invalid quality setting"))?
                .extract::<i32>()?;
            // -1 leaves the encoder default. libjpeg clamps other qualities to 1..100.
            if quality != -1 {
                options.push(("quality".to_owned(), quality.clamp(1, 100).to_string()));
            }
        }
        if let Some(value) = params.get_item("subsampling")?
            && let Some(sampling) = jpeg_subsampling(&value, source_sampling)?
        {
            options.push(("subsampling".to_owned(), sampling));
        }
    }
    Ok(options)
}
