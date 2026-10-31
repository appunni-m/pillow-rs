//! Native-byte affine-nearest/bilinear Transform route for exact L samples.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(
    ImageLuma8,
    crate::raster::GrayImage,
    1,
    false,
    "ImageLuma8",
    "transform ImageLuma8 output shape mismatch"
);
