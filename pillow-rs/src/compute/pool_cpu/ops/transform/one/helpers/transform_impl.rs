//! Native-byte nearest Transform route for the exact mode.

use crate::compute::pool_cpu::ops::transform::helpers::byte_mode_nearest;

byte_mode_nearest!(One, crate::raster::GrayImage, 1, "One", "transform One output shape mismatch");
