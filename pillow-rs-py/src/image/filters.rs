use super::execution::{pyimage_from_active_route, route_from_context, route_pending_image};
use super::{PyExecutionContext, PyImage};
use crate::{
    blur_radius_pair_from_python, c_int_from_python, filter_rank_from_python,
    filter_size_from_python, map_error,
};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAnyMethods, PyInt};

#[pymethods]
impl PyImage {
    // TODO(pillow-rs#3ptb): verify outputs, pass/radius fault order, and route
    // receipts for the private ImagingCore GaussianBlur path in CPU parity.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (radius, passes=3))]
    fn gaussian_blur(
        slf: Py<Self>,
        radius: &Bound<'_, PyAny>,
        passes: i32,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(|| inner.gaussian_blur_xy_with_passes(radius_x as f32, radius_y as f32, passes))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (radius, percent, threshold, execution_context=None))]
    fn unsharp_mask(
        slf: Py<Self>,
        radius: &Bound<'_, PyAny>,
        percent: &Bound<'_, PyAny>,
        threshold: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let radius = radius.extract::<f64>()? as f32;
        let percent = c_int_from_python(percent)?;
        let threshold = c_int_from_python(threshold)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let rs = py
            .detach(move || match execution_route {
                Some((requested_backend, context)) => inner.unsharp_mask_with_execution_route(
                    radius,
                    percent,
                    threshold,
                    requested_backend,
                    context,
                ),
                None => inner.unsharp_mask(radius, percent, threshold),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size=3))]
    fn max_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.max_filter(size))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size=3))]
    fn min_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.min_filter(size))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size=3))]
    fn median_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.median_filter(size))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // TODO(pillow-rs#3ptb): verify outputs, pass/radius fault order, and route
    // receipts for the private ImagingCore BoxBlur path in CPU parity.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (radius, passes=1))]
    fn box_blur(
        slf: Py<Self>,
        radius: &Bound<'_, PyAny>,
        passes: i32,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.box_blur_xy_with_passes(radius_x as f32, radius_y as f32, passes))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size, execution_context=None))]
    fn modefilter(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, true)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let rs = py
            .detach(move || match execution_route {
                Some((requested_backend, context)) => {
                    inner.mode_filter_with_execution_route(size, requested_backend, context)
                }
                None => inner.mode_filter(size),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size=3, rank=0))]
    fn rank_filter(slf: Py<Self>, size: i32, rank: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let rank = PyInt::new(py, rank);
        let rank = filter_rank_from_python(rank.as_any())?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.rank_filter(size, rank))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }
}
