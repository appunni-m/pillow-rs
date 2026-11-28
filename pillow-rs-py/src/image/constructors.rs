use super::execution::PyExecutionContext;
use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

// --- Image module functions ---

#[pyfunction]
fn image_merge(mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
    let mode = pillow_rs::Mode::from_pillow_name(mode)
        .ok_or_else(|| map_error(pillow_rs::PilError::KeyError(mode.to_owned())))?;
    let inputs = merge_inputs_from_python(bands)?;
    let rs = pillow_rs::image_merge_inputs(mode, &inputs).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_merge_callback_contract(
    mode: &str,
) -> PyResult<(usize, Option<String>, String, String, String)> {
    let mode = pillow_rs::Mode::from_pillow_name(mode)
        .ok_or_else(|| map_error(pillow_rs::PilError::KeyError(mode.to_owned())))?;
    let contract = pillow_rs::image_merge_callback_contract(mode).map_err(map_error)?;
    let subsequent_band_mode = contract
        .subsequent_band_mode
        .and_then(pillow_rs::Mode::pillow_name)
        .map(str::to_owned);
    Ok((
        contract.expected_band_count,
        subsequent_band_mode,
        contract.count_rejection.to_owned(),
        contract.mode_rejection.to_owned(),
        contract.size_rejection.to_owned(),
    ))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
#[pyfunction]
#[pyo3(signature = (image1, image2, mask, execution_context=None))]
fn image_composite(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    mask: &Bound<'_, PyImage>,
    execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let bm = mask.borrow();
    let execution_context = execution_context.map(|context| context.inner);
    let result = match execution_context {
        Some(context) => pillow_rs::image_composite_with_execution_context(
            &b1.inner, &b2.inner, &bm.inner, context,
        ),
        None => pillow_rs::image_composite(&b1.inner, &b2.inner, &bm.inner),
    }
    .map_err(map_error)?;
    Ok(PyImage { inner: result })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
#[pyfunction]
#[pyo3(signature = (image1, image2, execution_context=None))]
fn image_alpha_composite(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let execution_context = execution_context.map(|context| context.inner);
    match execution_context {
        Some(context) => {
            pillow_rs::image_alpha_composite_with_execution_context(&b1.inner, &b2.inner, context)
        }
        None => pillow_rs::image_alpha_composite(&b1.inner, &b2.inner),
    }
    .map(|image| PyImage { inner: image })
    .map_err(map_error)
}

/// Generate a 256×256 linear gradient image from black to white.
#[pyfunction]
fn image_linear_gradient(mode: &str) -> PyResult<PyImage> {
    let mode = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
        map_error(pillow_rs::PilError::ValueError(
            "image has wrong mode".into(),
        ))
    })?;
    let rs = pillow_rs::image_linear_gradient(mode).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a 256×256 radial gradient image from white (center) to black (edges).
#[pyfunction]
fn image_radial_gradient(mode: &str) -> PyResult<PyImage> {
    let mode = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
        map_error(pillow_rs::PilError::ValueError(
            "image has wrong mode".into(),
        ))
    })?;
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
    m.add_function(wrap_pyfunction!(image_merge_callback_contract, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_alpha_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_linear_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_radial_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_effect_mandelbrot, m)?)?;
    Ok(())
}
