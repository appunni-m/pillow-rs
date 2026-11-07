//! Pillow spelling adaptation for the image compatibility boundary.
//!
//! Raster identities and operation strategy selection remain typed. Internal
//! high-depth identities deliberately have no Pillow spelling.

use crate::raster::Mode;

/// TODO(pillow-rs#3ptb): replace the remaining GPU result-metadata caller with
/// a typed `Mode` when the GPU compatibility route adopts the raster contract.
/// CPU operation selection reads the mode carried by `DynamicImage` directly.
pub(crate) fn parse_pillow_mode(name: &str) -> Option<Mode> {
    Mode::from_pillow_name(name)
}

pub(crate) const fn pillow_mode_name(mode: Mode) -> Option<&'static str> {
    mode.pillow_name()
}
