use super::PyImage;
use super::arrays::image_info_value_to_python;
use super::execution::{PyExecutionContext, route_from_context, route_pending_image};
use crate::{Bound, Py, PyAny, PyResult, Python, map_error};
use pyo3::pymethods;
use pyo3::types::PyAnyMethods;

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    fn _has_core_only_enhancement_strategy(&self, operation: &str) -> PyResult<bool> {
        let operation = match operation {
            "brightness" => pillow_rs::OperationId::Brightness,
            "sharpness" => pillow_rs::OperationId::Sharpness,
            _ => {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "unknown enhancement operation",
                ));
            }
        };
        let mode = self.inner.mode_identity().map_err(map_error)?;
        Ok(matches!(
            operation.support().for_mode(mode).scope(),
            Some(pillow_rs::SupportScope::CoreOnly { .. })
        ))
    }

    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    #[pyo3(signature = (factor, execution_context=None))]
    fn enhance_brightness(
        &self,
        factor: f64,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let route = route_from_context(execution_context, py);
        let inner = route_pending_image(self.inner.clone(), route);
        let context = route.map(|route| route.context);
        let rs = py
            .detach(move || match context {
                Some(context) => inner
                    .enhance_brightness(factor)
                    .map(|image| image.with_execution_context(context)),
                None => inner.enhance_brightness(factor),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
    #[pyo3(signature = (factor, execution_context=None))]
    fn enhance_sharpness(
        &self,
        factor: f64,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let route = route_from_context(execution_context, py);
        let inner = route_pending_image(self.inner.clone(), route);
        let context = route.map(|route| route.context);
        let rs = py
            .detach(move || match context {
                Some(context) => inner
                    .enhance_sharpness(factor)
                    .map(|image| image.with_execution_context(context)),
                None => inner.enhance_sharpness(factor),
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
}
