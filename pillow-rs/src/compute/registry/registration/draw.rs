//! Draw operation registration.

use super::{prelude::Registry, prelude::PilError};

pub(super) fn register(entries: &mut Registry) -> Result<(), PilError> {
    let prepare = crate::compute::pool_cpu::ops::draw::ops::prepare_pipeline;
    entries.insert("DrawLine", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawRectangle", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawRoundedRect", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawEllipse", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawCircle", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawPolygon", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawArc", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawChord", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawPieslice", gpu_entry!(prepare, "draw.wgsl"));
    entries.insert("DrawPoint", gpu_entry!(prepare, "draw.wgsl"));
    Ok(())
}
