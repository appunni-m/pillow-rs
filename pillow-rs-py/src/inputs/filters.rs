use super::{Bound, PyAny, PyOverflowError, PyResult, PyTypeError, PyValueError, c_int_from_python};
use pyo3::types::PyAnyMethods;

pub(crate) fn filter_size_from_python(
    size: &Bound<'_, PyAny>,
    negative_is_identity: bool,
) -> PyResult<u32> {
    let size = c_int_from_python(size)?;
    if size < 0 {
        if negative_is_identity {
            return Ok(0);
        }
        return Err(PyValueError::new_err("bad kernel size"));
    }
    Ok(size as u32)
}

pub(crate) fn filter_rank_from_python(rank: &Bound<'_, PyAny>) -> PyResult<u32> {
    let rank = c_int_from_python(rank)?;
    if rank < 0 {
        return Err(PyValueError::new_err("bad rank value"));
    }
    Ok(rank as u32)
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
