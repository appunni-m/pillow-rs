//! Portable wide interleave entry for exact ImageLumaA8 Merge.

use crate::compute::pool_cpu::ops::composite::helpers::merge;
use crate::compute::pool_cpu::ops::composite::ops::MergeLayout;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    merge::interleave_wide(source, remaining, Mode::ImageLumaA8, MergeLayout::Two)
}
