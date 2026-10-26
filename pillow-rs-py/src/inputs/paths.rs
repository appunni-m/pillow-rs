use super::{
    Bound, Py, PyAny, PyBytes, PyErr, PyResult, PyString, PyValueError, Python, map_error,
};
use pyo3::types::{PyAnyMethods, PyBytesMethods};
use std::path::PathBuf;

pub(crate) fn host_path_from_python(value: &Bound<'_, PyAny>) -> PyResult<Option<PathBuf>> {
    if let Ok(path) = value.extract::<PathBuf>() {
        return Ok(Some(path));
    }
    let Ok(bytes) = value.cast::<PyBytes>() else {
        return Ok(None);
    };
    if bytes.as_bytes().contains(&0) {
        return Err(PyValueError::new_err("embedded null byte"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(Some(std::ffi::OsStr::from_bytes(bytes.as_bytes()).into()))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.as_bytes().to_vec())
            .map(PathBuf::from)
            .map(Some)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }
}

pub(crate) fn map_open_path_error(
    py: Python<'_>,
    original: &Bound<'_, PyAny>,
    path: &std::path::Path,
    error: std::io::Error,
) -> PyErr {
    if error.kind() == std::io::ErrorKind::NotFound {
        // Pillow keeps a bytes path as a bytes object in the public OSError
        // tuple. Preserve that host representation while Rust owns the actual
        // filesystem lookup.
        let filename: Py<PyAny> = if let Ok(bytes) = original.cast::<PyBytes>() {
            PyBytes::new(py, bytes.as_bytes()).into()
        } else {
            PyString::new(py, &path.to_string_lossy()).into()
        };
        return pyo3::exceptions::PyFileNotFoundError::new_err((
            error.raw_os_error().unwrap_or(2),
            "No such file or directory",
            filename,
        ));
    }
    if error.kind() == std::io::ErrorKind::IsADirectory {
        return pyo3::exceptions::PyIsADirectoryError::new_err((
            error.raw_os_error().unwrap_or(21),
            "Is a directory",
            path.to_string_lossy().to_string(),
        ));
    }
    map_error(error.into())
}
