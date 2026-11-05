//! Default Transform implementation for exact mode ImageLab8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageLab8,
    crate::raster::RgbImage,
    3,
    false,
    false,
    Generic,
    "transform ImageLab8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageLab8, 3, false, "ImageLab8");

