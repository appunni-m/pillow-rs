use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

// --- ImageColor ---

#[pyfunction]
fn getrgb(color: &str) -> PyResult<Py<PyAny>> {
    let (r, g, b, a) = pillow_rs::parse_color_str_unclamped(color).map_err(map_error)?;
    Python::attach(|py| {
        if pillow_rs::color_has_explicit_alpha(color) {
            Ok((r, g, b, a).into_py_any(py)?)
        } else {
            Ok((r, g, b).into_py_any(py)?)
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
fn getcolor(color: &str, mode: &str) -> PyResult<Py<PyAny>> {
    let (r, g, b, a) = pillow_rs::parse_color_str_unclamped(color).map_err(map_error)?;
    let result = pillow_rs::getcolor(r, g, b, a, mode).map_err(map_error)?;
    Python::attach(|py| match result {
        pillow_rs::ColorValue::Gray(value) => Ok(value.into_py_any(py)?),
        pillow_rs::ColorValue::GrayAlpha(gray, alpha) => Ok((gray, alpha).into_py_any(py)?),
        pillow_rs::ColorValue::Rgb(r, g, b) => Ok((r, g, b).into_py_any(py)?),
        pillow_rs::ColorValue::Rgba(r, g, b, a) => Ok((r, g, b, a).into_py_any(py)?),
        pillow_rs::ColorValue::Hsv(h, s, v) => Ok((h, s, v).into_py_any(py)?),
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
    let input = if color.cast::<PyTuple>().is_ok() || color.cast::<PyList>().is_ok() {
        color.extract::<Vec<u8>>().map_or(
            pillow_rs::PaletteColorInput::Invalid(repr),
            pillow_rs::PaletteColorInput::Components,
        )
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

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(getrgb, m)?)?;
    m.add_function(wrap_pyfunction!(getcolor, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_append, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_validate, m)?)?;
    m.add_function(wrap_pyfunction!(palette_save_to_file, m)?)?;
    m.add_function(wrap_pyfunction!(palette_to_text, m)?)?;
    Ok(())
}
