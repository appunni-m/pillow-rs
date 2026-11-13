use super::PyImage;
use super::arrays::image_info_value_to_python;
use crate::{Bound, Py, PyAny, PyResult, Python, map_error};
use pyo3::pymethods;
use pyo3::types::PyAnyMethods;

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=n/a
    fn preflight_enhance_sharpness(&self) -> PyResult<()> {
        self.inner.preflight_enhance_sharpness().map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn enhance_brightness(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_brightness(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn contrast_degenerate(&self, py: Python<'_>) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = py
            .detach(|| inner.contrast_degenerate())
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn enhance_contrast(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_contrast(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn enhance_color(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_color(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn color_degenerate(&self, py: Python<'_>) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = py.detach(|| inner.color_degenerate()).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=n/a
    fn color_transparency(
        slf: Py<Self>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // Color's metadata normalizer accepts component lists and converts
        // invalid input into a warning; public putpixel has stricter errors.
        let value = if let Ok(value) = value.extract::<i64>() {
            pillow_rs::PutPixelValue::Integer(value)
        } else if let Ok(value) = value.extract::<Vec<i64>>() {
            pillow_rs::PutPixelValue::Components(value)
        } else if let Ok(value) = value.extract::<Vec<f64>>() {
            pillow_rs::PutPixelValue::FloatComponents(value)
        } else if let Ok(value) = value.extract::<f64>() {
            pillow_rs::PutPixelValue::Float(value)
        } else {
            pillow_rs::PutPixelValue::Invalid
        };
        let image = slf.bind(py).borrow();
        let value = image.inner.color_transparency(value).map_err(map_error)?;
        image_info_value_to_python(py, value)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn enhance_sharpness(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_sharpness(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }
}
