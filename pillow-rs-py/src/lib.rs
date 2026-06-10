use pyo3::prelude::*;
use pyo3::types::PyType;
use pillow_rs_core::error::PilError;
use pillow_rs_core::image::Image as RsImage;

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> Self {
        // Default 1x1 RGB image for compatibility
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0)).unwrap();
        PyImage { inner: img }
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(_cls: &Bound<'_, PyType>, mode: &str, size: (u32, u32), color: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let c = if let Some(val) = color {
            if let Ok(hex_str) = val.extract::<String>() {
                pillow_rs_core::color::parse_color_str(&hex_str).map_err(|e| map_error(e))?
            } else if let Ok(i) = val.extract::<u8>() {
                (i, i, i, 255)
            } else if let Ok((r, g, b)) = val.extract::<(u8, u8, u8)>() {
                (r, g, b, 255)
            } else if let Ok((r, g, b, a)) = val.extract::<(u8, u8, u8, u8)>() {
                (r, g, b, a)
            } else if let Ok((l,)) = val.extract::<(u8,)>() {
                (l, l, l, 255)
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "color must be int, tuple, or string",
                ));
            }
        } else {
            (0, 0, 0, 0)
        };
        let img = RsImage::new(size.0, size.1, mode, c).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(path) = fp.extract::<String>() {
            let img = RsImage::open_path(&path).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else if let Ok(bytes) = fp.extract::<Vec<u8>>() {
            let img = RsImage::open_bytes(bytes).map_err(|e| map_error(e))?;
            Ok(PyImage { inner: img })
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected str or bytes",
            ))
        }
    }

    fn save(&mut self, fp: &str, format: Option<String>) -> PyResult<()> {
        self.inner
            .save(fp, format.as_deref())
            .map_err(|e| map_error(e))
    }

    fn resize(&self, size: (u32, u32), resample: Option<String>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .resize(size, resample.as_deref())
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: (u32, u32, u32, u32)) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, expand=false, fillcolor=None))]
    fn rotate(
        &self,
        angle: f64,
        expand: Option<bool>,
        fillcolor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let _ = fillcolor;
        let rs = self
            .inner
            .rotate(angle, expand.unwrap_or(false), None)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &str) -> PyResult<PyImage> {
        let rs = self.inner.transpose(method).map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        &self,
        mode: &str,
        matrix: Option<Vec<f64>>,
        dither: Option<String>,
        palette: Option<String>,
        colors: Option<u32>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .convert(mode, matrix, dither.as_deref(), palette.as_deref(), colors)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let _ = (im, box_coords, mask);
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "Image.paste",
        ))
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(|e| map_error(e))?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self
            .inner
            .filter(filter_type)
            .map_err(|e| map_error(e))?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes(&mut self) -> PyResult<Vec<u8>> {
        self.inner.to_bytes().map_err(|e| map_error(e))
    }

    fn thumbnail(&mut self, size: (u32, u32), resample: Option<String>) -> PyResult<()> {
        self.inner
            .thumbnail(size, resample.as_deref())
            .map_err(|e| map_error(e))
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(|e| map_error(e))
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(|e| map_error(e))?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(|e| map_error(e))
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self
                    .inner
                    .format_name()
                    .unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}

fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ImageError(err) => pyo3::exceptions::PyException::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => {
            pyo3::exceptions::PyNotImplementedError::new_err(msg)
        }
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
    }
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyImage>()?;
    Ok(())
}
