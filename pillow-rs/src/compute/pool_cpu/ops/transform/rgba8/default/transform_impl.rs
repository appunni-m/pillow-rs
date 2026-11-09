//! Default Transform implementation for exact mode ImageRgba8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageRgba8,
    crate::raster::RgbaImage,
    4,
    false,
    true,
    Generic,
    "transform ImageRgba8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageRgba8, 4, true, "ImageRgba8");
