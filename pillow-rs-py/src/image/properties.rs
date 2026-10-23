use crate::*;
use pyo3::pymethods;

#[pymethods]
impl PyImage {
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

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(map_error)
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
