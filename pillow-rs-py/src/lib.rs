//! Python bindings for the Rust image engine, exposed through the `PIL` namespace.
// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::redundant_clone)]

use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::{PyOverflowError, PyTypeError, PyUserWarning, PyValueError};
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pymodule;
use pyo3::types::PyAnyMethods;
use pyo3::types::PyBool;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyCapsule;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyFloat;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyModuleMethods;
use pyo3::types::PyString;
use pyo3::types::PyTuple;
use pyo3::types::PyType;
use pyo3::types::PyTypeMethods;
use std::borrow::Cow;

mod backend;
mod draw;
mod errors;
mod font;
mod getcolors;
mod gpu_batch;
mod inputs;
pub(crate) use inputs::*;
mod image;
mod image_chops;
mod image_color;
mod image_filter;
mod image_ops;
mod putdata;
mod save_options;
mod telemetry;
pub(crate) use errors::map_error;
#[cfg(test)]
use font::PilFontLoadCache;
pub(crate) use image::PyImage;
pub(crate) use image_chops::chops_offset_from_python;

// Pillow's custom exception for images exceeding its decompression-bomb limit.
#[cfg(not(feature = "parallel-wheel"))]
pyo3::create_exception!(
    _core,
    DecompressionBombError,
    pyo3::exceptions::PyException,
    "Raised when an image exceeds the configured decompression-bomb limit."
);
#[cfg(feature = "parallel-wheel")]
pyo3::create_exception!(
    _core_parallel,
    DecompressionBombError,
    pyo3::exceptions::PyException,
    "Raised when an image exceeds the configured decompression-bomb limit."
);

#[pymodule]
// Only the companion wheel uses a new init symbol; `--features parallel`
// source/benchmark builds deliberately retain the ordinary `_core` name.
#[cfg_attr(feature = "parallel-wheel", pyo3(name = "_core_parallel"))]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add(
        "DecompressionBombError",
        m.py().get_type::<DecompressionBombError>(),
    )?;
    image::register_image_class(m)?;
    gpu_batch::register(m)?;
    image::register_adapter_classes(m)?;
    draw::register_classes(m)?;
    font::register_classes(m)?;
    font::register_functions(m)?;
    image_ops::register(m)?;
    image_chops::register(m)?;
    image_color::register(m)?;
    image::register_analysis(m)?;
    image::register_functions(m)?;
    backend::register(m)?;
    image_filter::register(m)?;
    image::register_utilities(m)?;
    #[cfg(feature = "test-api")]
    {
        m.add_function(pyo3::wrap_pyfunction!(_arm_migration_test_fault, m)?)?;
        m.add_function(pyo3::wrap_pyfunction!(_clear_migration_test_fault, m)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod pilfont_load_cache_tests {
    use super::PilFontLoadCache;

    #[test]
    fn cache_requires_exact_metrics_and_bitmap_bytes() {
        let metrics = b"pilfont metrics";
        let bitmap = b"glyph bitmap";
        let font = pillow_rs::PilFont::load_default().expect("embedded default font is valid");
        let mut cache = PilFontLoadCache::default();

        cache.insert(metrics, bitmap, &font);

        assert!(cache.get(metrics, bitmap).is_some());
        assert!(cache.get(b"changed metrics", bitmap).is_none());
        assert!(cache.get(metrics, b"changed bitmap").is_none());
    }
}
