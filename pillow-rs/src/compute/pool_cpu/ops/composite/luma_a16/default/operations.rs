//! Serial native-sample implementations for ImageLumaA16.

use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const MODE: Mode = Mode::ImageLumaA16;

// NO_PILLOW_SUPPORT: native u16 luma-alpha Composite for an internal raster mode.

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
        CompositeLayout::U16 { channels: 2 },
    )
}
