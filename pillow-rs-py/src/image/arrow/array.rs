//! Arrow array descriptors backed by retained Rust image allocations.

use std::ffi::c_void;

use pyo3::exceptions::PyOverflowError;
use pyo3::prelude::PyResult;

use pillow_rs::ArrowExportBuffer;

use super::abi::{ArrayOwner, ArrowArray, release_array};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn array_node(
    image: ArrowExportBuffer,
    length: i64,
    buffers: Vec<*const c_void>,
    children: Vec<Box<ArrowArray>>,
) -> Box<ArrowArray> {
    let mut owner = Box::new(ArrayOwner {
        _image: image,
        buffers,
        children,
        child_ptrs: Vec::new(),
    });
    owner.child_ptrs = owner
        .children
        .iter_mut()
        .map(|child| child.as_mut() as *mut ArrowArray)
        .collect();
    let buffer_ptr = owner.buffers.as_ptr();
    let children_ptr = if owner.child_ptrs.is_empty() {
        std::ptr::null_mut()
    } else {
        owner.child_ptrs.as_mut_ptr()
    };
    Box::new(ArrowArray {
        length,
        null_count: 0,
        offset: 0,
        n_buffers: owner.buffers.len() as i64,
        n_children: owner.children.len() as i64,
        buffers: buffer_ptr,
        children: children_ptr,
        dictionary: std::ptr::null_mut(),
        release: Some(release_array),
        private_data: Box::into_raw(owner).cast(),
    })
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn data_pointer(buffer: &ArrowExportBuffer) -> *const c_void {
    if buffer.as_bytes().is_empty() {
        std::ptr::null()
    } else {
        buffer.as_bytes().as_ptr().cast()
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) fn array_for_buffer(buffer: &ArrowExportBuffer) -> PyResult<Box<ArrowArray>> {
    let length = i64::try_from(buffer.len())
        .map_err(|_| PyOverflowError::new_err("Arrow array length is too large"))?;
    let data = data_pointer(buffer);
    if buffer.is_multiband() {
        let child_length = length
            .checked_mul(4)
            .ok_or_else(|| PyOverflowError::new_err("Arrow image data is too large"))?;
        let child = array_node(
            buffer.clone(),
            child_length,
            vec![std::ptr::null(), data],
            Vec::new(),
        );
        Ok(array_node(
            buffer.clone(),
            length,
            vec![std::ptr::null()],
            vec![child],
        ))
    } else {
        Ok(array_node(
            buffer.clone(),
            length,
            vec![std::ptr::null(), data],
            Vec::new(),
        ))
    }
}
