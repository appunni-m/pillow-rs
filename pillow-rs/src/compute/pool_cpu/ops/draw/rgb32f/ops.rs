use crate::compute::registry::support::{ModeSupport, RejectionKind};
use crate::raster::Mode;

pub(crate) const MODE: Mode = Mode::ImageRgb32F;

pub(crate) fn support() -> ModeSupport {
    // NO_PILLOW_SUPPORT: internal 32-bit float RGB samples do not have a Pillow ImageDraw contract.
    ModeSupport::Rejected {
        mode: MODE,
        reason: "unsupported image mode",
        kind: RejectionKind::ValueError,
    }
}
