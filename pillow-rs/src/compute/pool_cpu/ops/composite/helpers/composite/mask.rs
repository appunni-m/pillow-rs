//! Exact luma or alpha sample extraction for Composite masks.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

#[derive(Clone, Copy)]
pub(super) struct Samples<'a> {
    bytes: &'a [u8],
    channels: usize,
    sample: usize,
    binary: bool,
}

impl Samples<'_> {
    pub(super) fn len(self) -> usize {
        self.bytes.len() / self.channels
    }

    #[inline]
    pub(super) fn get(self, pixel: usize) -> u8 {
        let value = self.bytes[pixel * self.channels + self.sample];
        if self.binary && value != 0 { 255 } else { value }
    }
}

pub(super) fn samples(
    mask: &DynamicImage,
    mask_alpha: bool,
) -> Result<Samples<'_>, PilError> {
    let (channels, sample, binary) = match (mask.mode(), mask_alpha) {
        (Mode::One, false) => (1usize, 0usize, true),
        (Mode::ImageLuma8, false) => (1, 0, false),
        (Mode::ImageLumaA8, true) => (2, 1, false),
        (Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8, true) => (4, 3, false),
        _ => return Err(PilError::ValueError("bad transparency mask".into())),
    };
    let dimensions = CheckedDims::new_allow_empty(mask.width(), mask.height(), channels as u8)?;
    let bytes = mask.as_bytes();
    if bytes.len() != dimensions.total_bytes() {
        return Err(PilError::ValueError("bad transparency mask".into()));
    }
    Ok(Samples { bytes, channels, sample, binary })
}
