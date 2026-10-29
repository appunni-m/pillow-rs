//! Dimension-driven EffectNoise implementation for exact input modes.

use crate::compute::helpers::effects::effect_noise_values;
use crate::error::PilError;
use crate::raster::{GrayImage, ImageBuffer, Pixel};

pub(crate) struct Kernel<'a, P: Pixel> {
    input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    sigma: f64,
}

pub(crate) fn prepare<'a, P: Pixel>(
    input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    sigma: f64,
) -> Kernel<'a, P> {
    Kernel { input, sigma }
}

pub(crate) fn execute<P: Pixel>(kernel: Kernel<'_, P>) -> Result<GrayImage, PilError> {
    let (width, height) = kernel.input.dimensions();
    let values = effect_noise_values(width, height, kernel.sigma)?;
    GrayImage::from_raw(width, height, values)
        .ok_or_else(|| PilError::ValueError("effect_noise buffer error".into()))
}
