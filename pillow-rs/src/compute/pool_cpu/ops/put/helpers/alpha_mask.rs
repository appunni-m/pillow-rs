//! Exact secondary-mask strategies for image-backed alpha replacement.

use crate::compute::registry::support::SecondaryInputFacts;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

#[derive(Clone, Copy)]
pub(crate) enum MaskStrategy<'a> {
    One(&'a GrayImage),
    ImageLuma8(&'a GrayImage),
}

impl<'a> MaskStrategy<'a> {
    pub(crate) fn new(image: &'a DynamicImage) -> Result<Self, PilError> {
        match image {
            DynamicImage::One(image) => Ok(Self::One(image)),
            DynamicImage::ImageLuma8(image) => Ok(Self::ImageLuma8(image)),
            _ => Err(PilError::ValueError("illegal image mode".into())),
        }
    }

    pub(crate) fn facts(&self) -> SecondaryInputFacts {
        match self {
            Self::One(image) => mask_facts(Mode::One, image),
            Self::ImageLuma8(image) => mask_facts(Mode::ImageLuma8, image),
        }
    }

    pub(crate) fn with_samples<T>(
        self,
        one: impl FnOnce(&[u8]) -> T,
        luma: impl FnOnce(&[u8]) -> T,
    ) -> T {
        match self {
            Self::One(image) => one(image.as_raw()),
            Self::ImageLuma8(image) => luma(image.as_raw()),
        }
    }

    pub(crate) fn image(self) -> &'a GrayImage {
        match self {
            Self::One(image) | Self::ImageLuma8(image) => image,
        }
    }
}

/// Pillow converts mode `1` alpha masks to `L`: zero remains zero and every
/// nonzero sample becomes 255. Keep that normalization fused into the target
/// operation instead of allocating an intermediate luma image.
/// Source: Pillow 12.2.0 `Convert.c::bit2l`.
#[inline(always)]
pub(crate) fn normalize_sample<const ONE: bool>(sample: u8) -> u8 {
    if ONE {
        if sample == 0 { 0 } else { u8::MAX }
    } else {
        sample
    }
}

// TODO(pillow-rs#3ptb): parity-check noncanonical One mask samples (0, 1, 255)
// across destination modes and selected Default/vector strategies.

fn mask_facts(mode: Mode, image: &GrayImage) -> SecondaryInputFacts {
    SecondaryInputFacts {
        mode,
        width: image.width(),
        height: image.height(),
        byte_len: Some(std::mem::size_of_val(image.as_raw().as_slice())),
    }
}
