use super::RsImage;
use crate::*;
use pyo3::pymethods;

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        // Default 1x1 RGB image for compatibility
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0)).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        size: (u32, u32),
        color: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let input = match color {
            Some(value) if mode == "RGB" && value.is_exact_instance_of::<PyTuple>() => {
                match value.extract::<(u8, u8, u8)>() {
                    Ok(rgb) => pillow_rs::PythonNewColorInput::from_parts(
                        None,
                        None,
                        Some(rgb),
                        None,
                        None,
                        None,
                        None,
                        true,
                    ),
                    Err(_) => new_color_input_from_python(color),
                }
            }
            _ => new_color_input_from_python(color),
        };
        let img = RsImage::new_with_input(size.0, size.1, mode, input).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[staticmethod]
    #[pyo3(name = "effect_noise_from_size")]
    fn effect_noise_from_size(size: (u32, u32), sigma: f64) -> PyResult<Self> {
        let image = pillow_rs::image_effect_noise_from_size(size, sigma).map_err(map_error)?;
        Ok(PyImage { inner: image })
    }

    #[classmethod]
    #[pyo3(signature = (fp, mode=None, formats=None))]
    fn open(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        fp: &Bound<'_, PyAny>,
        mode: Option<&Bound<'_, PyAny>>,
        formats: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let mode = open_mode_input_from_python(mode)?;
        let formats = open_formats_input_from_python(formats)?;
        let format_names =
            pillow_rs::validate_python_open_inputs(mode, formats).map_err(map_error)?;
        let format_refs = format_names
            .as_deref()
            .map(|names| names.iter().map(String::as_str).collect::<Vec<_>>());
        if let Some(path) = host_path_from_python(fp)? {
            let bytes = py
                .detach(|| std::fs::read(&path))
                .map_err(|error| map_open_path_error(py, fp, &path, error))?;
            let img = py
                .detach(|| RsImage::open_bytes_with_formats(bytes, format_refs.as_deref()))
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        } else {
            let bytes = fp.call_method0("read")?.extract::<Vec<u8>>()?;
            let img = py
                .detach(|| RsImage::open_bytes_with_formats(bytes, format_refs.as_deref()))
                .map_err(map_error)?;
            Ok(PyImage { inner: img })
        }
    }

    #[classmethod]
    fn validate_open_source(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<()> {
        if let Ok(bytes) = fp.cast::<PyBytes>() {
            pillow_rs::validate_python_open_source_bytes(bytes.as_bytes()).map_err(map_error)?;
        }
        Ok(())
    }

    #[classmethod]
    #[pyo3(signature = (mode=None, formats=None))]
    fn validate_open_inputs(
        _cls: &Bound<'_, PyType>,
        mode: Option<&Bound<'_, PyAny>>,
        formats: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        pillow_rs::validate_python_open_inputs(
            open_mode_input_from_python(mode)?,
            open_formats_input_from_python(formats)?,
        )
        .map(|_| ())
        .map_err(map_error)
    }

    #[pyo3(signature = (fp, format=None, compress_level=None))]
    fn save(
        &mut self,
        fp: &Bound<'_, PyAny>,
        format: Option<String>,
        compress_level: Option<i64>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let path = host_path_from_python(fp)?;
        if let Some(path) = path.as_deref() {
            if path.is_dir() {
                return Err(pyo3::exceptions::PyIsADirectoryError::new_err((
                    21,
                    "Is a directory",
                    path.to_string_lossy().to_string(),
                )));
            }
            if path.parent().is_some_and(|parent| !parent.exists()) {
                return Err(pyo3::exceptions::PyFileNotFoundError::new_err((
                    2,
                    "No such file or directory",
                    path.to_string_lossy().to_string(),
                )));
            }
        }
        let extension = path
            .as_deref()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str());
        let format = pillow_rs::Image::resolve_save_format(format.as_deref(), extension)
            .map_err(map_error)?;
        let encoder_options = save_options::encoder_options(&format, compress_level)?;
        let encoded = py
            .detach(|| self.inner.encode_with_options(&format, &encoder_options))
            .map_err(map_error)?;
        if let Some(path) = path {
            py.detach(|| std::fs::write(path, &encoded))
                .map_err(|error| map_error(error.into()))
        } else {
            fp.call_method1("write", (PyBytes::new(fp.py(), &encoded),))?;
            Ok(())
        }
    }

    fn tobytes_unpacked(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        py.detach(|| self.inner.tobytes_unpacked())
            .map_err(map_error)
    }

    fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: Vec<String>,
        py: Python<'_>,
    ) -> PyResult<Py<PyBytes>> {
        let mode = mode.to_owned();
        let encoder_name = encoder_name.to_owned();
        let bytes = py
            .detach(|| {
                self.inner
                    .tobytes_encoded_shared(&mode, &encoder_name, &args)
            })
            .map_err(map_error)?;
        Ok(telemetry::materialize_python_bytes(py, bytes.as_ref()))
    }

    /// Lock a lazy image pipeline to the sole active compute backend.
    ///
    /// This keeps an explicitly selected SIMD or GPU process from silently
    /// benchmarking CPU fallback. Ordinary multi-backend routing is unchanged.
    fn lock_active_backend(&mut self) -> PyResult<PyImage> {
        let active = pillow_rs::active_backends().map_err(map_error)?;
        let already_locked = active
            .first()
            .is_some_and(|backend| self.inner.backend() == Some(*backend));
        if active.len() == 1 && !already_locked {
            // Keep this Python handle's identity stable. Strict parity workflows
            // can retain live views over it across later operations.
            self.inner = self.inner.clone().use_backend(active[0]);
        }
        Ok(PyImage {
            inner: self.inner.clone(),
        })
    }

    fn load(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.load()).map_err(map_error)
    }

    fn seek(&mut self, frame: u32, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.seek(frame)).map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    #[staticmethod]
    #[pyo3(signature = (mode, size, data, decoder_name="raw"))]
    fn frombytes(
        mode: &str,
        size: (u32, u32),
        data: Cow<'_, [u8]>,
        decoder_name: &str,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let mode = mode.to_owned();
        let decoder_name = decoder_name.to_owned();
        // PyO3 borrows immutable bytes and extracts owned vectors for mutable
        // sequences. Preserve that ownership so raw raster modes can adopt an
        // extracted buffer instead of copying it a second time.
        py.detach(move || match data {
            Cow::Borrowed(data) => pillow_rs::image_frombytes(&mode, size, data, &decoder_name),
            Cow::Owned(data) => pillow_rs::image_frombytes_owned(&mode, size, data, &decoder_name),
        })
        .map(|img| PyImage { inner: img })
        .map_err(map_error)
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.verify()).map_err(map_error)
    }
}
