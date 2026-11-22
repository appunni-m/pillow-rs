//! Default transparent conversion from signed I samples.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute(
    source: &RgbaImage,
    target: Mode,
    key: [u8; 3],
) -> Result<DynamicImage, PilError> {
    let samples = source.as_raw().chunks_exact(4).map(|pixel| {
        i32::from_le_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]).clamp(0, i32::from(u8::MAX))
            as u8
    });
    crate::compute::pool_cpu::ops::color::helpers::convert::transparent::execute_gray_samples(
        source.width(),
        source.height(),
        target,
        key,
        samples,
    )
}
