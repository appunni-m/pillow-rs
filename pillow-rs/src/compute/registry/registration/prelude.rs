//! Shared imports for registry initialization modules.
//!
//! TODO(pillow-rs#3ptb): retire this direct CPU facade import map as the
//! remaining registrations move to operation-owned prepared callbacks.
//! Convert, ConvertLab, and RemapPalette use prepared typed color owners.

pub(super) use super::Registry;
pub(super) use crate::compute::pool_cpu::ops::draw::{
    op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse, op_draw_line, op_draw_pieslice,
    op_draw_point, op_draw_polygon, op_draw_rectangle, op_draw_rounded_rect,
};
pub(super) use crate::compute::pool_cpu::ops::effects::{
    op_blend_module, op_composite_module, op_merge, op_paste, op_transform,
};
pub(super) use crate::error::PilError;
pub(super) use crate::pipeline::PipelineOp;
pub(super) use crate::raster::DynamicImage;
