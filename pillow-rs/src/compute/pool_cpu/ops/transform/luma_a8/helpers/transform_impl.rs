//! Native-byte affine-nearest/bilinear Transform route for exact LA samples.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    2,
    true,
    "ImageLumaA8",
    "transform ImageLumaA8 output shape mismatch"
);
