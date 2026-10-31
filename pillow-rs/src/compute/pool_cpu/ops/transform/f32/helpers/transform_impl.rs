//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(ImageF32, crate::raster::RgbaImage, 4, "ImageF32", "transform ImageF32 output shape mismatch");
