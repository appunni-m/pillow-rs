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

    pub(crate) fn samples(self) -> &'a [u8] {
        self.image().as_raw()
    }

    pub(crate) fn image(self) -> &'a GrayImage {
        match self {
            Self::One(image) | Self::ImageLuma8(image) => image,
        }
    }
}

fn mask_facts(mode: Mode, image: &GrayImage) -> SecondaryInputFacts {
    SecondaryInputFacts {
        mode,
        width: image.width(),
        height: image.height(),
        byte_len: Some(std::mem::size_of_val(image.as_raw().as_slice())),
    }
}
