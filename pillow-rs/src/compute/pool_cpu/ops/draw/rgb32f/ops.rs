use crate::compute::registry::support::ModeSupport;
use crate::raster::Mode;

pub(crate) const MODE: Mode = Mode::ImageRgb32F;
// NO_PILLOW_SUPPORT: internal 32-bit float RGB samples do not have a Pillow ImageDraw contract.
pub(crate) const SUPPORT: ModeSupport = super::super::helpers::support::unsupported_support(MODE);

pub(crate) fn support() -> ModeSupport {
    SUPPORT
}
