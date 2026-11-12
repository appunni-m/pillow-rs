//! Shared destination construction for direct mode-native LAB strategies.

use crate::error::PilError;
use crate::raster::DynamicImage;

// TODO(pillow-rs#3ptb): verify all supported native source-mode LAB outputs,
// fault cases, architecture receipts, and default fallback receipts in CPU parity.
/// Runs the shared LAB transform over samples already decoded by a source-mode strategy.
#[inline]
pub(crate) fn execute<I>(width: u32, height: u32, samples: I) -> Result<DynamicImage, PilError>
where
    I: ExactSizeIterator<Item = [u8; 3]>,
{
    crate::compute::pool_cpu::ops::color::helpers::lab_kernel::convert_rgb_samples(
        width, height, samples,
    )
    .map(DynamicImage::ImageLab8)
}
