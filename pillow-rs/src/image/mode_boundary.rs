//! Pillow spelling adaptation for the image compatibility boundary.
//!
//! Raster identities and operation strategy selection remain typed. Internal
//! high-depth identities deliberately have no Pillow spelling.

use crate::raster::Mode;

/// TODO(cpu-migration): keep this parser only at host/API boundaries; remove
/// internal callers once those entry points accept `Mode` directly.
pub(crate) fn parse_pillow_mode(name: &str) -> Option<Mode> {
    Mode::from_pillow_name(name)
}

pub(crate) const fn pillow_mode_name(mode: Mode) -> Option<&'static str> {
    mode.pillow_name()
}

pub(crate) const fn pillow_mode_name_opt(mode: Option<Mode>) -> Option<&'static str> {
    match mode {
        Some(mode) => pillow_mode_name(mode),
        None => None,
    }
}
