use super::{Bound, PyAny, PyResult, PyTypeError, PyValueError, c_int_from_python};
use pyo3::types::{PyAnyMethods, PyTypeMethods};

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
    pillow_rs::arm_fault(point).map_err(PyValueError::new_err)
}

#[cfg(feature = "test-api")]
#[pyo3::pyfunction]
pub(crate) fn _clear_migration_test_fault() {
    pillow_rs::clear_fault();
}

// TODO(pillow-rs#3ptb): freeze private ImagingCore radius sequence and
// element-conversion errors against the Pillow parity receipt.
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
pub(crate) fn blur_radius_pair_from_python(radius: &Bound<'_, PyAny>) -> PyResult<(f64, f64)> {
    let type_name = radius.get_type().name()?.to_string();
    let values = radius
        .try_iter()
        .map_err(|_| {
            PyTypeError::new_err(format!(
                "argument 1 must be sequence of length 2, not {type_name}"
            ))
        })?
        .collect::<PyResult<Vec<_>>>()?;
    if values.len() != 2 {
        return Err(PyTypeError::new_err(format!(
            "argument 1 must be sequence of length 2, not {}",
            values.len()
        )));
    }
    let radius_x = values[0].extract::<f64>()?;
    let radius_y = values[1].extract::<f64>()?;
    Ok((radius_x, radius_y))
}
