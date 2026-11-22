use super::PyImage;
use super::arrays::image_info_value_to_python;
use super::execution::{PyExecutionContext, route_from_context, route_pending_image};
use crate::{Bound, Py, PyAny, PyResult, Python, map_error};
use pyo3::pymethods;
use pyo3::types::PyAnyMethods;

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=n/a
    fn preflight_enhance_sharpness(&self) -> PyResult<()> {
        self.inner.preflight_enhance_sharpness().map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn enhance_brightness(slf: Py<Self>, factor: f64, py: Python<'_>) -> PyResult<PyImage> {
        let route = route_from_context(None, py);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.enhance_brightness(factor))
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn contrast_degenerate(
        &self,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let route = route_from_context(execution_context, py);
        let inner = route_pending_image(self.inner.clone(), route);
        let execution_context = route.map(|route| route.context);
        let rs = py
            .detach(move || match execution_context {
                Some(context) => inner.contrast_degenerate_with_execution_context(context),
                None => inner.contrast_degenerate(),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // TODO(pillow-rs#3ptb): verify fallback Blend output, error order, and
    // selected/fallback receipts across declared modes and architectures.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn enhance_contrast(slf: Py<Self>, factor: f64, py: Python<'_>) -> PyResult<PyImage> {
        let route = route_from_context(None, py);
        let inner = route_pending_image(slf.bind(py).borrow().inner.clone(), route);
        let rs = py
            .detach(move || match route {
                Some(route) => inner.enhance_contrast_with_execution_context(factor, route.context),
                None => inner.enhance_contrast(factor),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // TODO(pillow-rs#3ptb): verify fallback Blend output, error order, and
    // selected/fallback receipts across declared modes and architectures.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn enhance_color(slf: Py<Self>, factor: f64, py: Python<'_>) -> PyResult<PyImage> {
        let route = route_from_context(None, py);
        let inner = route_pending_image(slf.bind(py).borrow().inner.clone(), route);
        let rs = py
            .detach(move || match route {
                Some(route) => inner.enhance_color_with_execution_context(factor, route.context),
                None => inner.enhance_color(factor),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn color_degenerate(
        &self,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let route = route_from_context(execution_context, py);
        let inner = route_pending_image(self.inner.clone(), route);
        let execution_context = route.map(|route| route.context);
        let rs = py
            .detach(move || match execution_context {
                Some(context) => inner.color_degenerate_with_execution_context(context),
                None => inner.color_degenerate(),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (value, execution_context=None))]
    fn color_transparency(
        slf: Py<Self>,
        value: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
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
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let image = slf.bind(py).borrow();
        let inner = route_pending_image(image.inner.clone(), route);
        let value = match execution_route {
            Some((requested_backend, context)) => {
                inner.color_transparency_with_execution_route(value, requested_backend, context)
            }
            None => inner.color_transparency(value),
        }
        .map_err(map_error)?;
        image_info_value_to_python(py, value)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn enhance_sharpness(slf: Py<Self>, factor: f64, py: Python<'_>) -> PyResult<PyImage> {
        let route = route_from_context(None, py);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.enhance_sharpness(factor))
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }
}
