use crate::DecompressionBombError;
use pillow_rs::PilError;
use pyo3::prelude::*;

pub(crate) fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::OsError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::AssertionError(msg) => pyo3::exceptions::PyAssertionError::new_err(msg),
        PilError::IndexError(msg) => pyo3::exceptions::PyIndexError::new_err(msg),
        PilError::KeyError(msg) => pyo3::exceptions::PyKeyError::new_err(msg),
        PilError::KeyErrorInt(key) => pyo3::exceptions::PyKeyError::new_err(key),
        PilError::AttributeError(msg) => pyo3::exceptions::PyAttributeError::new_err(msg),
        PilError::EOFError(msg) => pyo3::exceptions::PyEOFError::new_err(msg),
        PilError::UnsupportedLibraqm => {
            let message = PilError::UnsupportedLibraqm.to_string();
            pyo3::exceptions::PyKeyError::new_err(
                message
                    .strip_prefix('\'')
                    .and_then(|inner| inner.strip_suffix('\''))
                    .unwrap_or(&message)
                    .to_owned(),
            )
        }
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::OverflowError(msg) => pyo3::exceptions::PyOverflowError::new_err(msg),
        PilError::DecompressionBombError(msg) => DecompressionBombError::new_err(msg),
        PilError::MemoryError(msg) => pyo3::exceptions::PyMemoryError::new_err(msg),
        PilError::UnicodeEncodeError {
            encoding,
            object,
            start,
            end,
            reason,
            ..
        } => {
            pyo3::exceptions::PyUnicodeEncodeError::new_err((encoding, object, start, end, reason))
        }
        PilError::SyntaxError(msg) => pyo3::exceptions::PySyntaxError::new_err(msg),
        PilError::SystemError(msg) => pyo3::exceptions::PySystemError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ZeroDivisionError(msg) => pyo3::exceptions::PyZeroDivisionError::new_err(msg),
        // Pillow reports deferred decoder failures (for example, a valid PNG
        // header whose image payload is missing) as OSError from Image.load.
        // Keep the Rust codec error message while preserving that public
        // exception category at the binding boundary.
        PilError::ImageError(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => pyo3::exceptions::PyNotImplementedError::new_err(msg),
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) if err.kind() == std::io::ErrorKind::NotFound => {
            pyo3::exceptions::PyFileNotFoundError::new_err(err.to_string())
        }
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
        // AS PER DESIGN — DO NOT REMOVE: New error variants from Fix 9
        PilError::PaletteError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::InternalError(msg) => pyo3::exceptions::PyException::new_err(msg),
        PilError::DimensionError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
    }
}
