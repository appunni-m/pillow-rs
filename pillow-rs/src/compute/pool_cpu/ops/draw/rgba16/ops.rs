use crate::compute::registry::support::ModeSupport;
use crate::raster::Mode;

pub(crate) const MODE: Mode = Mode::ImageRgba16;

pub(crate) fn support() -> ModeSupport {
    // NO_PILLOW_SUPPORT: internal 16-bit RGBA samples do not have a Pillow ImageDraw contract.
    super::super::ops::unsupported_mode(MODE)
}
