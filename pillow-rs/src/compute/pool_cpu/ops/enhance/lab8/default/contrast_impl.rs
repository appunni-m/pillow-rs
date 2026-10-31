//! Default LAB Contrast implementation.

use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbImage};

pub(crate) fn execute(
    source: &RgbImage,
    base: &RgbImage,
    factor: f64,
    blend: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::blend::execute_lab_samples(
        base, source, factor, blend,
    )
}
