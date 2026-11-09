//! Default Transform implementation for exact mode ImageRgbx8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageRgbx8,
    crate::raster::RgbaImage,
    4,
    false,
    false,
    Generic,
    "transform ImageRgbx8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageRgbx8, 4, false, "ImageRgbx8");
