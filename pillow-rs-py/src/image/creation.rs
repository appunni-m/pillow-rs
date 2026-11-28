use super::RsImage;
use super::execution::{
    PyExecutionContext, PythonExecutionRoute, route_from_context, route_pending_image,
};
use crate::*;
use pyo3::PyTypeInfo;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyAttributeError, PyBufferError};
use pyo3::pymethods;
use pyo3::types::{
    PyAnyMethods, PyByteArray, PyInt, PyList, PyMemoryView, PyString, PyTuple, PyTupleMethods,
};
use std::io::Write;

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

struct SaveTarget<'py> {
    path: Option<HostPath>,
    filename_path: Option<HostPath>,
    writer: Bound<'py, PyAny>,
}

struct SaveOutputFile {
    file: std::fs::File,
    remove_on_failure: Option<HostPath>,
}

impl SaveOutputFile {
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn write_all(self, py: Python<'_>, bytes: Vec<u8>) -> (std::io::Result<()>, Self) {
        let Self {
            mut file,
            remove_on_failure,
        } = self;
        let (result, file) = py.detach(move || {
            let result = file.write_all(&bytes);
            (result, file)
        });
        (
            result,
            Self {
                file,
                remove_on_failure,
            },
        )
    }

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    fn cleanup_after_failure(self, py: Python<'_>) -> PyResult<()> {
        let Self {
            file,
            remove_on_failure,
        } = self;
        drop(file);
        let Some(path) = remove_on_failure else {
            return Ok(());
        };
        match std::fs::remove_file(&path.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => Ok(()),
            Err(error) => Err(map_host_path_error(py, &path, error)),
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn save_target_from_python<'py>(
    fp: &Bound<'py, PyAny>,
    py: Python<'py>,
) -> PyResult<SaveTarget<'py>> {
    let mut path = host_path_from_python(fp)?;
    let mut writer = fp.clone();
    if path.is_none() {
        let stdout = py.import("sys")?.getattr("stdout")?;
        let text_io_wrapper = py.import("io")?.getattr("TextIOWrapper")?;
        if fp.eq(&stdout)? && stdout.is_instance(&text_io_wrapper)? {
            // Pillow redirects this public file-like target to its binary stream.
            writer = stdout.getattr("buffer")?;
        }
    }

    let mut filename_path = path.as_ref().map(|path| path.clone_ref(py));
    if !filename_path
        .as_ref()
        .is_some_and(|path| !path.path.as_os_str().is_empty())
    {
        match writer.getattr("name") {
            Ok(_) => {
                let name_for_check = writer.getattr("name")?;
                if host_path_is_pathlike(&name_for_check)? {
                    let name_for_path = writer.getattr("name")?;
                    filename_path = Some(host_path_from_fspath(&name_for_path)?);
                    if path
                        .as_ref()
                        .is_some_and(|path| path.path.as_os_str().is_empty())
                    {
                        if let Some(filename) = filename_path.as_ref() {
                            path = Some(filename.clone_ref(py));
                        }
                    }
                }
            }
            Err(error) if error.is_instance_of::<PyAttributeError>(py) => {}
            Err(error) => return Err(error),
        }
    }

    Ok(SaveTarget {
        path,
        filename_path,
        writer,
    })
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn save_extension_from_target(target: &SaveTarget<'_>, py: Python<'_>) -> PyResult<Option<String>> {
    let Some(filename_path) = target.filename_path.as_ref() else {
        return Ok(None);
    };
    let os_path = py.import("os")?.getattr("path")?;
    let extension = os_path
        .call_method1("splitext", (filename_path.value.bind(py),))?
        .get_item(1)?;
    match filename_path.kind {
        HostPathKind::Text => {
            let extension = extension.call_method0("lower")?;
            match extension.extract::<String>() {
                Ok(extension) => Ok(Some(extension)),
                Err(error)
                    if error.is_instance_of::<pyo3::exceptions::PyUnicodeEncodeError>(py) =>
                {
                    let message = PyString::new(py, "unknown file extension: ")
                        .call_method1("__add__", (&extension,))?;
                    Err(PyErr::from_value(
                        pyo3::exceptions::PyValueError::type_object(py)
                            .call1((message,))?
                            .into_any(),
                    ))
                }
                Err(error) => Err(error),
            }
        }
        HostPathKind::Bytes => extension
            .call_method0("lower")?
            .call_method0("decode")?
            .extract()
            .map(Some),
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn save_format_from_python(
    format: Option<&Bound<'_, PyAny>>,
    target: &SaveTarget<'_>,
    before_encode: Option<&Bound<'_, PyAny>>,
    py: Python<'_>,
) -> PyResult<String> {
    let explicit_format = match format {
        Some(value) if value.is_truthy()? => Some(value),
        _ => None,
    };
    let extension = if explicit_format.is_none() {
        save_extension_from_target(target, py)?
    } else {
        None
    };
    let extension_format = if explicit_format.is_none() {
        Some(pillow_rs::Image::resolve_save_format(None, extension.as_deref()).map_err(map_error)?)
    } else {
        None
    };
    if let Some(before_encode) = before_encode {
        // Realize wrapper-owned lazy metadata after format discovery, before encoding.
        before_encode.call0()?;
    }
    match explicit_format {
        Some(format) => {
            let format = format.call_method0("upper")?.extract::<String>()?;
            pillow_rs::Image::resolve_save_format(Some(&format), None).map_err(map_error)
        }
        None => extension_format.ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("save format extension was not resolved")
        }),
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn create_save_file(path: Option<&HostPath>, py: Python<'_>) -> PyResult<Option<SaveOutputFile>> {
    if let Some(path) = path {
        if host_path_contains_null(path) {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "embedded null byte",
            ));
        }
    }
    let Some(path) = path else {
        return Ok(None);
    };
    let remove_on_failure = (!path.path.exists()).then(|| path.clone_ref(py));
    std::fs::File::create(&path.path)
        .map(|file| {
            Some(SaveOutputFile {
                file,
                remove_on_failure,
            })
        })
        .map_err(|error| map_host_path_error(py, path, error))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn save_execution_route(
    inner: &pillow_rs::Image,
    provider: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<PythonExecutionRoute>> {
    if inner.is_materialized() {
        return Ok(None);
    }
    let Some(provider) = provider else {
        return Ok(None);
    };
    let context = provider.call0()?;
    if context.is_none() {
        return Ok(None);
    }
    let context = context.extract::<pyo3::PyRef<'_, PyExecutionContext>>()?;
    Ok(Some(PythonExecutionRoute {
        requested_backend: context.requested_backend,
        pinned_backend: context.pinned_backend,
        context: context.inner,
    }))
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

    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
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
                .detach(|| std::fs::read(&path.path))
                .map_err(|error| map_host_path_error(py, &path, error))?;
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

    // TODO(pillow-rs#3ptb): verify file-like callback and partial-write
    // behavior, encoder faults, and route receipts in CPU parity.
    // VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
    #[pyo3(signature = (
        fp,
        format=None,
        compress_level=None,
        before_encode=None,
        execution_context_provider=None
    ))]
    fn save(
        slf: Py<Self>,
        fp: &Bound<'_, PyAny>,
        format: Option<&Bound<'_, PyAny>>,
        compress_level: Option<&Bound<'_, PyAny>>,
        before_encode: Option<&Bound<'_, PyAny>>,
        execution_context_provider: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let target = save_target_from_python(fp, py)?;
        let format = save_format_from_python(format, &target, before_encode, py)?;
        let mut output_file = create_save_file(target.path.as_ref(), py)?;
        let save_result = (|| {
            let compress_level = compress_level
                .map(|value| value.extract::<i64>())
                .transpose()?;
            let encoder_options = save_options::encoder_options(&format, compress_level)?;
            let inner = slf.bind(py).borrow().inner.clone();
            let execution_route = save_execution_route(&inner, execution_context_provider)?;
            let inner = route_pending_image(inner, execution_route);
            let encoded = py
                .detach(move || inner.encode_with_options(&format, &encoder_options))
                .map_err(map_error)?;
            if let Some(file_output) = output_file.take() {
                let (write_result, file) = file_output.write_all(py, encoded);
                output_file = Some(file);
                write_result.map_err(|error| map_error(error.into()))?;
            } else {
                target
                    .writer
                    .call_method1("write", (PyBytes::new(target.writer.py(), &encoded),))?;
            }
            Ok(())
        })();
        if let Err(error) = save_result {
            if let Some(output_file) = output_file.take() {
                output_file.cleanup_after_failure(py)?;
            }
            return Err(error);
        }
        Ok(())
    }

    // TODO(pillow-rs#3ptb): verify unpacked font-mask bytes and selected/fallback
    // receipts when the mask image is still pending.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn tobytes_unpacked(&self, py: Python<'_>) -> PyResult<Vec<u8>> {
        let route = route_from_context(None, py);
        let inner = route_pending_image(self.inner.clone(), route);
        py.detach(move || inner.tobytes_unpacked())
            .map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify encoder fault order, route receipts, exact
    // encoded bytes, and public-call latency in the frozen CPU matrix.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (encoder_name, args, execution_context=None))]
    fn tobytes_encoded(
        slf: Py<Self>,
        encoder_name: &str,
        args: Vec<String>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyBytes>> {
        let execution_route = route_from_context(execution_context, py);
        let inner = slf.bind(py).borrow().inner.clone();
        let inner = route_pending_image(inner, execution_route);
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

    // TODO(pillow-rs#3ptb): verify selected/fallback load receipts and errors
    // at the wrapper load boundary.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn load(
        &mut self,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let execution_route = route_from_context(execution_context, py);
        let mut inner = self.inner.clone();
        inner = py
            .detach(move || {
                let mut inner = route_pending_image(inner, execution_route);
                inner.load()?;
                Ok(inner)
            })
            .map_err(map_error)?;
        self.inner = inner;
        Ok(())
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
