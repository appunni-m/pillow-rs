//! Serial native-sample implementations for ImageLumaA8.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

const MODE: Mode = Mode::ImageLumaA8;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::merge::interleave(
        source,
        remaining,
        MODE,
        crate::compute::pool_cpu::ops::composite::ops::MergeLayout::Two,
    )
}

pub(crate) fn blend(
    source: &DynamicImage,
    other: &DynamicImage,
    alpha: f64,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::blend::execute(
        source, other, MODE, alpha, 2,
    )
}
