//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(
    ImagePalette8,
    crate::raster::GrayImage,
    1,
    "ImagePalette8",
    "transform ImagePalette8 output shape mismatch"
);
