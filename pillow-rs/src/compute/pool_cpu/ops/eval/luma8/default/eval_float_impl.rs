//! Native L sample lookup into Pillow's floating output mode.

use crate::compute::pool_cpu::ops::eval::helpers::point_float;
use crate::error::PilError;
use crate::raster::{GrayImage, RgbaImage};

pub(crate) fn execute(input: &GrayImage, lut: &[f64]) -> Result<RgbaImage, PilError> {
    point_float::execute(input, lut)
}
