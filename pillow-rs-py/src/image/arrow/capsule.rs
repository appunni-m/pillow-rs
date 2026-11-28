//! Single-use Arrow PyCapsule construction and teardown.
#![allow(unsafe_code)] // CPython capsule callbacks own Arrow C-data lifetimes.

use std::ffi::c_void;
use std::ptr::NonNull;

use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyCapsule;

use super::abi::{ArrowArray, ArrowSchema};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
unsafe extern "C" fn schema_capsule_destructor(capsule: *mut pyo3::ffi::PyObject) {
    // SAFETY: CPython calls this with the live capsule created below. Arrow
    // consumers may rename a consumed capsule, so use its current valid name.
    let name = unsafe { pyo3::ffi::PyCapsule_GetName(capsule) };
    if name.is_null() {
        // SAFETY: clearing the lookup error is required because destructors may
        // not leave a Python exception set while reference counts are dropped.
        unsafe { pyo3::ffi::PyErr_Clear() };
        return;
    }
    // SAFETY: the capsule name came from this live capsule; its pointer holds
    // the ArrowSchema Box installed by schema_capsule.
    let pointer = unsafe { pyo3::ffi::PyCapsule_GetPointer(capsule, name) };
    if pointer.is_null() {
        // SAFETY: clear a failed capsule lookup during destructor execution.
        unsafe { pyo3::ffi::PyErr_Clear() };
        return;
    }
    let schema = pointer.cast::<ArrowSchema>();
    // SAFETY: this callback is attached only to the ArrowSchema pointer above.
    if let Some(release) = unsafe { (*schema).release } {
        // SAFETY: release is the producer callback stored in the live schema.
        unsafe { release(schema) };
    }
    // SAFETY: schema_capsule transferred this Box allocation to this capsule.
    drop(unsafe { Box::from_raw(schema) });
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
unsafe extern "C" fn array_capsule_destructor(capsule: *mut pyo3::ffi::PyObject) {
    // SAFETY: CPython calls this with the live capsule created below. Arrow
    // consumers may rename a consumed capsule, so use its current valid name.
    let name = unsafe { pyo3::ffi::PyCapsule_GetName(capsule) };
    if name.is_null() {
        // SAFETY: clearing the lookup error is required during decref teardown.
        unsafe { pyo3::ffi::PyErr_Clear() };
        return;
    }
    // SAFETY: the capsule name came from this live capsule; its pointer holds
    // the ArrowArray Box installed by array_capsule.
    let pointer = unsafe { pyo3::ffi::PyCapsule_GetPointer(capsule, name) };
    if pointer.is_null() {
        // SAFETY: clear a failed capsule lookup during destructor execution.
        unsafe { pyo3::ffi::PyErr_Clear() };
        return;
    }
    let array = pointer.cast::<ArrowArray>();
    // SAFETY: this callback is attached only to the ArrowArray pointer above.
    if let Some(release) = unsafe { (*array).release } {
        // SAFETY: release is the producer callback stored in the live array.
        unsafe { release(array) };
    }
    // SAFETY: array_capsule transferred this Box allocation to this capsule.
    drop(unsafe { Box::from_raw(array) });
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) fn schema_capsule<'py>(
    py: Python<'py>,
    schema: Box<ArrowSchema>,
) -> PyResult<Bound<'py, PyCapsule>> {
    let raw = Box::into_raw(schema);
    // SAFETY: Box::into_raw always returns a non-null allocation pointer.
    let pointer = unsafe { NonNull::new_unchecked(raw.cast::<c_void>()) };
    // SAFETY: pointer owns a live ArrowSchema Box; its destructor releases the
    // C-data structure and frees the Box if the capsule is never consumed.
    match unsafe {
        PyCapsule::new_with_pointer_and_destructor(
            py,
            pointer,
            c"arrow_schema",
            Some(schema_capsule_destructor),
        )
    } {
        Ok(capsule) => Ok(capsule),
        Err(error) => {
            // SAFETY: capsule creation failed, so ownership remains with raw.
            if let Some(release) = unsafe { (*raw).release } {
                // SAFETY: release is the producer callback stored in raw.
                unsafe { release(raw) };
            }
            // SAFETY: raw is still the Box allocation from Box::into_raw.
            drop(unsafe { Box::from_raw(raw) });
            Err(error)
        }
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) fn array_capsule<'py>(
    py: Python<'py>,
    array: Box<ArrowArray>,
) -> PyResult<Bound<'py, PyCapsule>> {
    let raw = Box::into_raw(array);
    // SAFETY: Box::into_raw always returns a non-null allocation pointer.
    let pointer = unsafe { NonNull::new_unchecked(raw.cast::<c_void>()) };
    // SAFETY: pointer owns a live ArrowArray Box; its destructor releases the
    // C-data structure and frees the Box if the capsule is never consumed.
    match unsafe {
        PyCapsule::new_with_pointer_and_destructor(
            py,
            pointer,
            c"arrow_array",
            Some(array_capsule_destructor),
        )
    } {
        Ok(capsule) => Ok(capsule),
        Err(error) => {
            // SAFETY: capsule creation failed, so ownership remains with raw.
            if let Some(release) = unsafe { (*raw).release } {
                // SAFETY: release is the producer callback stored in raw.
                unsafe { release(raw) };
            }
            // SAFETY: raw is still the Box allocation from Box::into_raw.
            drop(unsafe { Box::from_raw(raw) });
            Err(error)
        }
    }
}
