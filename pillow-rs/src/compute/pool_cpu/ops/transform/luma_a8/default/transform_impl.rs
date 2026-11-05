//! Default Transform implementation for exact mode ImageLumaA8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    2,
    false,
    true,
    LumaAlpha,
    "transform ImageLumaA8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageLumaA8, 2, true, "ImageLumaA8");

