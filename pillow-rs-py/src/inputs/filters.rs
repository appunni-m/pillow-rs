use super::{Bound, PyAny, PyOverflowError, PyResult, PyTypeError, PyValueError};
use pyo3::types::PyAnyMethods;

pub(crate) fn filter_size_from_python(size: i64, negative_is_identity: bool) -> PyResult<u32> {
    if size < 0 {
        if negative_is_identity {
            return Ok(0);
        }
        return Err(PyValueError::new_err("bad kernel size"));
    }
    u32::try_from(size)
        .map_err(|_| PyOverflowError::new_err("Python int too large to convert to C ssize_t"))
}

pub(crate) fn filter_rank_from_python(rank: i64) -> PyResult<u32> {
    if rank < 0 {
        return Err(PyValueError::new_err("bad rank value"));
    }
    u32::try_from(rank)
        .map_err(|_| PyOverflowError::new_err("Python int too large to convert to C ssize_t"))
}

#[cfg(feature = "test-api")]
#[pyo3::pyfunction]
pub(crate) fn _arm_migration_test_fault(point: &str) -> PyResult<()> {
    pillow_rs::test_api::arm_fault(point).map_err(PyValueError::new_err)
}

#[cfg(feature = "test-api")]
#[pyo3::pyfunction]
pub(crate) fn _clear_migration_test_fault() {
    pillow_rs::test_api::clear_fault();
}

pub(crate) fn blur_radius_pair_from_python(
    radius: Option<&Bound<'_, PyAny>>,
    default: f64,
) -> PyResult<(f64, f64)> {
    match radius {
        None => Ok((default, default)),
        Some(value) => {
            if let Ok(radius) = value.extract::<f64>() {
                return Ok((radius, radius));
            }
            let values = value.extract::<Vec<f64>>().map_err(|_| {
                PyTypeError::new_err("radius must be a number or a sequence of two numbers")
            })?;
            if values.len() != 2 {
                return Err(PyTypeError::new_err(format!(
                    "argument 1 must be sequence of length 2, not {}",
                    values.len()
                )));
            }
            Ok((values[0], values[1]))
        }
    }
}
