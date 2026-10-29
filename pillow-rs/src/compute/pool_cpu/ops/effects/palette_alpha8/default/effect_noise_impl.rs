//! EffectNoise Default implementation for exact input mode ImagePaletteAlpha8.

use crate::error::PilError;
use crate::raster::GrayImage;
use super::super::ops::Input;

pub(crate) type Kernel<'a> = crate::compute::pool_cpu::ops::effects::helpers::noise::Kernel<'a, Input>;

pub(crate) fn prepare(input: &Input, sigma: f64) -> Kernel<'_> {
    crate::compute::pool_cpu::ops::effects::helpers::noise::prepare(input, sigma)
}

pub(crate) fn execute(kernel: Kernel<'_>) -> Result<GrayImage, PilError> {
    crate::compute::pool_cpu::ops::effects::helpers::noise::execute(kernel)
}
