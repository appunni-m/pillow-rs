//! Format-specific option parsing for Python image saving.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};

fn jpeg_subsampling(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if let Ok(name) = value.extract::<String>() {
        match name.as_str() {
            "4:4:4" | "4:2:2" | "4:2:0" => return Ok(Some(name)),
            // Pillow's historical spelling for 4:2:0.
            "4:1:1" => return Ok(Some("4:2:0".to_owned())),
            _ => {}
        }
    }
    let sampling = match value.extract::<i32>()? {
        0 => "4:4:4",
        1 => "4:2:2",
        2 => "4:2:0",
        // Other integer values, including Pillow's -1, leave the encoder default.
        _ => return Ok(None),
    };
    Ok(Some(sampling.to_owned()))
}

pub(super) fn encoder_options(
    format: &str,
    compress_level: Option<i64>,
    params: Option<&Bound<'_, PyDict>>,
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
        && matches!(
            format.to_ascii_lowercase().as_str(),
            "jpeg" | "jpg" | "jpe" | "jfif"
        )
    {
        if let Some(value) = params.get_item("quality")? {
            let quality = value
                .extract::<i32>()
                .map_err(|_| PyValueError::new_err("Invalid quality setting"))?;
            // -1 leaves the encoder default. libjpeg clamps other qualities to 1..100.
            if quality != -1 {
                options.push(("quality".to_owned(), quality.clamp(1, 100).to_string()));
            }
        }
        if let Some(value) = params.get_item("subsampling")?
            && let Some(sampling) = jpeg_subsampling(&value)?
        {
            options.push(("subsampling".to_owned(), sampling));
        }
    }
    Ok(options)
}
