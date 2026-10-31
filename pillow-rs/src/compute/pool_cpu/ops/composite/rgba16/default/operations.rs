//! Serial native-sample implementations for ImageRgba16.

use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const MODE: Mode = Mode::ImageRgba16;

// NO_PILLOW_SUPPORT: native u16 RGBA Composite for an internal raster mode.

pub(crate) fn composite(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::composite::execute(
        source,
        destination,
        mask,
        mask_alpha,
        MODE,
        CompositeLayout::U16 { channels: 4 },
    )
}
