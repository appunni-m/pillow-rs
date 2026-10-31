//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_vector;

byte_mode_vector!(ImageRgba8, crate::raster::RgbaImage, 4, true, "ImageRgba8", "transform ImageRgba8 output shape mismatch");
