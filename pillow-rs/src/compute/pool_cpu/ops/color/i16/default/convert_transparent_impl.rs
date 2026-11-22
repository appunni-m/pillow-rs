//! Default transparent conversion from native unsigned I;16 samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

type Samples = ImageBuffer<Luma<u16>, Vec<u16>>;

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &Samples,
    target: Mode,
    key: [u8; 3],
) -> Result<DynamicImage, PilError> {
    let samples = source
        .pixels()
        .map(|pixel| pixel[0].min(u16::from(u8::MAX)) as u8);
    crate::compute::pool_cpu::ops::color::helpers::convert::transparent::execute_gray_samples(
        source.width(),
        source.height(),
        target,
        key,
        samples,
    )
}
