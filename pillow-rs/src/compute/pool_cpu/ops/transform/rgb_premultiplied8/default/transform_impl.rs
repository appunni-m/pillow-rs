//! Default Transform implementation for exact mode ImageRgbPremultiplied8.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    ImageRgbPremultiplied8,
    crate::raster::RgbaImage,
    4,
    false,
    false,
    Generic,
    "transform ImageRgbPremultiplied8 output buffer shape mismatch"
);

