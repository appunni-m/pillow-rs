use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

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

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(image_merge, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_alpha_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_linear_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_radial_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_effect_mandelbrot, m)?)?;
    Ok(())
}
