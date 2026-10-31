//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(ImageRgbPremultiplied8, crate::raster::RgbaImage, 4, "ImageRgbPremultiplied8", "transform ImageRgbPremultiplied8 output shape mismatch");
