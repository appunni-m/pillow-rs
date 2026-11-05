//! Default Transform implementation for exact mode ImageYCbCr8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageYCbCr8,
    crate::raster::RgbImage,
    3,
    false,
    false,
    Rgb,
    "transform ImageYCbCr8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageYCbCr8, 3, false, "ImageYCbCr8");

