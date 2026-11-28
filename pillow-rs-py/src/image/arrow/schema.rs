//! Arrow schema creation for Pillow mode and physical-band metadata.

use std::ffi::{CString, c_void};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;

use pillow_rs::ArrowExportBuffer;

use super::abi::{ArrowSchema, SchemaOwner, release_schema};

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn metadata_for_bands(bands: &[&str]) -> Vec<u8> {
    let names = bands
        .iter()
        .map(|band| format!("\"{band}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let json = format!("{{\"bands\": [{names}]}}");
    let mut metadata = Vec::with_capacity(17 + json.len());
    metadata.extend_from_slice(&1_i32.to_ne_bytes());
    metadata.extend_from_slice(&5_i32.to_ne_bytes());
    metadata.extend_from_slice(b"image");
    metadata.extend_from_slice(&(json.len() as i32).to_ne_bytes());
    metadata.extend_from_slice(json.as_bytes());
    metadata
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
fn schema_node(format: &str, name: &str, metadata: Option<Vec<u8>>) -> PyResult<Box<ArrowSchema>> {
    let owner = Box::new(SchemaOwner {
        format: CString::new(format).map_err(|_| PyValueError::new_err("invalid Arrow format"))?,
        name: CString::new(name).map_err(|_| PyValueError::new_err("invalid Arrow field name"))?,
        metadata,
        children: Vec::new(),
        child_ptrs: Vec::new(),
    });
    let schema = Box::new(ArrowSchema {
        format: owner.format.as_ptr(),
        name: owner.name.as_ptr(),
        metadata: owner
            .metadata
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr().cast()),
        flags: 0,
        n_children: 0,
        children: std::ptr::null_mut(),
        dictionary: std::ptr::null_mut(),
        release: Some(release_schema),
        private_data: Box::into_raw(owner).cast::<c_void>(),
    });
    Ok(schema)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
#[allow(unsafe_code)] // Child pointer addresses are owned by this schema node.
fn attach_schema_child(parent: &mut ArrowSchema, child: Box<ArrowSchema>) {
    // SAFETY: schema_node sets private_data to a live Box<SchemaOwner>, which
    // remains owned by the parent's release callback until it is released.
    let owner = unsafe { &mut *parent.private_data.cast::<SchemaOwner>() };
    owner.children.push(child);
    owner.child_ptrs = owner
        .children
        .iter_mut()
        .map(|child| child.as_mut() as *mut ArrowSchema)
        .collect();
    parent.n_children = owner.children.len() as i64;
    parent.children = owner.child_ptrs.as_mut_ptr();
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=n/a
pub(crate) fn schema_for_buffer(buffer: &ArrowExportBuffer) -> PyResult<Box<ArrowSchema>> {
    if buffer.is_multiband() {
        let mut parent = schema_node("+w:4", "", None)?;
        let child_metadata = Some(metadata_for_bands(buffer.bands()));
        let child = schema_node(buffer.format(), buffer.name(), child_metadata)?;
        attach_schema_child(&mut parent, child);
        Ok(parent)
    } else {
        schema_node(
            buffer.format(),
            buffer.bands().first().copied().unwrap_or(buffer.name()),
            Some(metadata_for_bands(buffer.bands())),
        )
    }
}
