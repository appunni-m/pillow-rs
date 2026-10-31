//! Default Transform implementation for exact mode ImagePalette8.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    ImagePalette8,
    crate::raster::GrayImage,
    1,
    true,
    false,
    Generic,
    "transform ImagePalette8 output buffer shape mismatch"
);

