use super::PyImage;
use crate::{inputs::c_int_from_python, map_error};
use pyo3::exceptions::{PyAttributeError, PyOverflowError, PyValueError};
use pyo3::prelude::{Bound, Py, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAny, PyAnyMethods, PyBool, PyInt};

fn update_core_image<T: Send>(
    slf: &Py<PyImage>,
    py: Python<'_>,
    operation: impl FnOnce(&mut pillow_rs::Image) -> Result<T, pillow_rs::PilError> + Send,
) -> PyResult<T> {
    let inner = slf.bind(py).borrow().inner.clone();
    let (result, inner) = py.detach(move || {
        let mut inner = inner;
        let result = operation(&mut inner);
        (result, inner)
    });
    slf.bind(py).borrow_mut().inner = inner;
    result.map_err(map_error)
}

fn putalpha_input_impl(
    slf: Py<PyImage>,
    alpha: &Bound<'_, PyAny>,
    py: Python<'_>,
    prepare_destination: bool,
    allow_image_fallback: bool,
) -> PyResult<bool> {
    let is_builtin_integer =
        alpha.is_exact_instance_of::<PyInt>() || alpha.is_exact_instance_of::<PyBool>();
    if prepare_destination && !is_builtin_integer {
        // Pillow changes the destination mode before scalar conversion or
        // image-mask inspection. Release the borrow before either can run
        // Python code that re-enters this wrapper.
        update_core_image(&slf, py, pillow_rs::Image::prepare_putalpha_input)?;
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
                    update_core_image(&slf, py, pillow_rs::Image::prepare_putalpha_input)?;
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
    update_core_image(&slf, py, move |image| image.putalpha_with_input(input))
        .map(|_| true)
}

#[pymethods]
impl PyImage {
    fn putalpha_input(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        putalpha_input_impl(slf, alpha, py, true, false).map(|_| ())
    }

    fn putalpha_input_prepared(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<bool> {
        putalpha_input_impl(slf, alpha, py, false, true)
    }

    fn prepare_putalpha_input(&mut self) -> PyResult<()> {
        self.inner.prepare_putalpha_input().map_err(map_error)
    }

    fn validate_putalpha_data_destination(&self) -> PyResult<()> {
        self.inner
            .validate_putalpha_data_destination()
            .map_err(map_error)
    }

    fn prepare_putalpha_data_input(&mut self) -> PyResult<()> {
        self.inner
            .prepare_putalpha_data_input()
            .map_err(map_error)
    }

    fn putalpha_image_input(
        slf: Py<Self>,
        mask: &Bound<'_, PyImage>,
        py: Python<'_>,
    ) -> PyResult<()> {
        // The facade reserves this path for exact Image objects so ordinary
        // valid masks keep one prepared PutAlphaData operation without a
        // staged promotion pass.
        let mask = mask.borrow().inner.clone();
        update_core_image(&slf, py, move |image| {
            image.putalpha_with_input(pillow_rs::PutAlphaInput::Image(Box::new(mask)))
        })
    }
}
