//! Python protocol adapter for `Image.putdata`.
//!
//! The image crate owns value normalization, mode validation, and storage.
//! This module owns only the host-language work that cannot cross that
//! boundary: Python sequence protocol checks, exact built-in bulk extraction,
//! and the callback-visible order of per-item coercion.

use super::{PyImage, map_error};
use pillow_rs::{PutDataInput, PutDataValue, PutDataValueKind};
use pyo3::exceptions::{PySystemError, PyTypeError};
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{
    PyAnyMethods, PyBytes, PyBytesMethods, PyInt, PyList, PyListMethods, PyTuple, PyTupleMethods,
};

#[allow(unsafe_code)]
fn python_is_sequence(value: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `Bound` guarantees a non-null, GIL-bound borrowed pointer for
    // this call. `PySequence_Check` only inspects the object's type slots and
    // neither steals a reference nor stores the pointer.
    unsafe { pyo3::ffi::PySequence_Check(value.as_ptr()) != 0 }
}

fn putdata_value_from_python(
    value: &Bound<'_, PyAny>,
    kind: PutDataValueKind,
) -> PyResult<PutDataValue> {
    if matches!(kind, PutDataValueKind::Numeric) {
        if python_is_sequence(value) {
            // Preserve the shape distinction for the core's canonical
            // "sequence must be flattened" error instead of terminating in
            // the binding before putdata_bytes sees the value.
            return Ok(PutDataValue::Components(Vec::new()));
        }
        // Pillow's numeric `_putdata` path deliberately clears conversion
        // errors after writing the sentinel returned by PyFloat_AsDouble.
        return Ok(PutDataValue::Number(value.extract::<f64>().unwrap_or(-1.0)));
    }

    if value.is_instance_of::<PyInt>() {
        return value.extract::<i64>().map(PutDataValue::Packed);
    }

    // Multiband Pillow putdata rejects scalar floats through the same shape
    // validation as other non-tuple values. Preserve the numeric distinction
    // for the core so it owns the public error contract instead of the
    // binding short-circuiting that path.
    if value.is_instance_of::<pyo3::types::PyFloat>() {
        return value.extract::<f64>().map(PutDataValue::Number);
    }

    let Ok(tuple) = value.downcast::<PyTuple>() else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let tuple_len = tuple.len();
    if tuple_len == 1 {
        let packed = tuple.get_item(0)?;
        if packed.is_instance_of::<PyInt>() {
            return packed.extract::<i64>().map(PutDataValue::Packed);
        }
        if matches!(kind, PutDataValueKind::Components { channels: 2 }) {
            return Err(PySystemError::new_err(
                "new style getargs format but argument is not a tuple",
            ));
        }
        return Err(PyTypeError::new_err(
            "color must be int, or tuple of one, three or four elements",
        ));
    }

    // Keep tuple extraction in the binding, but let the core own arity
    // validation. That preserves one canonical error path for every backend
    // and lets public parity inputs exercise the same shape checks as the
    // Rust API.
    let mut components = Vec::with_capacity(tuple_len);
    for (index, item) in tuple.iter().enumerate() {
        let component = if index == 0 {
            item.extract::<i64>()? as i128
        } else {
            item.extract::<i32>()? as i128
        };
        components.push(component);
    }
    Ok(PutDataValue::Components(components))
}

fn putdata_bulk(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    scale: f64,
    offset: f64,
) -> PyResult<bool> {
    // Pillow's I;16 bytes fast path copies the supplied bytes into the raw
    // two-byte sample buffer. It does not coerce each byte into a separate
    // numeric sample as the generic sequence path does.
    if let Ok(bytes) = data.downcast::<PyBytes>() {
        let consumed = slf
            .try_borrow_mut()?
            .inner
            .putdata_bytes_fast_path(bytes.as_bytes(), entry_count, scale, offset)
            .map_err(map_error)?;
        if consumed {
            return Ok(true);
        }
    }

    // Exact built-in numeric elements have no user callbacks to observe
    // between writes. Container exactness alone is insufficient: a list can
    // contain a custom scalar with __index__/__float__, and Pillow processes
    // that item through the per-pixel path (including partial writes before a
    // later error). Let Rust normalize only the callback-free subset in one
    // operation.
    if is_exact_builtin_numeric_sequence(data) {
        if let Ok(values) = data.extract::<Vec<i64>>() {
            slf.try_borrow_mut()?
                .inner
                .putdata_input(PutDataInput::Integers(&values), scale, offset)
                .map_err(map_error)?;
            return Ok(true);
        }
        if let Ok(values) = data.extract::<Vec<f64>>() {
            slf.try_borrow_mut()?
                .inner
                .putdata_input(PutDataInput::Numbers(&values), scale, offset)
                .map_err(map_error)?;
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_exact_builtin_numeric_sequence(data: &Bound<'_, PyAny>) -> bool {
    let is_number = |item: Bound<'_, PyAny>| {
        item.downcast_exact::<PyInt>().is_ok()
            || item.downcast_exact::<pyo3::types::PyFloat>().is_ok()
    };
    if let Ok(list) = data.downcast_exact::<PyList>() {
        return list.iter().all(is_number);
    }
    if let Ok(tuple) = data.downcast_exact::<PyTuple>() {
        return tuple.iter().all(is_number);
    }
    false
}

fn write_item(
    slf: &Bound<'_, PyImage>,
    pixel_index: usize,
    item: Bound<'_, PyAny>,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
) -> PyResult<()> {
    // No PyImage borrow may span coercion: __index__ and __float__ can
    // re-enter this same public image and must observe earlier writes.
    let value = putdata_value_from_python(&item, kind)?;
    slf.try_borrow_mut()?
        .inner
        .putdata_value_at(pixel_index, &value, scale, offset)
        .map_err(map_error)
}

fn putdata_exact_sequence(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
) -> PyResult<bool> {
    // CPython's PySequence_Fast retains exact lists and tuples instead of
    // copying them. Read each exact-list item only when its pixel is due so
    // coercing an earlier item can replace a later one, as Pillow exposes.
    if let Ok(list) = data.downcast_exact::<PyList>() {
        for pixel_index in 0..entry_count {
            write_item(
                slf,
                pixel_index,
                list.get_item(pixel_index)?,
                kind,
                scale,
                offset,
            )?;
        }
        return Ok(true);
    }
    if let Ok(tuple) = data.downcast_exact::<PyTuple>() {
        for pixel_index in 0..entry_count {
            write_item(
                slf,
                pixel_index,
                tuple.get_item(pixel_index)?,
                kind,
                scale,
                offset,
            )?;
        }
        return Ok(true);
    }
    Ok(false)
}

fn putdata_generic_sequence(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
) -> PyResult<()> {
    // Pillow calls PySequence_Fast before coercing any pixel. Materialize
    // generic sequence items first, map iteration failures to its fixed
    // error, then process only the count reported by the original sequence.
    let iterator = data
        .iter()
        .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;
    let mut items = Vec::new();
    for item in iterator {
        items.push(item.map_err(|_| PyTypeError::new_err("argument must be a sequence"))?);
    }

    for (pixel_index, item) in items.into_iter().take(entry_count).enumerate() {
        write_item(slf, pixel_index, item, kind, scale, offset)?;
    }
    Ok(())
}

pub(crate) fn putdata_formatted(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    scale: f64,
    offset: f64,
) -> PyResult<()> {
    if !python_is_sequence(data) {
        return Err(PyTypeError::new_err("argument must be a sequence"));
    }
    let entry_count = data
        .len()
        .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;

    let kind = {
        let image = slf.try_borrow()?;
        image
            .inner
            .validate_putdata_length(entry_count)
            .map_err(map_error)?;
        image.inner.putdata_value_kind().map_err(map_error)?
    };

    if putdata_bulk(slf, data, entry_count, scale, offset)? {
        return Ok(());
    }
    if putdata_exact_sequence(slf, data, entry_count, kind, scale, offset)? {
        return Ok(());
    }
    putdata_generic_sequence(slf, data, entry_count, kind, scale, offset)
}
