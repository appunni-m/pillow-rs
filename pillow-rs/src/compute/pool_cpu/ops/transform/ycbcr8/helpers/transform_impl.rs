//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(ImageYCbCr8, crate::raster::RgbImage, 3, false, "ImageYCbCr8", "transform ImageYCbCr8 output shape mismatch");
