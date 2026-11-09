//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    4,
    false,
    "ImageCmyk8",
    "transform ImageCmyk8 output shape mismatch"
);
