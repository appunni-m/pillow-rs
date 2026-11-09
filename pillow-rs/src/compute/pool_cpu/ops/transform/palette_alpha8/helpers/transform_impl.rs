//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(
    ImagePaletteAlpha8,
    crate::raster::GrayAlphaImage,
    2,
    "ImagePaletteAlpha8",
    "transform ImagePaletteAlpha8 output shape mismatch"
);
