//! Default Transform implementation for exact mode ImagePalette8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImagePalette8,
    crate::raster::GrayImage,
    1,
    true,
    false,
    Generic,
    "transform ImagePalette8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImagePalette8, 1, false, "ImagePalette8");

