//! Python Arrow C-data adapter over Rust-owned typed image storage.

mod abi;
mod array;
mod capsule;
mod import;
mod schema;

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::PyCapsule;

use super::PyImage;
use super::execution::{route_from_active_wrapper, route_pending_image};
use crate::map_error;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(super) fn register(module: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    import::register(module)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn arrow_buffer(image: &PyImage, py: Python<'_>) -> PyResult<pillow_rs::ArrowExportBuffer> {
    let image = route_pending_image(image.inner.clone(), route_from_active_wrapper(py));
    image.arrow_export_buffer().map_err(map_error)
}

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn __arrow_c_schema__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let buffer = arrow_buffer(self, py)?;
        let schema = schema::schema_for_buffer(&buffer)?;
        capsule::schema_capsule(py, schema)
    }

    // Pillow ignores requested_schema and returns a pair of independent
    // single-use capsules for its native Arrow image layout.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[pyo3(signature = (requested_schema=None))]
    fn __arrow_c_array__<'py>(
        &self,
        requested_schema: Option<&Bound<'_, PyAny>>,
        py: Python<'py>,
    ) -> PyResult<(Bound<'py, PyCapsule>, Bound<'py, PyCapsule>)> {
        let _ = requested_schema;
        let buffer = arrow_buffer(self, py)?;
        let schema = schema::schema_for_buffer(&buffer)?;
        let array = array::array_for_buffer(&buffer)?;
        Ok((
            capsule::schema_capsule(py, schema)?,
            capsule::array_capsule(py, array)?,
        ))
    }
}
