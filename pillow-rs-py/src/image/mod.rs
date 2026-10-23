use pillow_rs::Image as RsImage;
use pyo3::prelude::{Bound, PyModule, PyResult};
use pyo3::pyclass;
use pyo3::types::PyModuleMethods;
mod analysis;
mod arrays;
mod constructors;
mod creation;
mod data;
mod operations;
mod point;
mod properties;
mod sequence;
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
pub(super) fn register_adapter_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPointTransform>()?;
    m.add_class::<PyImageSequenceIterator>()?;
    Ok(())
}
pub(super) fn register_analysis(m: &Bound<'_, PyModule>) -> PyResult<()> {
    analysis::register(m)
}
pub(super) fn register_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    constructors::register(m)
}
pub(super) fn register_utilities(m: &Bound<'_, PyModule>) -> PyResult<()> {
    arrays::register(m)
}
