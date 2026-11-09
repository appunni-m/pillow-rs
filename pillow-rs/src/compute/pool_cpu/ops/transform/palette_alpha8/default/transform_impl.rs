//! Default Transform implementation for exact mode ImagePaletteAlpha8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte_mode_default, byte_mode_mesh_record};

byte_mode_default!(
    ImagePaletteAlpha8,
    crate::raster::GrayAlphaImage,
    2,
    false,
    false,
    Generic,
    "transform ImagePaletteAlpha8 output buffer shape mismatch"
);

byte_mode_mesh_record!(ImagePaletteAlpha8, 2, false, "ImagePaletteAlpha8");
