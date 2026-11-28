use pillow_rs::Image as RsImage;
use pyo3::prelude::{Bound, PyModule, PyResult};
use pyo3::pyclass;
use pyo3::types::PyModuleMethods;
mod analysis;
mod arrays;
mod arrow;
mod chops;
mod constructors;
mod convert;
mod creation;
mod crop;
mod data;
mod enhance;
mod execution;
mod filters;
mod imaging_core;
mod operations;
mod palette;
mod point;
mod properties;
mod put_alpha;
mod quantize;
mod sequence;
mod transform;
pub(crate) use execution::{PyExecutionContext, route_from_active_wrapper, route_pending_image};
pub(crate) use palette::palette_model_from_python;
use point::PyPointTransform;
use sequence::PyImageSequenceIterator;
#[pyclass(name = "Image")]
pub(crate) struct PyImage {
    pub(crate) inner: RsImage,
}

pub(super) fn register_image_class(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImage>()?;
    Ok(())
}
// VERIFY: source=reviewed@2026-11-23; parity=n/a; benchmark=n/a
pub(super) fn register_adapter_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPointTransform>()?;
    m.add_class::<PyExecutionContext>()?;
    m.add_class::<PyImageSequenceIterator>()?;
    m.add_class::<operations::PyRotatePlan>()?;
    m.add_class::<transform::PyTransformPlan>()?;
    m.add_class::<transform::PyTransformDataPlan>()?;
    m.add_class::<transform::PyTransformMeshState>()?;
    m.add_class::<operations::PyPasteBoxPlan>()?;
    m.add_class::<operations::PyPasteSourcePlan>()?;
    m.add_class::<palette::PyRemapPalettePlan>()?;
    m.add_class::<operations::PyThumbnailPlan>()?;
    m.add_class::<convert::PyConvertMatrixTransparency>()?;
    operations::register_adapter_functions(m)?;
    Ok(())
}
pub(super) fn register_analysis(m: &Bound<'_, PyModule>) -> PyResult<()> {
    analysis::register(m)
}
pub(super) fn register_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    constructors::register(m)?;
    arrow::register(m)
}
pub(super) fn register_utilities(m: &Bound<'_, PyModule>) -> PyResult<()> {
    arrays::register(m)
}
