//! Default transparent conversion from normalized bilevel samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &GrayImage,
    target: Mode,
    key: [u8; 3],
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::color::helpers::convert::transparent::execute_gray_samples(
        source.width(),
        source.height(),
        target,
        key,
        source.as_raw().iter().copied(),
    )
}
