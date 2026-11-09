//! Default Transform implementation for exact mode ImageCmyk8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    4,
    false,
    false,
    Generic,
    "transform ImageCmyk8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImageCmyk8, 4, false, "ImageCmyk8");
