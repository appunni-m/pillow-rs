use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

// --- ImageChops module-level functions ---

pub(crate) fn chops_offset_from_python(offset: &Bound<'_, PyAny>) -> PyResult<f64> {
    // PyArg_ParseTuple's "i" accepts __index__, checks C long first, then int.
    // Extract inside the binding to retain Pillow's unprefixed error messages.
    let value = offset.extract::<std::os::raw::c_long>().map_err(|error| {
        if error.is_instance_of::<PyOverflowError>(offset.py()) {
            PyOverflowError::new_err("Python int too large to convert to C long")
        } else {
            error
        }
    })?;
    let value = i32::try_from(value).map_err(|_| {
        PyOverflowError::new_err(if value < 0 {
            "signed integer is less than minimum"
        } else {
            "signed integer is greater than maximum"
        })
    })?;
    Ok(f64::from(value))
}

#[pyfunction]
fn chops_add(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let offset = chops_offset_from_python(offset)?;
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::chops_add(&b1, &b2, scale, offset)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_subtract(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let offset = chops_offset_from_python(offset)?;
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::chops_subtract(&b1, &b2, scale, offset)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_multiply(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::chops_multiply(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_screen(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::chops_screen(&b1, &b2))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_darker(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::chops_darker(&b1, &b2))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_lighter(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::chops_lighter(&b1, &b2))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_difference(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::chops_difference(&b1, &b2)))
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

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(chops_add, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract, m)?)?;
    m.add_function(wrap_pyfunction!(chops_multiply, m)?)?;
    m.add_function(wrap_pyfunction!(chops_screen, m)?)?;
    m.add_function(wrap_pyfunction!(chops_darker, m)?)?;
    m.add_function(wrap_pyfunction!(chops_lighter, m)?)?;
    m.add_function(wrap_pyfunction!(chops_difference, m)?)?;
    m.add_function(wrap_pyfunction!(chops_duplicate, m)?)?;
    m.add_function(wrap_pyfunction!(chops_invert, m)?)?;
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
    Ok(())
}
