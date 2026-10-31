//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(ImageRgbx8, crate::raster::RgbaImage, 4, "ImageRgbx8", "transform ImageRgbx8 output shape mismatch");
