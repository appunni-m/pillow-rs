//! Serial native-sample implementations for ImageRgb32F.

use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const MODE: Mode = Mode::ImageRgb32F;

// NO_PILLOW_SUPPORT: native f32 RGB Composite for an internal raster mode.

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
        CompositeLayout::F32 { channels: 3 },
    )
}
