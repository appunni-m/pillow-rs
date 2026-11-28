//! Arrow C Data Interface structures and Rust-owned release state.
#![allow(unsafe_code)] // Arrow C callbacks require checked raw-pointer access.

use std::ffi::{CString, c_char, c_void};

use pillow_rs::ArrowExportBuffer;

#[repr(C)]
pub(crate) struct ArrowSchema {
    pub(crate) format: *const c_char,
    pub(crate) name: *const c_char,
    pub(crate) metadata: *const c_char,
    pub(crate) flags: i64,
    pub(crate) n_children: i64,
    pub(crate) children: *mut *mut ArrowSchema,
    pub(crate) dictionary: *mut ArrowSchema,
    pub(crate) release: Option<unsafe extern "C" fn(*mut ArrowSchema)>,
    pub(crate) private_data: *mut c_void,
}

pub(crate) struct SchemaOwner {
    pub(crate) format: CString,
    pub(crate) name: CString,
    pub(crate) metadata: Option<Vec<u8>>,
    pub(crate) children: Vec<Box<ArrowSchema>>,
    pub(crate) child_ptrs: Vec<*mut ArrowSchema>,
}

#[repr(C)]
pub(crate) struct ArrowArray {
    pub(crate) length: i64,
    pub(crate) null_count: i64,
    pub(crate) offset: i64,
    pub(crate) n_buffers: i64,
    pub(crate) n_children: i64,
    pub(crate) buffers: *const *const c_void,
    pub(crate) children: *mut *mut ArrowArray,
    pub(crate) dictionary: *mut ArrowArray,
    pub(crate) release: Option<unsafe extern "C" fn(*mut ArrowArray)>,
    pub(crate) private_data: *mut c_void,
}

impl Drop for ArrowSchema {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            // SAFETY: every Rust-owned ArrowSchema is boxed before creation and
            // stores only this module's release callback. Drop runs before the
            // Box allocation is freed, so the callback receives a live struct.
            unsafe { release(self) };
        }
    }
}

impl Drop for ArrowArray {
    fn drop(&mut self) {
        if let Some(release) = self.release {
            // SAFETY: every Rust-owned ArrowArray is boxed before creation and
            // stores only this module's release callback. Drop runs before the
            // Box allocation is freed, so the callback receives a live struct.
            unsafe { release(self) };
        }
    }
}

pub(crate) struct ArrayOwner {
    pub(crate) _image: ArrowExportBuffer,
    pub(crate) buffers: Vec<*const c_void>,
    pub(crate) children: Vec<Box<ArrowArray>>,
    pub(crate) child_ptrs: Vec<*mut ArrowArray>,
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) unsafe extern "C" fn release_schema(schema: *mut ArrowSchema) {
    if schema.is_null() {
        return;
    }
    // SAFETY: Arrow invokes this callback with the live structure pointer whose
    // non-null release member was installed when the Rust-owned capsule was made.
    let schema = unsafe { &mut *schema };
    if schema.release.is_none() {
        return;
    }
    if !schema.children.is_null() {
        for index in 0..schema.n_children as usize {
            // SAFETY: children points to n_children pointers owned by SchemaOwner.
            let child = unsafe { *schema.children.add(index) };
            if !child.is_null() {
                // SAFETY: each non-null child pointer is a live ArrowSchema
                // allocated by schema_node and owned by this parent schema.
                if let Some(release) = unsafe { (*child).release } {
                    // SAFETY: release is the producer callback stored in child.
                    unsafe { release(child) };
                }
            }
        }
    }
    let owner = schema.private_data.cast::<SchemaOwner>();
    schema.release = None;
    schema.private_data = std::ptr::null_mut();
    if !owner.is_null() {
        // SAFETY: private_data was created by Box::into_raw in schema_node.
        drop(unsafe { Box::from_raw(owner) });
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) unsafe extern "C" fn release_array(array: *mut ArrowArray) {
    if array.is_null() {
        return;
    }
    // SAFETY: Arrow invokes this callback with the live structure pointer whose
    // non-null release member was installed when the Rust-owned capsule was made.
    let array = unsafe { &mut *array };
    if array.release.is_none() {
        return;
    }
    if !array.children.is_null() {
        for index in 0..array.n_children as usize {
            // SAFETY: children points to n_children pointers owned by ArrayOwner.
            let child = unsafe { *array.children.add(index) };
            if !child.is_null() {
                // SAFETY: each non-null child pointer is a live ArrowArray
                // allocated by array_node and owned by this parent array.
                if let Some(release) = unsafe { (*child).release } {
                    // SAFETY: release is the producer callback stored in child.
                    unsafe { release(child) };
                }
            }
        }
    }
    let owner = array.private_data.cast::<ArrayOwner>();
    array.release = None;
    array.private_data = std::ptr::null_mut();
    if !owner.is_null() {
        // SAFETY: private_data was created by Box::into_raw in array_node.
        drop(unsafe { Box::from_raw(owner) });
    }
}
