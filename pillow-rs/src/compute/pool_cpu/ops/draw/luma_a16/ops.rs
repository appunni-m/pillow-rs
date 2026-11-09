use crate::compute::registry::support::ModeSupport;
use crate::raster::Mode;

pub(crate) const MODE: Mode = Mode::ImageLumaA16;
// NO_PILLOW_SUPPORT: internal 16-bit LA samples do not have a Pillow ImageDraw contract.
pub(crate) const SUPPORT: ModeSupport = super::super::helpers::support::unsupported_support(MODE);

pub(crate) fn support() -> ModeSupport {
    SUPPORT
}
