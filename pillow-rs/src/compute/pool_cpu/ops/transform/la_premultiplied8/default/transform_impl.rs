//! Default Transform implementation for exact mode ImageLaPremultiplied8.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_default;

byte_mode_default!(
    ImageLaPremultiplied8,
    crate::raster::GrayAlphaImage,
    2,
    false,
    false,
    Generic,
    "transform ImageLaPremultiplied8 output buffer shape mismatch"
);

