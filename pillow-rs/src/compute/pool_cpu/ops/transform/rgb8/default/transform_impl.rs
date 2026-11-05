//! Default Transform implementation for exact mode ImageRgb8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageRgb8,
    crate::raster::RgbImage,
    3,
    false,
    false,
    Rgb,
    "transform ImageRgb8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageRgb8, 3, false, "ImageRgb8");

