use super::{Bound, Py, PyAny, PyBytes, PyErr, PyResult, PyString, PyValueError, Python};
use pyo3::types::{PyAnyMethods, PyBytesMethods};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostPathKind {
    Text,
    Bytes,
}

pub(crate) struct HostPath {
    pub(crate) path: PathBuf,
    pub(crate) kind: HostPathKind,
    pub(crate) value: Py<PyAny>,
}

impl HostPath {
    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            path: self.path.clone(),
            kind: self.kind,
            value: self.value.clone_ref(py),
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// TODO(pillow-rs#3ptb): verify str, bytes, and PathLike outputs, null-path
// error order, and original filename payloads in public open/save parity.
pub(crate) fn host_path_from_python(value: &Bound<'_, PyAny>) -> PyResult<Option<HostPath>> {
    if !host_path_is_pathlike(value)? {
        return Ok(None);
    }
    host_path_from_fspath(value).map(Some)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// TODO(pillow-rs#3ptb): verify str, bytes, and PathLike classification in the
// save `.name` property access matrix.
pub(crate) fn host_path_is_pathlike(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    if value.is_instance_of::<PyString>() || value.cast::<PyBytes>().is_ok() {
        return Ok(true);
    }
    let pathlike = value.py().import("os")?.getattr("PathLike")?;
    value.is_instance(&pathlike)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn host_path_from_fspath(value: &Bound<'_, PyAny>) -> PyResult<HostPath> {
    let path_value = value.py().import("os")?.call_method1("fspath", (value,))?;
    host_path_from_fspath_value(path_value)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn host_path_from_fspath_value(path_value: Bound<'_, PyAny>) -> PyResult<HostPath> {
    if let Ok(bytes) = path_value.cast::<PyBytes>() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let path = std::ffi::OsStr::from_bytes(bytes.as_bytes()).into();
            let value = path_value.unbind();
            return Ok(HostPath {
                path,
                kind: HostPathKind::Bytes,
                value,
            });
        }
        #[cfg(windows)]
        {
            let decoded = path_value
                .py()
                .import("os")?
                .call_method1("fsdecode", (&path_value,))?;
            let text = decoded.cast::<PyString>()?;
            let path = windows_path_from_python_string(&text)?;
            return Ok(HostPath {
                path,
                kind: HostPathKind::Bytes,
                value: path_value.unbind(),
            });
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let path = String::from_utf8(bytes.as_bytes().to_vec())
                .map_err(|error| PyValueError::new_err(error.to_string()))?;
            return Ok(HostPath {
                path: PathBuf::from(path),
                kind: HostPathKind::Bytes,
                value: path_value.unbind(),
            });
        }
    }
    if path_value.cast::<PyString>().is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let encoded_value = path_value
                .py()
                .import("os")?
                .call_method1("fsencode", (&path_value,))?;
            let encoded_bytes = encoded_value.cast::<PyBytes>()?;
            let path = std::ffi::OsStr::from_bytes(encoded_bytes.as_bytes()).into();
            let value = path_value.unbind();
            return Ok(HostPath {
                path,
                kind: HostPathKind::Text,
                value,
            });
        }
        #[cfg(windows)]
        {
            let text = path_value.cast::<PyString>()?;
            let path = windows_path_from_python_string(&text)?;
            return Ok(HostPath {
                path,
                kind: HostPathKind::Text,
                value: path_value.unbind(),
            });
        }
        #[cfg(all(not(unix), not(windows)))]
        {
            let text = path_value.cast::<PyString>()?;
            let path = text.extract::<String>()?;
            return Ok(HostPath {
                path: PathBuf::from(path),
                kind: HostPathKind::Text,
                value: path_value.unbind(),
            });
        }
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        "expected str, bytes or os.PathLike object",
    ))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(windows)]
fn windows_path_from_python_string(value: &Bound<'_, PyString>) -> PyResult<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    let encoded = value.call_method1("encode", ("utf-16-le", "surrogatepass"))?;
    let bytes = encoded.cast::<PyBytes>()?.as_bytes();
    let wide = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Ok(std::ffi::OsString::from_wide(&wide).into())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// TODO(pillow-rs#3ptb): verify errno, strerror, subclass, and filename tuple
// parity for filesystem failures from public open/save paths.
pub(crate) fn map_host_path_error(py: Python<'_>, path: &HostPath, error: std::io::Error) -> PyErr {
    if error.kind() == std::io::ErrorKind::InvalidInput && host_path_contains_null(path) {
        return PyValueError::new_err("embedded null byte");
    }
    let errno = error.raw_os_error().unwrap_or_else(|| match error.kind() {
        std::io::ErrorKind::NotFound => 2,
        std::io::ErrorKind::PermissionDenied => 13,
        std::io::ErrorKind::IsADirectory => 21,
        _ => 5,
    });
    let strerror = py
        .import("os")
        .and_then(|os| os.call_method1("strerror", (errno,)))
        .and_then(|message| message.extract::<String>())
        .unwrap_or_else(|_| error.to_string());
    let filename = host_path_value(py, path);
    match error.kind() {
        std::io::ErrorKind::NotFound => {
            pyo3::exceptions::PyFileNotFoundError::new_err((errno, strerror, filename))
        }
        std::io::ErrorKind::PermissionDenied => {
            pyo3::exceptions::PyPermissionError::new_err((errno, strerror, filename))
        }
        std::io::ErrorKind::IsADirectory => {
            pyo3::exceptions::PyIsADirectoryError::new_err((errno, strerror, filename))
        }
        _ => pyo3::exceptions::PyOSError::new_err((errno, strerror, filename)),
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn host_path_contains_null(path: &HostPath) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.path.as_os_str().as_bytes().contains(&0)
    }
    #[cfg(not(unix))]
    {
        path.path.to_string_lossy().contains('\0')
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn host_path_value(py: Python<'_>, path: &HostPath) -> Py<PyAny> {
    path.value.clone_ref(py)
}
