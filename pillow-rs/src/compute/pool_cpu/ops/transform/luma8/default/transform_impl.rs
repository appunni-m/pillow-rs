//! Default Transform implementation for exact mode ImageLuma8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageLuma8,
    crate::raster::GrayImage,
    1,
    false,
    false,
    Luma,
    "transform ImageLuma8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageLuma8, 1, false, "ImageLuma8");

