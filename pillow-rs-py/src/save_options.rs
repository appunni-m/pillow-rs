//! Format-specific option parsing for Python image saving.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;

pub(super) fn encoder_options(
    format: &str,
    compress_level: Option<i64>,
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
    Ok(options)
}
