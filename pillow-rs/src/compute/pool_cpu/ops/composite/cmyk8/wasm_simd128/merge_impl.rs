//! Portable wide interleave entry for exact ImageCmyk8 Merge.

use crate::compute::pool_cpu::ops::composite::ops::MergeLayout;
use crate::compute::pool_cpu::ops::composite::helpers::merge;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    merge::interleave_wide(source, remaining, Mode::ImageCmyk8, MergeLayout::Four)
}
