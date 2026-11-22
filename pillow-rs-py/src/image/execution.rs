use super::PyImage;
use crate::errors::map_error;
use pyo3::prelude::{PyRef, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyList, PyListMethods, PyModule};
use pyo3::{pyclass, pymethods};

#[derive(Clone, Copy)]
pub(crate) struct PythonExecutionRoute {
    pub(crate) requested_backend: Option<pillow_rs::Backend>,
    pub(crate) pinned_backend: Option<pillow_rs::Backend>,
    pub(crate) context: pillow_rs::ExecutionContext,
}

pub(crate) type ExecutionRoute = Option<PythonExecutionRoute>;

// Direct ImagingCore calls made inside wrapper callbacks have no explicit
// context argument. Resolve the active owner at the binding boundary.
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn route_from_active_wrapper(py: Python<'_>) -> ExecutionRoute {
    let module = PyModule::import(py, "pillow_rs.image").ok()?;
    let scope = module
        .getattr("_IMAGE_EXECUTION_CONTEXT")
        .ok()?
        .call_method0("get")
        .ok()?;
    if scope.is_none() {
        return None;
    }
    let scope = scope.cast::<PyList>().ok()?;
    let mut context = scope.get_item(1).ok()?;
    if context.is_none() {
        let owner = scope.get_item(0).ok()?;
        context = owner.call_method0("_current_execution_context").ok()?;
    }
    if context.is_none() {
        return None;
    }
    context
        .extract::<PyRef<'_, PyExecutionContext>>()
        .ok()
        .map(|context| PythonExecutionRoute {
            requested_backend: context.requested_backend,
            pinned_backend: context.pinned_backend,
            context: context.inner,
        })
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn route_from_context(
    context: Option<PyRef<'_, PyExecutionContext>>,
    py: Python<'_>,
) -> ExecutionRoute {
    context
        .map(|context| PythonExecutionRoute {
            requested_backend: context.requested_backend,
            pinned_backend: context.pinned_backend,
            context: context.inner,
        })
        .or_else(|| route_from_active_wrapper(py))
}

/// Opaque CPU route captured for nested public Image callbacks.
#[pyclass(name = "_ExecutionContext", frozen)]
pub(crate) struct PyExecutionContext {
    pub(crate) inner: pillow_rs::ExecutionContext,
    pub(crate) requested_backend: Option<pillow_rs::Backend>,
    pub(crate) pinned_backend: Option<pillow_rs::Backend>,
}

// TODO(pillow-rs#3ptb): verify terminal bytes/faults and selected/fallback
// receipts for pending graphs across CPU and declared architectures.
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn route_pending_image(
    image: pillow_rs::Image,
    route: ExecutionRoute,
) -> pillow_rs::Image {
    let Some(route) = route else {
        return image;
    };
    if image.is_materialized() {
        return image;
    }
    let image = match route.pinned_backend {
        Some(backend) => image.use_backend(backend),
        None => image,
    };
    image.with_execution_context(route.context)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn pyimage_from_active_route(image: pillow_rs::Image, py: Python<'_>) -> PyImage {
    PyImage {
        inner: route_pending_image(image, route_from_active_wrapper(py)),
    }
}

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn _capture_execution_context(&self) -> PyResult<PyExecutionContext> {
        self.inner
            .capture_cpu_execution_route()
            .map(|(requested_backend, inner)| PyExecutionContext {
                inner,
                requested_backend,
                pinned_backend: self.inner.backend(),
            })
            .map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify pinned-backend and Architecture handoff on
    // lazy Python-derived results through the frozen selected/fallback matrix.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn _with_execution_context(
        &self,
        execution_context: pyo3::PyRef<'_, PyExecutionContext>,
    ) -> PyImage {
        let mut inner = self.inner.clone();
        if let Some(backend) = execution_context.pinned_backend {
            inner = inner.use_backend(backend);
        }
        PyImage {
            inner: inner.with_execution_context(execution_context.inner),
        }
    }
}
