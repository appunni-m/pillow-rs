//! Default Transform implementation for exact mode ImageHsv8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageHsv8,
    crate::raster::RgbImage,
    3,
    false,
    false,
    Rgb,
    "transform ImageHsv8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageHsv8, 3, false, "ImageHsv8");

