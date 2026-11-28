//! Arrow C Data Interface input for the public `Image.fromarrow` wrapper.
#![allow(unsafe_code)] // Arrow's C ABI validates foreign pointers at this boundary.

use std::ffi::{CStr, c_char, c_void};
use std::slice;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::ffi;
use pyo3::prelude::{Bound, PyAny, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyAnyMethods, PyBytes, PySequence, PySequenceMethods, PyTypeMethods};
use pyo3::wrap_pyfunction;

use super::abi::{ArrowArray, ArrowSchema};
use crate::image::PyImage;
use crate::{c_int_from_python, map_error};
use pillow_rs::ArrowImportLayout;

const ARROW_ARRAY_BUFFER_ERROR: &str = "Arrow Array, exactly 2 buffers required";
const INVALID_ARROW_ARRAY_ERROR: &str = "Invalid Arrow array mode or size mismatch";

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[pyfunction]
pub(super) fn image_from_arrow(
    py: Python<'_>,
    mode_name: &str,
    size: &Bound<'_, PyAny>,
    schema_capsule: &Bound<'_, PyAny>,
    array_capsule: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let signed_size = arrow_size_from_python(size)?;
    let schema = schema_pointer(schema_capsule, py)?;
    let array = array_pointer(array_capsule, py)?;
    if signed_size.0 < 0 || signed_size.1 < 0 {
        return Err(PyValueError::new_err("bad image size"));
    }
    let mode = pillow_rs::Mode::from_pillow_name(mode_name)
        .ok_or_else(|| PyValueError::new_err("unrecognized image mode"))?;
    let image_size = pillow_rs::validate_arrow_import_size(mode, signed_size).map_err(map_error)?;
    let pixel_count = checked_pixel_count(image_size)?;
    let layout = layout_from_schema(mode, schema)?;
    let bytes = arrow_buffer_values(mode, layout, array, pixel_count)?;
    let image =
        pillow_rs::Image::from_arrow_buffer(mode, signed_size, layout, bytes).map_err(map_error)?;
    Ok(PyImage { inner: image })
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn arrow_size_from_python(size: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    let sequence = size.cast::<PySequence>().map_err(|_| {
        let name = size
            .get_type()
            .name()
            .map_or_else(|_| "object".to_owned(), |name| name.to_string());
        PyTypeError::new_err(format!("argument 2 must be a 2-item sequence, not {name}"))
    })?;
    if size.is_instance_of::<PyBytes>() {
        return Err(PyTypeError::new_err(
            "argument 2 must be a 2-item sequence, not bytes",
        ));
    }
    let length = sequence.len()?;
    if length != 2 {
        return Err(PyTypeError::new_err(format!(
            "argument 2 must be a sequence of length 2, not {length}"
        )));
    }
    let width = c_int_from_python(&sequence.get_item(0)?)?;
    let height = c_int_from_python(&sequence.get_item(1)?)?;
    Ok((width, height))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn schema_pointer<'a>(capsule: &'a Bound<'_, PyAny>, py: Python<'_>) -> PyResult<&'a ArrowSchema> {
    let pointer = unsafe {
        // SAFETY: CPython checks the object and exact capsule name. Arrow's
        // producer contract requires the named pointer to address a live
        // ArrowSchema; `capsule` keeps its owner alive during this call.
        ffi::PyCapsule_GetPointer(capsule.as_ptr(), c"arrow_schema".as_ptr())
    };
    let pointer = non_null_capsule_pointer(pointer, py)?;
    // SAFETY: a compliant producer keeps the schema allocation alive while
    // its capsule is live. C-data capsules cannot make arbitrary forged
    // pointers safe; this importer relies on the Arrow producer contract.
    let schema = unsafe { &*pointer.cast::<ArrowSchema>() };
    if schema.release.is_none() {
        return Err(invalid_arrow_array());
    }
    Ok(schema)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn array_pointer<'a>(capsule: &'a Bound<'_, PyAny>, py: Python<'_>) -> PyResult<&'a ArrowArray> {
    let pointer = unsafe {
        // SAFETY: CPython checks the object and exact capsule name. Arrow's
        // producer contract requires the named pointer to address a live
        // ArrowArray; `capsule` keeps its owner alive during this call.
        ffi::PyCapsule_GetPointer(capsule.as_ptr(), c"arrow_array".as_ptr())
    };
    let pointer = non_null_capsule_pointer(pointer, py)?;
    // SAFETY: a compliant producer keeps the array allocation alive while its
    // capsule is live. C-data capsules cannot make arbitrary forged pointers
    // safe; this importer relies on the Arrow producer contract.
    let array = unsafe { &*pointer.cast::<ArrowArray>() };
    if array.release.is_none() {
        return Err(invalid_arrow_array());
    }
    Ok(array)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn non_null_capsule_pointer(pointer: *mut c_void, py: Python<'_>) -> PyResult<*mut c_void> {
    if pointer.is_null() {
        Err(pyo3::PyErr::fetch(py))
    } else {
        Ok(pointer)
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn checked_pixel_count(size: (u32, u32)) -> PyResult<usize> {
    usize::try_from(u64::from(size.0) * u64::from(size.1))
        .map_err(|_| PyValueError::new_err(INVALID_ARROW_ARRAY_ERROR))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn layout_from_schema(mode: pillow_rs::Mode, schema: &ArrowSchema) -> PyResult<ArrowImportLayout> {
    let format = schema_format(schema.format)?;
    let layout = match (mode, format.as_slice()) {
        (
            pillow_rs::Mode::One | pillow_rs::Mode::ImageLuma8 | pillow_rs::Mode::ImagePalette8,
            b"C",
        ) => ArrowImportLayout::ScalarU8,
        (pillow_rs::Mode::ImageI32, b"i") => ArrowImportLayout::ScalarI32,
        (pillow_rs::Mode::ImageF32, b"f") => ArrowImportLayout::ScalarF32,
        (
            pillow_rs::Mode::ImageI16
            | pillow_rs::Mode::ImageI16L
            | pillow_rs::Mode::ImageI16B
            | pillow_rs::Mode::ImageI16N,
            b"s",
        ) => ArrowImportLayout::ScalarI16,
        (mode, b"I") if accepts_packed_i32(mode) => ArrowImportLayout::PackedI32Multiband,
        (mode, b"+w:4") if is_multiband_byte_mode(mode) => {
            validate_fixed_list_schema(schema)?;
            ArrowImportLayout::FixedSizeByteList4
        }
        (mode, b"C") if is_multiband_byte_mode(mode) => {
            if schema.n_children != 0 {
                return Err(invalid_arrow_array());
            }
            ArrowImportLayout::InterleavedByte4
        }
        _ => return Err(invalid_arrow_array()),
    };
    Ok(layout)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn is_multiband_byte_mode(mode: pillow_rs::Mode) -> bool {
    mode.band_count() > 1 && mode.pillow_pixel_size() == Some(4)
}

// Pillow accepts an int32 array as a physical four-byte lane carrier for any
// multiband mode with four-byte pixels.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn accepts_packed_i32(mode: pillow_rs::Mode) -> bool {
    is_multiband_byte_mode(mode)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn schema_format(format: *const c_char) -> PyResult<Vec<u8>> {
    if format.is_null() {
        return Err(invalid_arrow_array());
    }
    // SAFETY: ArrowSchema.format is a producer-owned NUL-terminated C string
    // and the schema capsule remains alive for the duration of the import.
    let format = unsafe { CStr::from_ptr(format) };
    Ok(format.to_bytes().to_vec())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn validate_fixed_list_schema(schema: &ArrowSchema) -> PyResult<()> {
    if schema.n_children < 1 || schema.children.is_null() {
        return Err(invalid_arrow_array());
    }
    // SAFETY: a compliant producer allocates the declared child pointer array
    // and keeps it alive with the parent schema capsule.
    let child = unsafe { *schema.children };
    if child.is_null() {
        return Err(invalid_arrow_array());
    }
    // SAFETY: a compliant producer keeps each declared child schema alive
    // while the parent capsule is live. The callback check rejects an already
    // released child before any of its other fields are read.
    let child_schema = unsafe { &*child };
    if child_schema.release.is_none() {
        return Err(invalid_arrow_array());
    }
    let child_format = child_schema.format;
    if schema_format(child_format)?.as_slice() != b"C" {
        return Err(invalid_arrow_array());
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn arrow_buffer_values<'a>(
    mode: pillow_rs::Mode,
    layout: ArrowImportLayout,
    array: &'a ArrowArray,
    pixel_count: usize,
) -> PyResult<&'a [u8]> {
    let (expected_length, element_width, physical_width) = match layout {
        ArrowImportLayout::ScalarU8 => (pixel_count, 1, 1),
        ArrowImportLayout::ScalarI32 | ArrowImportLayout::ScalarF32 => (pixel_count, 4, 4),
        ArrowImportLayout::ScalarI16 => (pixel_count, 2, 2),
        ArrowImportLayout::FixedSizeByteList4 => (pixel_count, 1, 4),
        ArrowImportLayout::InterleavedByte4 => (
            pixel_count.checked_mul(4).ok_or_else(invalid_arrow_array)?,
            1,
            4,
        ),
        ArrowImportLayout::PackedI32Multiband => (pixel_count, 4, 4),
    };
    validate_array_shape(layout, array, expected_length, pixel_count)?;
    let data_array = if layout == ArrowImportLayout::FixedSizeByteList4 {
        array_child(array)?
    } else {
        array
    };
    let byte_length = pixel_count
        .checked_mul(physical_width)
        .ok_or_else(invalid_arrow_array)?;
    if mode.pillow_pixel_size() != Some(physical_width) {
        return Err(invalid_arrow_array());
    }
    arrow_values(data_array, element_width, byte_length)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn validate_array_shape(
    layout: ArrowImportLayout,
    array: &ArrowArray,
    expected_length: usize,
    pixel_count: usize,
) -> PyResult<()> {
    if usize::try_from(array.length).ok() != Some(expected_length) {
        return Err(invalid_arrow_array());
    }
    if layout == ArrowImportLayout::FixedSizeByteList4 {
        if array.n_children != 1 {
            return Err(invalid_arrow_array());
        }
        let child = array_child(array)?;
        let expected_child_length = pixel_count.checked_mul(4).ok_or_else(invalid_arrow_array)?;
        if usize::try_from(child.length).ok() != Some(expected_child_length) {
            return Err(invalid_arrow_array());
        }
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn array_child(array: &ArrowArray) -> PyResult<&ArrowArray> {
    if array.children.is_null() || array.n_children < 1 {
        return Err(invalid_arrow_array());
    }
    // SAFETY: a compliant producer allocates the declared child pointer array
    // and keeps it alive with the parent array capsule.
    let child = unsafe { *array.children };
    if child.is_null() {
        return Err(invalid_arrow_array());
    }
    // SAFETY: a compliant producer keeps each declared child array alive while
    // the parent capsule is live. Reject released children before reading
    // their shape or buffer fields.
    let child = unsafe { &*child };
    if child.release.is_none() {
        return Err(invalid_arrow_array());
    }
    Ok(child)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn arrow_values(array: &ArrowArray, element_width: usize, byte_length: usize) -> PyResult<&[u8]> {
    if array.n_buffers != 2 || array.buffers.is_null() {
        return Err(arrow_array_buffer_error());
    }
    // SAFETY: a compliant producer provides two pointers when n_buffers is
    // two; the live array capsule retains this pointer array for the call.
    let data = unsafe { *array.buffers.add(1) };
    if data.is_null() || array.offset < 0 || byte_length > isize::MAX as usize {
        return Err(arrow_array_buffer_error());
    }
    let offset = usize::try_from(array.offset)
        .ok()
        .and_then(|offset| offset.checked_mul(element_width))
        .filter(|offset| *offset <= isize::MAX as usize)
        .ok_or_else(invalid_arrow_array)?;
    // SAFETY: the Arrow C Data Interface requires the producer's values buffer
    // to cover `length + offset` elements of its declared format. `byte_length`
    // comes from validated dimensions, and the capsule keeps the allocation
    // alive through this borrowed slice. A producer that violates this memory
    // contract can still cause undefined behavior.
    let values = unsafe { slice::from_raw_parts(data.cast::<u8>().add(offset), byte_length) };
    Ok(values)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn arrow_array_buffer_error() -> PyErr {
    PyValueError::new_err(ARROW_ARRAY_BUFFER_ERROR)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn invalid_arrow_array() -> PyErr {
    PyValueError::new_err(INVALID_ARROW_ARRAY_ERROR)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(image_from_arrow, module)?)?;
    Ok(())
}
