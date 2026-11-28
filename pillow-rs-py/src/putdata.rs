//! Python protocol adapter for `Image.putdata`.
//!
//! The image crate owns value normalization, mode validation, and storage.
//! This module owns only the host-language work that cannot cross that
//! boundary: Python sequence protocol checks, exact built-in bulk extraction,
//! and the callback-visible order of per-item coercion.

use super::{PyImage, map_error};
use bytes::Bytes;
use pillow_rs::{Mode, PutDataExecutionContext, PutDataInput, PutDataValue, PutDataValueKind};
use pyo3::PyTypeInfo;
use pyo3::exceptions::{PySystemError, PyTypeError};
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{
    PyAnyMethods, PyBytes, PyBytesMethods, PyDict, PyFloat, PyInt, PyList, PyListMethods, PyTuple,
    PyTupleMethods, PyType, PyTypeMethods,
};

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn python_is_sequence(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    // Exact numbers are common pixel values and cannot provide the sequence
    // protocol. Keep this fast path ahead of the type inspection below.
    if value.cast_exact::<PyInt>().is_ok() || value.cast_exact::<PyFloat>().is_ok() {
        return Ok(false);
    }

    let ty = value.get_type();
    // CPython's sequence check excludes dict subclasses even when they define
    // `__getitem__`.
    if ty.is_subclass_of::<PyDict>()? {
        return Ok(false);
    }

    // CPython treats Python classes with `__getitem__` as sequences. Inspect
    // the type's MRO dictionaries through the built-in `type.__getattribute__`
    // so a custom metaclass cannot run callbacks during this preflight. This
    // preserves support for duck-typed sequences without invoking `__len__`,
    // `__iter__`, or `__getitem__` before the existing validation order.
    let type_getattribute = PyType::type_object(value.py()).getattr("__getattribute__")?;
    let mro = type_getattribute
        .call1((ty, "__mro__"))?
        .cast_into::<PyTuple>()?;

    for base in mro.iter() {
        let namespace = type_getattribute.call1((base, "__dict__"))?;
        if namespace.contains("__getitem__")? {
            return Ok(true);
        }
    }
    Ok(false)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_value_from_python(
    value: &Bound<'_, PyAny>,
    kind: PutDataValueKind,
) -> PyResult<PutDataValue> {
    if matches!(kind, PutDataValueKind::Numeric) {
        if python_is_sequence(value)? {
            // Preserve the shape distinction for the core's canonical
            // "sequence must be flattened" error instead of terminating in
            // the binding before putdata_bytes sees the value.
            return Ok(PutDataValue::Components(Vec::new()));
        }
        // Pillow's numeric `_putdata` propagates PyFloat_AsDouble failures.
        return Ok(PutDataValue::Number(value.extract::<f64>()?));
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

    let Ok(tuple) = value.cast::<PyTuple>() else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let tuple_len = tuple.len();
    if matches!(kind, PutDataValueKind::FloatComponents { .. }) {
        if !kind.accepts_component_arity(tuple_len) {
            return Ok(PutDataValue::FloatComponents(Vec::new()));
        }
        let components = tuple
            .iter()
            .map(|item| item.extract::<f64>())
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(PutDataValue::FloatComponents(components));
    }
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

    // TODO(pillow-rs#3ptb): verify malformed LA/RGB tuple shape wins over
    // component conversion errors in the frozen CPU parity cases.
    if !kind.accepts_component_arity(tuple_len) {
        // Pillow checks the tuple shape before converting its components.
        // Carry an invalid shape to core without triggering an earlier
        // Python integer overflow or type error; core emits the canonical
        // mode-specific arity error.
        return Ok(PutDataValue::Components(Vec::new()));
    }

    if matches!(kind, PutDataValueKind::NativeU16Components { .. }) {
        let components = tuple
            .iter()
            .map(|item| item.extract::<i64>().map(i128::from))
            .collect::<PyResult<Vec<_>>>()?;
        return Ok(PutDataValue::Components(components));
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_bulk(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    byte_image_mode: bool,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
    route: Option<(Option<pillow_rs::Backend>, pillow_rs::ExecutionContext)>,
) -> PyResult<bool> {
    if putdata_raw_bytes(
        slf,
        data,
        entry_count,
        byte_image_mode,
        scale,
        offset,
        route,
    )? {
        return Ok(true);
    }

    // Exact built-in numeric elements have no user callbacks to observe
    // between writes. Container exactness alone is insufficient: a list can
    // contain a custom scalar with __index__/__float__, and Pillow processes
    // that item through the per-pixel path (including partial writes before a
    // later error). Let Rust normalize only the callback-free subset in one
    // operation.
    if is_exact_builtin_numeric_sequence(data, kind) {
        if let Ok(values) = data.extract::<Vec<i64>>() {
            slf.try_borrow_mut()?
                .inner
                .putdata_input_with_execution_context(
                    PutDataInput::Integers(&values),
                    scale,
                    offset,
                    route,
                )
                .map_err(map_error)?;
            return Ok(true);
        }
        // Numeric byte, I, and F modes use Pillow's `PyFloat_AsDouble`
        // conversion before applying scale/offset. If exact integer
        // extraction fails, a complete f64 extraction retains that
        // callback-free bulk route. If f64 conversion also fails, fall
        // through so per-item writes preserve the successful prefix and the
        // failing value's original exception.
        if matches!(kind, PutDataValueKind::Numeric)
            && let Ok(values) = data.extract::<Vec<f64>>()
        {
            slf.try_borrow_mut()?
                .inner
                .putdata_input_with_execution_context(
                    PutDataInput::Numbers(&values),
                    scale,
                    offset,
                    route,
                )
                .map_err(map_error)?;
            return Ok(true);
        }
    }

    // CoreOnly multiband modes can write exact built-in component tuples
    // without Python callbacks. Extract the complete valid sequence before
    // entering core; malformed or custom values retain the ordered per-item
    // path below, including any prefix writes before an error.
    if let Some(values) = putdata_native_component_tuples(data, kind) {
        slf.try_borrow_mut()?
            .inner
            .putdata_values_with_execution_context(&values, scale, offset, route)
            .map_err(map_error)?;
        return Ok(true);
    }

    Ok(false)
}

// Pillow uses raw byte storage for its image8 modes (`1`, `L`, `P`, and `I;16*`).
// Exact bytes may retain their allocation; subclass bytes are consumed without
// retaining Python-owned storage.
// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_raw_bytes(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    byte_image_mode: bool,
    scale: f64,
    offset: f64,
    route: Option<(Option<pillow_rs::Backend>, pillow_rs::ExecutionContext)>,
) -> PyResult<bool> {
    if !byte_image_mode {
        return Ok(false);
    }
    let Ok(bytes) = data.cast::<PyBytes>() else {
        return Ok(false);
    };
    if data.cast_exact::<PyBytes>().is_ok()
        && let Some(retained) = retained_python_bytes(data)
    {
        let mut image = slf.try_borrow_mut()?;
        if image
            .inner
            .putdata_shared_bytes_fast_path_with_context(
                retained,
                entry_count,
                scale,
                offset,
                route,
            )
            .map_err(map_error)?
        {
            return Ok(true);
        }
    }
    slf.try_borrow_mut()?
        .inner
        .putdata_bytes_fast_path_with_execution_context(
            bytes.as_bytes(),
            entry_count,
            scale,
            offset,
            route,
        )
        .map_err(map_error)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn retained_python_bytes(data: &Bound<'_, PyAny>) -> Option<Bytes> {
    data.extract::<Bytes>().ok()
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn is_exact_builtin_numeric_sequence(data: &Bound<'_, PyAny>, kind: PutDataValueKind) -> bool {
    let is_number = |item: Bound<'_, PyAny>| {
        item.cast_exact::<PyInt>().is_ok() || item.cast_exact::<pyo3::types::PyFloat>().is_ok()
    };
    let is_integer = |item: Bound<'_, PyAny>| item.cast_exact::<PyInt>().is_ok();
    if let Ok(list) = data.cast_exact::<PyList>() {
        return match kind {
            // Multiband `putdata` treats an integer as a packed pixel but
            // rejects a scalar float.  A mixed exact list therefore cannot
            // be normalized as Vec<f64>: Pillow writes earlier packed ints
            // before rejecting the later float.
            PutDataValueKind::Components { .. } => list.iter().all(is_integer),
            PutDataValueKind::NativeU16Components { .. } => list.iter().all(is_integer),
            PutDataValueKind::FloatComponents { .. } => false,
            PutDataValueKind::Numeric => list.iter().all(is_number),
        };
    }
    if let Ok(tuple) = data.cast_exact::<PyTuple>() {
        return match kind {
            PutDataValueKind::Components { .. } => tuple.iter().all(is_integer),
            PutDataValueKind::NativeU16Components { .. } => tuple.iter().all(is_integer),
            PutDataValueKind::FloatComponents { .. } => false,
            PutDataValueKind::Numeric => tuple.iter().all(is_number),
        };
    }
    false
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_native_component_tuples(
    data: &Bound<'_, PyAny>,
    kind: PutDataValueKind,
) -> Option<Vec<PutDataValue>> {
    if !matches!(
        kind,
        PutDataValueKind::NativeU16Components { .. } | PutDataValueKind::FloatComponents { .. }
    ) {
        return None;
    }
    let items = if let Ok(list) = data.cast_exact::<PyList>() {
        list.iter().collect::<Vec<_>>()
    } else if let Ok(tuple) = data.cast_exact::<PyTuple>() {
        tuple.iter().collect::<Vec<_>>()
    } else {
        return None;
    };

    let mut values = Vec::with_capacity(items.len());
    for item in items {
        let tuple = item.cast_exact::<PyTuple>().ok()?;
        let packed_native_tuple = matches!(kind, PutDataValueKind::NativeU16Components { .. })
            && tuple.len() == 1
            && tuple.get_item(0).ok()?.cast_exact::<PyInt>().is_ok();
        if !packed_native_tuple && !kind.accepts_component_arity(tuple.len()) {
            return None;
        }
        let exact_components = tuple.iter().all(|component| match kind {
            PutDataValueKind::NativeU16Components { .. } => component.cast_exact::<PyInt>().is_ok(),
            PutDataValueKind::FloatComponents { .. } => {
                component.cast_exact::<PyInt>().is_ok() || component.cast_exact::<PyFloat>().is_ok()
            }
            PutDataValueKind::Numeric | PutDataValueKind::Components { .. } => false,
        });
        if !exact_components {
            return None;
        }
        values.push(putdata_value_from_python(&item, kind).ok()?);
    }
    Some(values)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn write_item(
    slf: &Bound<'_, PyImage>,
    pixel_index: usize,
    item: Bound<'_, PyAny>,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
    context: &mut PutDataExecutionContext,
) -> PyResult<()> {
    // No PyImage borrow may span coercion: __index__ and __float__ can
    // re-enter this same public image and must observe earlier writes.
    let value = putdata_value_from_python(&item, kind)?;
    slf.try_borrow_mut()?
        .inner
        .putdata_value_at_with_context(pixel_index, &value, scale, offset, context)
        .map_err(map_error)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_exact_sequence(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    entry_count: usize,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
    context: &mut PutDataExecutionContext,
) -> PyResult<bool> {
    // CPython's PySequence_Fast retains exact lists and tuples instead of
    // copying them. Read each exact-list item only when its pixel is due so
    // coercing an earlier item can replace a later one, as Pillow exposes.
    if let Ok(list) = data.cast_exact::<PyList>() {
        for pixel_index in 0..entry_count {
            write_item(
                slf,
                pixel_index,
                list.get_item(pixel_index)?,
                kind,
                scale,
                offset,
                context,
            )?;
        }
        return Ok(true);
    }
    if let Ok(tuple) = data.cast_exact::<PyTuple>() {
        for pixel_index in 0..entry_count {
            write_item(
                slf,
                pixel_index,
                tuple.get_item(pixel_index)?,
                kind,
                scale,
                offset,
                context,
            )?;
        }
        return Ok(true);
    }
    Ok(false)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn putdata_generic_sequence(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    kind: PutDataValueKind,
    scale: f64,
    offset: f64,
    context: &mut PutDataExecutionContext,
) -> PyResult<()> {
    // Pillow materializes generic sequences before coercing or writing pixels.
    // PySequence_Fast replaces only TypeError from iterator acquisition with
    // its canonical message and preserves other exception classes.
    // TODO(pillow-rs#3ptb): verify reported versus materialized sequence
    // lengths, __length_hint__ callbacks, materialization faults, and prefix
    // writes after materialization.
    let iterator = data.try_iter().map_err(|error| {
        if error.is_instance_of::<PyTypeError>(data.py()) {
            PyTypeError::new_err("argument must be a sequence")
        } else {
            error
        }
    })?;
    // PySequence_List materializes the iterator into a list, including its
    // length-hint protocol. Use the exact built-in list's extend path here so
    // those host callbacks finish before the first pixel conversion.
    let items = PyList::empty(data.py());
    items.call_method1("extend", (iterator,))?;
    slf.try_borrow()?
        .inner
        .validate_putdata_length(items.len())
        .map_err(map_error)?;

    for (pixel_index, item) in items.iter().enumerate() {
        write_item(slf, pixel_index, item, kind, scale, offset, context)?;
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn putdata_formatted(
    slf: &Bound<'_, PyImage>,
    data: &Bound<'_, PyAny>,
    scale: f64,
    offset: f64,
    route: Option<(Option<pillow_rs::Backend>, pillow_rs::ExecutionContext)>,
) -> PyResult<()> {
    if !python_is_sequence(data)? {
        return Err(PyTypeError::new_err("argument must be a sequence"));
    }
    let image_mode = {
        let image = slf.try_borrow()?;
        image.inner.mode_identity().map_err(map_error)?
    };
    let byte_image_mode = matches!(
        image_mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImagePalette8
            | Mode::ImageI16
            | Mode::ImageI16L
            | Mode::ImageI16B
            | Mode::ImageI16N
    );
    let bytes = data.cast::<PyBytes>().ok();
    // Pillow uses PyBytes_GET_SIZE for its image8 byte path; other modes use
    // PySequence_Size and then materialize the iterator.
    let entry_count = match (&bytes, byte_image_mode) {
        (Some(bytes), true) => bytes.as_bytes().len(),
        _ => data.len()?,
    };

    // The binding snapshots the mode's value shape before coercion. Core
    // resolves the live mode on each write, so callback-driven mode changes
    // need an explicit Pillow contract before this snapshot can be trusted.
    // TODO(pillow-rs#3ptb): cover a custom __index__/__float__ callback that
    // changes image.mode between PutData entries.
    let kind = {
        let image = slf.try_borrow()?;
        image
            .inner
            .validate_putdata_length(entry_count)
            .map_err(map_error)?;
        image.inner.putdata_value_kind().map_err(map_error)?
    };

    if putdata_bulk(
        slf,
        data,
        entry_count,
        byte_image_mode,
        kind,
        scale,
        offset,
        route,
    )? {
        return Ok(());
    }
    let mut context = {
        let image = slf.try_borrow()?;
        match route {
            Some((requested_backend, context)) => image
                .inner
                .putdata_execution_context_for_route(requested_backend, context),
            None => image.inner.putdata_execution_context(),
        }
    };
    if putdata_exact_sequence(slf, data, entry_count, kind, scale, offset, &mut context)? {
        return Ok(());
    }
    putdata_generic_sequence(slf, data, kind, scale, offset, &mut context)
}
