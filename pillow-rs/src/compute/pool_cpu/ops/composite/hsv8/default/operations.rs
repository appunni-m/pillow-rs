//! Serial native-sample implementations for ImageHsv8.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

const MODE: Mode = Mode::ImageHsv8;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::merge::interleave(
        source,
        remaining,
        MODE,
        crate::compute::pool_cpu::ops::composite::ops::MergeLayout::Three {
            lab_encoding: false,
        },
    )
}
