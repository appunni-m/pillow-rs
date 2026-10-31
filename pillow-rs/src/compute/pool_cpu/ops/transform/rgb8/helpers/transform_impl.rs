//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(ImageRgb8, crate::raster::RgbImage, 3, false, "ImageRgb8", "transform ImageRgb8 output shape mismatch");
