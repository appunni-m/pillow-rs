use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

// --- ImageFilter helper functions ---

/// Validate and normalize 3D LUT size. Accepts int (converted to 3-tuple)
/// or sequence of 3 ints. Each dimension must be in [2, 65].
#[pyfunction]
fn color3dlut_check_size(size: &Bound<'_, PyAny>) -> PyResult<(u32, u32, u32)> {
    let values = match size.extract::<Vec<f64>>() {
        Ok(values) => values,
        Err(_) => vec![size.extract::<f64>().map_err(|_| {
            PyValueError::new_err("Size should be either an integer or a tuple of three integers.")
        })?],
    };
    pillow_rs::color3dlut_check_size(&values).map_err(map_error)
}

/// Validate and flatten a 3D LUT table from Python object.
/// Handles flat list of floats and list of tuples.
/// Always returns a flat Vec<f64> of validated length.
#[pyfunction]
#[pyo3(signature = (table_obj, size, channels=3, copy_table=true))]
fn color3dlut_new(
    table_obj: &Bound<'_, PyAny>,
    size: (u32, u32, u32),
    channels: u32,
    copy_table: bool,
) -> PyResult<Vec<f64>> {
    let _ = copy_table;
    let table = if let Ok(table) = table_obj.extract::<Vec<Vec<f64>>>() {
        pillow_rs::Color3DLutTable::Nested(table)
    } else {
        pillow_rs::Color3DLutTable::Flat(table_obj.extract::<Vec<f64>>().map_err(|_| {
            PyValueError::new_err(
                "Table must be a sequence of floats or a sequence of tuples of floats.",
            )
        })?)
    };
    pillow_rs::color3dlut_prepare_table(table, size, channels).map_err(map_error)
}

/// Generate a 3D LUT table by calling a Python callback for each position.
/// The callback receives (r, g, b) normalized to [0, 1] and returns a tuple
/// of `channels` floats.
#[pyfunction]
fn color3dlut_generate(
    size: (u32, u32, u32),
    channels: u32,
    callback: Py<PyAny>,
    py: Python,
) -> PyResult<Vec<f64>> {
    pillow_rs::color3dlut_generate_table(
        size,
        channels,
        |args| {
            let point = PyTuple::new(py, args)?;
            callback.call(py, &point, None)?.extract(py)
        },
        map_error,
    )
}

/// Transform a 3D LUT table by calling a Python callback for each entry.
/// If `with_normals` is true, the callback receives (r_norm, g_norm, b_norm, *values).
/// Otherwise, receives (*values).
/// The callback returns a tuple of `channels_out` floats.
#[pyfunction]
fn color3dlut_transform(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels_in: u32,
    channels_out: Option<u32>,
    with_normals: bool,
    callback: Py<PyAny>,
    py: Python,
) -> PyResult<(Vec<f64>, u32)> {
    let input = pillow_rs::prepare_color3dlut(table, size, channels_in).map_err(map_error)?;
    pillow_rs::color3dlut_transform_table(
        &input,
        channels_out,
        with_normals,
        |args| {
            let point = PyTuple::new(py, args)?;
            callback.call(py, &point, None)?.extract(py)
        },
        map_error,
    )
}

#[pyfunction]
fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    pillow_rs::color3dlut_repr(table_type, size, channels, target_mode)
}

#[pyfunction]
fn kernel_validate_coefficients(kernel: Option<Vec<f64>>, size: (u32, u32)) -> PyResult<()> {
    pillow_rs::validate_kernel_coefficients(kernel.as_deref(), size).map_err(map_error)
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(color3dlut_check_size, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_new, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_generate, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_transform, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_repr, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_validate_coefficients, m)?)?;
    Ok(())
}
