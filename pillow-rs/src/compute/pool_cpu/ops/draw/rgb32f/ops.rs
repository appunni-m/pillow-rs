use crate::compute::registry::support::ModeSupport;
use crate::raster::Mode;

pub(crate) const MODE: Mode = Mode::ImageRgb32F;

pub(crate) fn support() -> ModeSupport {
    // NO_PILLOW_SUPPORT: internal 32-bit float RGB samples do not have a Pillow ImageDraw contract.
    super::super::ops::unsupported_mode(MODE)
}
