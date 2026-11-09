use pyo3::exceptions::{PyMemoryError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{
    PyAnyMethods, PyDict, PyList, PyListMethods, PyMapping, PyMemoryView, PySequence, PyTuple,
    PyTupleMethods,
};

const WRONG_TABLE_LENGTH: &str =
    "The table should have table_channels * size1D * size2D * size3D float items.";

/// Read an ImagingCore Color3DLUT table from Pillow's buffer or sequence
/// inputs, preserving its buffer fast path and CPython sequence protocol.
pub(crate) fn color_lut_table_from_python(
    table: &Bound<'_, PyAny>,
    expected_length: usize,
) -> PyResult<Vec<f64>> {
    if let Some(values) = color_lut_float_buffer(table, expected_length) {
        return Ok(values);
    }

    color_lut_sequence_values(table, expected_length)
}

fn color_lut_sequence_values(
    table: &Bound<'_, PyAny>,
    expected_length: usize,
) -> PyResult<Vec<f64>> {
    // Accept registered sequences and custom indexed sequences, while
    // rejecting mapping-only inputs such as dictionaries.
    let known_sequence = table.cast::<PySequence>().is_ok();
    let custom_sequence = table.get_type().hasattr("__getitem__")?
        && table.cast::<PyMapping>().is_err()
        && table.cast::<PyDict>().is_err();
    if !known_sequence && !custom_sequence {
        return Err(PyTypeError::new_err("argument must be a sequence"));
    }

    // Pillow replaces a failed PySequence_Size with its wrong-length ValueError
    // because that result cannot equal the required length.
    if !matches!(table.len(), Ok(length) if length == expected_length) {
        return Err(PyValueError::new_err(WRONG_TABLE_LENGTH));
    }

    let mut values = Vec::new();
    values
        .try_reserve_exact(expected_length)
        .map_err(|_| PyMemoryError::new_err("unable to allocate Color3DLUT table"))?;

    // PySequence_Fast keeps list/tuple storage live and snapshots every other
    // sequence before converting any numeric item. Preserve those callback
    // boundaries while keeping PyFloat_AsDouble-equivalent extraction in Rust.
    let snapshot =
        if table.is_exact_instance_of::<PyList>() || table.is_exact_instance_of::<PyTuple>() {
            None
        } else {
            Some(color_lut_sequence_snapshot(table, expected_length)?)
        };

    let mut last_conversion_error = None;
    for index in 0..expected_length {
        let item = match snapshot.as_ref() {
            Some(items) => items
                .get(index)
                .cloned()
                .ok_or_else(|| PyValueError::new_err(WRONG_TABLE_LENGTH))?,
            None => color_lut_fast_sequence_item(table, index)?,
        };
        match item.extract::<f64>() {
            Ok(value) => values.push(value),
            Err(error) => {
                values.push(0.0);
                last_conversion_error = Some(error);
            }
        }
    }

    if let Some(error) = last_conversion_error {
        return Err(error);
    }
    Ok(values)
}

fn color_lut_sequence_snapshot<'py>(
    table: &Bound<'py, PyAny>,
    expected_length: usize,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut items = Vec::new();
    items
        .try_reserve_exact(expected_length)
        .map_err(|_| PyMemoryError::new_err("unable to allocate Color3DLUT sequence"))?;
    for item in table.try_iter()? {
        items
            .try_reserve(1)
            .map_err(|_| PyMemoryError::new_err("unable to allocate Color3DLUT sequence"))?;
        items.push(item?);
    }
    Ok(items)
}

fn color_lut_fast_sequence_item<'py>(
    table: &Bound<'py, PyAny>,
    index: usize,
) -> PyResult<Bound<'py, PyAny>> {
    if let Ok(list) = table.cast::<PyList>() {
        return list.get_item(index);
    }
    if let Ok(tuple) = table.cast::<PyTuple>() {
        return tuple.get_item(index);
    }
    table.get_item(index)
}

fn color_lut_float_buffer(value: &Bound<'_, PyAny>, expected_length: usize) -> Option<Vec<f64>> {
    let memoryview = PyMemoryView::from(value).ok()?;
    if !memoryview
        .getattr("c_contiguous")
        .ok()?
        .extract::<bool>()
        .ok()?
        || memoryview.getattr("ndim").ok()?.extract::<usize>().ok()? != 1
    {
        return None;
    }

    let shape = memoryview
        .getattr("shape")
        .ok()?
        .extract::<Vec<usize>>()
        .ok()?;
    if shape.len() != 1 || shape[0] != expected_length {
        return None;
    }

    let format = memoryview
        .getattr("format")
        .ok()?
        .extract::<String>()
        .ok()?;
    let bytes_per_item = match format.as_str() {
        "e" => 2,
        "f" => 4,
        "d" => 8,
        _ => return None,
    };
    if memoryview.getattr("nbytes").ok()?.extract::<usize>().ok()?
        != expected_length * bytes_per_item
    {
        return None;
    }
    let byte_view = memoryview.call_method1("cast", ("B",)).ok()?;
    let bytes = byte_view
        .call_method0("tobytes")
        .ok()?
        .extract::<Vec<u8>>()
        .ok()?;

    match format.as_str() {
        "e" => Some(
            bytes
                .chunks_exact(2)
                .map(|bytes| {
                    f64::from(pillow_float16_to_float32(u16::from_ne_bytes([
                        bytes[0], bytes[1],
                    ])))
                })
                .collect(),
        ),
        "f" => Some(
            bytes
                .chunks_exact(4)
                .map(|bytes| {
                    f64::from(f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                })
                .collect(),
        ),
        "d" => Some(
            bytes
                .chunks_exact(8)
                .map(|bytes| {
                    f64::from_ne_bytes([
                        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                        bytes[7],
                    ])
                })
                .collect(),
        ),
        _ => None,
    }
}

/// Pillow 12.2's ColorLUT half-buffer adapter treats subnormals as zero and
/// applies its historical exponent adjustment to all other bit patterns.
fn pillow_float16_to_float32(value: u16) -> f32 {
    let exponent = u32::from(value & 0x7c00);
    let sign = u32::from(value & 0x8000) << 16;
    let mut bits = u32::from(value & 0x7fff) << 13;
    bits = bits.wrapping_add(0x3800_0000);
    if exponent == 0 {
        bits = 0;
    }
    f32::from_bits(bits | sign)
}
