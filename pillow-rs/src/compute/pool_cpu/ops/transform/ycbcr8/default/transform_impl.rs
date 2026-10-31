//! Default Transform implementation for exact mode ImageYCbCr8.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    ImageYCbCr8,
    crate::raster::RgbImage,
    3,
    false,
    false,
    Rgb,
    "transform ImageYCbCr8 output buffer shape mismatch"
);

