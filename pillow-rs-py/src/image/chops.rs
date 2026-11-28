use super::PyImage;
use super::execution::{ExecutionRoute, route_from_context, route_pending_image};
use crate::inputs::c_int_from_c_long;
use crate::{Bound, Py, PyResult, Python, map_error};
use pyo3::pymethods;

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn eager_chop(
    image: pillow_rs::Image,
    route: ExecutionRoute,
) -> Result<pillow_rs::Image, pillow_rs::PilError> {
    let mut image = route_pending_image(image, route);
    image.load()?;
    Ok(image)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn binary_chop<F>(
    first: pillow_rs::Image,
    second: pillow_rs::Image,
    context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
    py: Python<'_>,
    operation: F,
) -> PyResult<PyImage>
where
    F: FnOnce(
            &pillow_rs::Image,
            &pillow_rs::Image,
        ) -> Result<pillow_rs::Image, pillow_rs::PilError>
        + Send
        + 'static,
{
    let route = route_from_context(context, py);
    let result = py.detach(move || eager_chop(operation(&first, &second)?, route));
    let result = result.map_err(map_error)?;
    Ok(PyImage { inner: result })
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn unary_chop<F>(
    image: pillow_rs::Image,
    context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
    py: Python<'_>,
    operation: F,
) -> PyResult<PyImage>
where
    F: FnOnce(&pillow_rs::Image) -> Result<pillow_rs::Image, pillow_rs::PilError> + Send + 'static,
{
    let route = route_from_context(context, py);
    let result = py.detach(move || eager_chop(operation(&image)?, route));
    let result = result.map_err(map_error)?;
    Ok(PyImage { inner: result })
}

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (execution_context=None))]
    fn chop_invert(
        slf: Py<Self>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let image = slf.bind(py).borrow().inner.clone();
        unary_chop(image, execution_context, py, pillow_rs::chops_invert)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_lighter(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_lighter,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_darker(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_darker,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_difference(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_difference,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_multiply(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_multiply,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_screen(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_screen,
        )
    }

    // Pillow's `O!|fi` binding parses scale as a C float and offset as a C int.
    // VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-9151618946984508b6673146ca4f53d0; benchmark=n/a
    #[pyo3(signature = (other, scale=1.0, offset=0, execution_context=None))]
    fn chop_add(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        scale: f32,
        offset: std::os::raw::c_long,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        let offset = c_int_from_c_long(offset)?;
        binary_chop(first, second, execution_context, py, move |a, b| {
            pillow_rs::chops_add(a, b, f64::from(scale), f64::from(offset))
        })
    }

    // Pillow's `O!|fi` binding parses scale as a C float and offset as a C int.
    // VERIFY: source=reviewed@2026-11-26; parity=pass@migration-parity-9151618946984508b6673146ca4f53d0; benchmark=n/a
    #[pyo3(signature = (other, scale=1.0, offset=0, execution_context=None))]
    fn chop_subtract(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        scale: f32,
        offset: std::os::raw::c_long,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        let offset = c_int_from_c_long(offset)?;
        binary_chop(first, second, execution_context, py, move |a, b| {
            pillow_rs::chops_subtract(a, b, f64::from(scale), f64::from(offset))
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_add_modulo(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_add_modulo,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_subtract_modulo(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_subtract_modulo,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_and(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_logical_and,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_or(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_logical_or,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_xor(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_logical_xor,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_soft_light(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_soft_light,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_hard_light(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_hard_light,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (other, execution_context=None))]
    fn chop_overlay(
        slf: Py<Self>,
        other: &Bound<'_, PyImage>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let first = slf.bind(py).borrow().inner.clone();
        let second = other.borrow().inner.clone();
        binary_chop(
            first,
            second,
            execution_context,
            py,
            pillow_rs::chops_overlay,
        )
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (xoffset, yoffset, execution_context=None))]
    fn offset(
        slf: Py<Self>,
        xoffset: i32,
        yoffset: i32,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let image = slf.bind(py).borrow().inner.clone();
        unary_chop(image, execution_context, py, move |image| {
            pillow_rs::chops_offset(image, xoffset, yoffset)
        })
    }
}
