use crate::*;
use pyo3::pymethods;

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(map_error)
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(map_error)?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(map_error)?;
        Ok(h)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        let mode = self.inner.mode_identity().map_err(map_error)?;
        // NO_PILLOW_SUPPORT: expose exact Rust names for internal-only rasters.
        Ok(mode
            .pillow_name()
            .or_else(|| mode.rust_name())
            .unwrap_or("unknown")
            .to_owned())
    }

    #[getter]
    fn bands(&self) -> PyResult<usize> {
        self.inner
            .mode_identity()
            .map(|mode| mode.band_count())
            .map_err(map_error)
    }

    #[getter]
    fn readonly(&self) -> i32 {
        // The Python adapter copies buffer and array inputs into Rust-owned
        // pixel storage, so a loaded core image is always writable.
        0
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self.inner.format_name().unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}
