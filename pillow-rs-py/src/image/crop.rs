use super::PyImage;
use super::execution::{PyExecutionContext, route_from_active_wrapper, route_pending_image};
use crate::{Bound, Py, PyAny, PyResult, Python, c_int_from_python, map_error};
use pyo3::pymethods;
use pyo3::types::PyAnyMethods;

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn crop_box_from_python<'py>(value: &Bound<'py, PyAny>) -> PyResult<(i32, i32, i32, i32)> {
    let (left, top, right, bottom): (
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
        Bound<'py, PyAny>,
    ) = value.extract()?;
    Ok((
        c_int_from_python(&left)?,
        c_int_from_python(&top)?,
        c_int_from_python(&right)?,
        c_int_from_python(&bottom)?,
    ))
}

#[pymethods]
impl PyImage {
    // TODO(pillow-rs#3ptb): verify Crop pixels, C-int overflow faults, and
    // selected/fallback receipts for each declared CPU architecture.
    // The shared binding parser applies Pillow's fixed C-int conversion;
    // Rust then applies the default crop extent and allocation guards.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (box_coords=None))]
    fn crop(
        slf: Py<Self>,
        box_coords: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let box_coords = box_coords.map(crop_box_from_python).transpose()?;
        let execution_route = route_from_active_wrapper(py);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || {
                inner
                    .crop(box_coords)
                    .map(|image| route_pending_image(image, execution_route))
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    // TODO(pillow-rs#3ptb): verify the host-normalized fixed C-int carrier,
    // bomb-check policy boundary, pixels, and route receipts.
    // The facade has completed Pillow's comparisons, load, `round()`, `int()`,
    // and arbitrary-precision bomb check before this C-int extraction. This
    // separate entry cannot disable the default guard on direct `_core` crop.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (box_coords, max_image_pixels, execution_context=None))]
    fn _crop_with_max_pixels(
        slf: Py<Self>,
        box_coords: Option<&Bound<'_, PyAny>>,
        max_image_pixels: Option<u64>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let box_coords = box_coords.map(crop_box_from_python).transpose()?;
        let execution_context = execution_context.map(|context| context.inner);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || {
                inner.crop_with_max_pixels_and_context(
                    box_coords,
                    max_image_pixels,
                    execution_context,
                )
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }
}
