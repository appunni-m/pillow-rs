use super::RsImage;
use crate::*;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyBufferError;
use pyo3::pymethods;
use pyo3::types::{
    PyAnyMethods, PyByteArray, PyInt, PyList, PyMemoryView, PyString, PyTuple, PyTupleMethods,
};

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn finish_frombytes_payload(
    py: Python<'_>,
    mode: pillow_rs::Mode,
    size: (u32, u32),
    decoder_name: String,
    raw_decoder: Option<pillow_rs::RawDecoderPlan>,
    data: Cow<'_, [u8]>,
) -> PyResult<PyImage> {
    py.detach(move || match (raw_decoder, data) {
        (Some(plan), Cow::Borrowed(data)) => {
            pillow_rs::image_frombytes_with_plan(size, data, &decoder_name, plan)
        }
        (Some(plan), Cow::Owned(data)) => {
            pillow_rs::image_frombytes_owned_with_plan(size, data, &decoder_name, plan)
        }
        (None, Cow::Borrowed(data)) => pillow_rs::image_frombytes(mode, size, data, &decoder_name),
        (None, Cow::Owned(data)) => {
            pillow_rs::image_frombytes_owned(mode, size, data, &decoder_name)
        }
    })
    .map(|inner| PyImage { inner })
    .map_err(map_error)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn raw_decoder_plan_from_python(
    mode: pillow_rs::Mode,
    args: &Bound<'_, PyTuple>,
) -> PyResult<pillow_rs::RawDecoderPlan> {
    let nested_args = if args.len() == 1 {
        args.get_item(0)?.cast::<PyTuple>().ok().cloned()
    } else {
        None
    };
    let args = nested_args.as_ref().unwrap_or(args);
    if args.len() > 3 {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "function takes at most 4 arguments ({} given)",
            args.len() + 1
        )));
    }
    let rawmode = if args.is_empty() {
        None
    } else {
        Some(args.get_item(0)?.extract::<String>()?)
    };
    let stride = if args.len() > 1 {
        c_int_from_python(&args.get_item(1)?)?
    } else {
        0
    };
    let orientation = if args.len() > 2 {
        c_int_from_python(&args.get_item(2)?)?
    } else {
        1
    };
    pillow_rs::RawDecoderPlan::new(mode, rawmode.as_deref(), stride, orientation).map_err(map_error)
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        // Default 1x1 RGB image for compatibility
        let img =
            RsImage::new(1, 1, pillow_rs::Mode::ImageRgb8, (0, 0, 0, 0)).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    #[staticmethod]
    fn _new_palette_lookup_required(
        mode: &Bound<'_, PyAny>,
        color: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let mode_is_palette = mode
            .rich_compare(PyString::new(mode.py(), "P"), CompareOp::Eq)?
            .is_truthy()?;
        let mode = mode_is_palette.then_some(pillow_rs::Mode::ImagePalette8);
        pillow_rs::Image::new_palette_lookup_required(mode, || {
            if !color.is_instance_of::<PyList>() && !color.is_instance_of::<PyTuple>() {
                return Ok(None);
            }
            // Match isinstance/all/len short-circuit order; len runs only
            // after every component passes the integer check.
            for value in color.try_iter()? {
                if !value?.is_instance_of::<PyInt>() {
                    return Ok(None);
                }
            }
            Ok(Some(color.len()?))
        })
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        size: &Bound<'_, PyAny>,
        color: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Self> {
        let size = new_size_from_python(size, py)?;
        let mode = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
            map_error(pillow_rs::PilError::ValueError(
                "unrecognized image mode".into(),
            ))
        })?;
        let input = new_color_input_from_python(mode, color)?;
        let img = RsImage::new_with_input(size.0, size.1, mode, input).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[staticmethod]
    #[pyo3(name = "effect_noise_from_size")]
    fn effect_noise_from_size(size: (u32, u32), sigma: f64) -> PyResult<Self> {
        let image = pillow_rs::image_effect_noise_from_size(size, sigma).map_err(map_error)?;
        Ok(PyImage { inner: image })
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
        let format_names = pillow_rs::validate_open_inputs(mode, formats).map_err(map_error)?;
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
            pillow_rs::validate_open_source_bytes(bytes.as_bytes()).map_err(map_error)?;
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
        pillow_rs::validate_open_inputs(
            open_mode_input_from_python(mode)?,
            open_formats_input_from_python(formats)?,
        )
        .map(|_| ())
        .map_err(map_error)
    }

    #[pyo3(signature = (fp, format=None, compress_level=None, before_encode=None))]
    fn save(
        slf: Py<Self>,
        fp: &Bound<'_, PyAny>,
        format: Option<&Bound<'_, PyAny>>,
        compress_level: Option<&Bound<'_, PyAny>>,
        before_encode: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        // TODO(pillow-rs#3ptb): freeze file-object name/extension, writer-option,
        // and path/write fault precedence in CPU parity.
        // Preserve format extraction before PathLike conversion. Compression
        // conversion follows the wrapper load and filesystem preflight below.
        let format = format
            .map(|value| value.extract::<String>())
            .transpose()?
            .filter(|format| !format.is_empty());
        let path = host_path_from_python(fp)?;
        let extension = path
            .as_deref()
            .and_then(|path| path.extension())
            .and_then(|extension| extension.to_str());
        let extension_format = if format.is_none() {
            Some(pillow_rs::Image::resolve_save_format(None, extension).map_err(map_error)?)
        } else {
            None
        };
        if let Some(before_encode) = before_encode {
            // The Python Image wrapper injects its virtual load method here so
            // dirty palettes are realized after format discovery and before
            // the Rust encoder reads the image. No PyImage borrow is active.
            before_encode.call0()?;
        }
        let format = match format {
            Some(format) => pillow_rs::Image::resolve_save_format(Some(&format), extension)
                .map_err(map_error)?,
            None => extension_format.ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("save format extension was not resolved")
            })?,
        };
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
        let compress_level = compress_level
            .map(|value| value.extract::<i64>())
            .transpose()?;
        let encoder_options = save_options::encoder_options(&format, compress_level)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let encoded = py
            .detach(move || inner.encode_with_options(&format, &encoder_options))
            .map_err(map_error)?;
        if let Some(path) = path {
            py.detach(|| std::fs::write(path, &encoded))
                .map_err(|error| map_error(error.into()))
        } else {
            fp.call_method1("write", (PyBytes::new(fp.py(), &encoded),))?;
            Ok(())
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
    fn tobytes_unpacked(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        py.detach(|| self.inner.tobytes_unpacked())
            .map_err(map_error)
    }

    fn tobytes_encoded(
        slf: Py<Self>,
        encoder_name: &str,
        args: Vec<String>,
        py: Python<'_>,
    ) -> PyResult<Py<PyBytes>> {
        let inner = slf.bind(py).borrow().inner.clone();
        let mode = inner.mode_identity().map_err(map_error)?;
        let encoder_name = encoder_name.to_owned();
        let bytes = py
            .detach(move || inner.tobytes_encoded_shared(mode, &encoder_name, &args))
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

    fn seek(slf: Py<Self>, frame: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let frame = frame.extract::<u32>()?;
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let inner = inner;
            let result = inner.seek(frame);
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    #[staticmethod]
    #[pyo3(signature = (mode, size, data, decoder_name="raw", *args))]
    fn frombytes(
        mode: &Bound<'_, PyAny>,
        size: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
        decoder_name: &str,
        args: &Bound<'_, PyTuple>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (width, height) = nonnegative_output_size_items_from_python(size)?;
        let mode_name = mode.extract::<String>()?;
        // NO_PILLOW_SUPPORT: accept Rust-only mode names only for typed raw input.
        let mode = pillow_rs::Mode::from_pillow_name(&mode_name)
            .or_else(|| pillow_rs::Mode::from_rust_name(&mode_name))
            .ok_or_else(|| {
                map_error(pillow_rs::PilError::ValueError(
                    "unrecognized image mode".into(),
                ))
            })?;
        let size = frombytes_size_from_python(&width, &height)?;
        if size.0 == 0 || size.1 == 0 {
            return finish_frombytes_payload(
                py,
                mode,
                size,
                "raw".to_owned(),
                None,
                Cow::Borrowed(&[]),
            );
        }
        if decoder_name != "raw" {
            return Err(map_error(pillow_rs::PilError::IOError(format!(
                "decoder {decoder_name} not available"
            ))));
        }
        let decoder_name = decoder_name.to_owned();
        // Pillow creates and validates the selected decoder before asking the
        // input object for a buffer. Keep rawmode errors at that boundary.
        let raw_decoder = raw_decoder_plan_from_python(mode, args)?;
        if let Ok(bytes) = data.cast::<PyBytes>() {
            return finish_frombytes_payload(
                py,
                mode,
                size,
                decoder_name,
                Some(raw_decoder),
                Cow::Borrowed(bytes.as_bytes()),
            );
        }
        if data.is_instance_of::<PyByteArray>() {
            return finish_frombytes_payload(
                py,
                mode,
                size,
                decoder_name,
                Some(raw_decoder),
                Cow::Owned(data.extract::<Vec<u8>>()?),
            );
        }
        let memoryview = PyMemoryView::from(data).map_err(|error| {
            if error.is_instance_of::<PyTypeError>(py) {
                if let Ok(message) = error
                    .value(py)
                    .str()
                    .and_then(|value| value.extract::<String>())
                {
                    if let Some(message) = message.strip_prefix("memoryview: ") {
                        return PyTypeError::new_err(message.to_owned());
                    }
                }
            }
            error
        })?;
        // Pillow 13's `_imaging.c::PyImaging_GetBuffer` requests
        // `PyBUF_SIMPLE` (https://github.com/python-pillow/Pillow/blob/13.0.0/src/_imaging.c#L2519-L2522),
        // which requires C-contiguous storage. `memoryview.tobytes()` alone
        // would flatten a strided view and accept input that Pillow rejects.
        let c_contiguous = memoryview.getattr("c_contiguous")?.extract::<bool>()?;
        if !c_contiguous {
            return Err(PyBufferError::new_err(
                "memoryview: underlying buffer is not C-contiguous",
            ));
        }
        let decoded_bytes = memoryview.call_method0("tobytes")?;
        let decoded_bytes = decoded_bytes.cast::<PyBytes>()?;
        finish_frombytes_payload(
            py,
            mode,
            size,
            decoder_name,
            Some(raw_decoder),
            Cow::Borrowed(decoded_bytes.as_bytes()),
        )
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.verify()).map_err(map_error)
    }
}
