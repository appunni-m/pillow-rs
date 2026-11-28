use super::PyImage;
use super::execution::{ExecutionRoute, route_from_context};
use crate::{inputs::c_int_from_python, map_error};
use pyo3::exceptions::{PyAttributeError, PyOverflowError, PyValueError};
use pyo3::prelude::{Bound, Py, PyRef, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAny, PyAnyMethods, PyBool, PyInt};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn update_core_image<T: Send>(
    slf: &Py<PyImage>,
    py: Python<'_>,
    route: ExecutionRoute,
    operation: impl FnOnce(&mut pillow_rs::Image, ExecutionRoute) -> Result<T, pillow_rs::PilError>
    + Send,
) -> PyResult<T> {
    let inner = slf.bind(py).borrow().inner.clone();
    let (result, inner) = py.detach(move || {
        let mut inner = inner;
        let result = operation(&mut inner, route);
        (result, inner)
    });
    slf.bind(py).borrow_mut().inner = inner;
    result.map_err(map_error)
}

// TODO(pillow-rs#3ptb): verify scalar and image PutAlpha outputs, promotion and
// callback fault order, and pinned/selected route receipts across mode pairs.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn putalpha_input_impl(
    slf: Py<PyImage>,
    alpha: &Bound<'_, PyAny>,
    py: Python<'_>,
    prepare_destination: bool,
    allow_image_fallback: bool,
    route: ExecutionRoute,
) -> PyResult<bool> {
    let is_builtin_integer =
        alpha.is_exact_instance_of::<PyInt>() || alpha.is_exact_instance_of::<PyBool>();
    if prepare_destination && !is_builtin_integer {
        // Pillow changes the destination mode before scalar conversion or
        // image-mask inspection. Release the borrow before either can run
        // Python code that re-enters this wrapper.
        update_core_image(&slf, py, route, |image, route| match route {
            Some(route) => image
                .prepare_putalpha_input_with_execution_route(route.pinned_backend, route.context),
            None => image.prepare_putalpha_input(),
        })?;
    }

    let input = if alpha.is_exact_instance_of::<PyBool>() {
        pillow_rs::PutAlphaInput::Integer(if alpha.is_truthy()? { 1 } else { 0 })
    } else {
        match c_int_from_python(alpha) {
            Ok(value) => pillow_rs::PutAlphaInput::Integer(i64::from(value)),
            Err(error) if error.is_instance_of::<PyOverflowError>(py) => {
                // Pillow promotes the destination before its native
                // fillband parser rejects a value outside C-int range.
                if prepare_destination && is_builtin_integer {
                    update_core_image(&slf, py, route, |image, route| match route {
                        Some(route) => image.prepare_putalpha_input_with_execution_route(
                            route.pinned_backend,
                            route.context,
                        ),
                        None => image.prepare_putalpha_input(),
                    })?;
                }
                return Err(error);
            }
            Err(error)
                if error.is_instance_of::<PyAttributeError>(py)
                    || error.is_instance_of::<PyValueError>(py) =>
            {
                if allow_image_fallback {
                    // Pillow catches these errors around fillband only. Ask
                    // the wrapper to construct the public L image and route
                    // it through Image.putalpha, preserving host behavior.
                    return Ok(false);
                } else {
                    return Err(error);
                }
            }
            // PyArg's C-int parser propagates every other conversion error,
            // including a custom __index__ TypeError, unchanged.
            Err(error) => return Err(error),
        }
    };

    // `__index__` conversion above may re-enter this image; borrow it only
    // once host-language coercion has finished.
    update_core_image(&slf, py, route, move |image, route| match route {
        Some(route) => image.putalpha_with_input_and_execution_route(
            input,
            route.pinned_backend,
            route.context,
        ),
        None => image.putalpha_with_input(input),
    })
    .map(|_| true)
}

#[pymethods]
impl PyImage {
    /// Promotes RGB or RGBX storage to RGBA with opaque alpha in place.
    // TODO(pillow-rs#3ptb): verify mode rejection order, RGB/RGBX output bytes,
    // mutable-handle visibility, and active architecture route receipts.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn setalpha(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        update_core_image(&slf, py, route, |image, route| {
            let mode = image.mode_identity()?;
            if !matches!(
                mode,
                pillow_rs::Mode::ImageRgb8 | pillow_rs::Mode::ImageRgbx8
            ) {
                return Err(pillow_rs::PilError::ValueError(
                    "image has wrong mode".into(),
                ));
            }
            match route {
                Some(route) => image.putalpha_with_input_and_execution_route(
                    pillow_rs::PutAlphaInput::Integer(255),
                    route.pinned_backend,
                    route.context,
                ),
                None => image.putalpha_with_input(pillow_rs::PutAlphaInput::Integer(255)),
            }
        })
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[pyo3(signature = (alpha, execution_context=None))]
    fn putalpha_input(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        execution_context: Option<PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
        putalpha_input_impl(slf, alpha, py, true, false, route).map(|_| ())
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[pyo3(signature = (alpha, execution_context=None))]
    fn putalpha_input_prepared(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        execution_context: Option<PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<bool> {
        let route = route_from_context(execution_context, py);
        putalpha_input_impl(slf, alpha, py, false, true, route)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn prepare_putalpha_input(
        &mut self,
        execution_context: Option<PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
        let result = match route {
            Some(route) => self
                .inner
                .prepare_putalpha_input_with_execution_route(route.pinned_backend, route.context),
            None => self.inner.prepare_putalpha_input(),
        };
        result.map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn validate_putalpha_data_destination(&self) -> PyResult<()> {
        self.inner
            .validate_putalpha_data_destination()
            .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn prepare_putalpha_data_input(
        &mut self,
        execution_context: Option<PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
        let result = match route {
            Some(route) => self.inner.prepare_putalpha_data_input_with_execution_route(
                route.pinned_backend,
                route.context,
            ),
            None => self.inner.prepare_putalpha_data_input(),
        };
        result.map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[pyo3(signature = (mask, execution_context=None))]
    fn putalpha_image_input(
        slf: Py<Self>,
        mask: &Bound<'_, PyImage>,
        execution_context: Option<PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        // The facade reserves this path for exact Image objects so ordinary
        // valid masks keep one prepared PutAlphaData operation without a
        // staged promotion pass.
        let mask = mask.borrow().inner.clone();
        update_core_image(
            &slf,
            py,
            route_from_context(execution_context, py),
            move |image, route| {
                let input = pillow_rs::PutAlphaInput::Image(Box::new(mask));
                match route {
                    Some(route) => image.putalpha_with_input_and_execution_route(
                        input,
                        route.pinned_backend,
                        route.context,
                    ),
                    None => image.putalpha_with_input(input),
                }
            },
        )
    }
}
