use super::PyImage;
use crate::{image_from_python, map_error};
use pyo3::prelude::{Bound, Py, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAny, PyAnyMethods, PyBool, PyInt, PyTypeMethods};

fn putalpha_input_impl(
    slf: Py<PyImage>,
    alpha: &Bound<'_, PyAny>,
    py: Python<'_>,
    prepare_destination: bool,
) -> PyResult<()> {
    let is_builtin_integer =
        alpha.is_exact_instance_of::<PyInt>() || alpha.is_exact_instance_of::<PyBool>();
    if prepare_destination && !is_builtin_integer {
        // Pillow changes the destination mode before scalar conversion or
        // image-mask inspection. Release the borrow before either can run
        // Python code that re-enters this wrapper.
        let mut image = slf.bind(py).borrow_mut();
        py.detach(|| image.inner.prepare_putalpha_input())
            .map_err(map_error)?;
    }

    let input = if alpha.is_exact_instance_of::<PyBool>() {
        pillow_rs::PutAlphaInput::Integer(if alpha.is_truthy()? { 1 } else { 0 })
    } else if let Some(mask) = image_from_python(alpha) {
        pillow_rs::PutAlphaInput::Image(Box::new(mask))
    } else if let Ok(value) = alpha.extract::<i64>() {
        pillow_rs::PutAlphaInput::Integer(value)
    } else if alpha.is_instance_of::<PyInt>() {
        // Clamp arbitrary-size Python integers in the core's byte domain
        // without narrowing or converting through floating point.
        let value = if alpha.lt(0)? { i64::MIN } else { i64::MAX };
        pillow_rs::PutAlphaInput::Integer(value)
    } else {
        pillow_rs::PutAlphaInput::Invalid(alpha.get_type().name()?.to_string())
    };

    // `__index__` conversion above may re-enter this image; borrow it only
    // once host-language coercion has finished.
    let mut image = slf.bind(py).borrow_mut();
    py.detach(|| image.inner.putalpha_with_input(input))
        .map_err(map_error)
}

#[pymethods]
impl PyImage {
    fn putalpha_input(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        putalpha_input_impl(slf, alpha, py, true)
    }

    fn putalpha_input_prepared(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        putalpha_input_impl(slf, alpha, py, false)
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
        let mut image = slf.bind(py).borrow_mut();
        py.detach(|| {
            image
                .inner
                .putalpha_with_input(pillow_rs::PutAlphaInput::Image(Box::new(mask)))
        })
        .map_err(map_error)
    }
}
