//! Buffer-preserving mode assignment at explicit image adapter boundaries.

use super::{DynamicImage, Mode};

impl DynamicImage {
    /// Assigns a Pillow mode to an image at an adapter boundary while
    /// preserving its existing sample buffer.
    ///
    /// The returned error contains the original image when its storage cannot
    /// represent the requested mode. Internal high-depth buffers are never
    /// reinterpreted as a Pillow mode by this adapter.
    pub(crate) fn with_mode(self, mode: Mode) -> Result<Self, Self> {
        if self.mode() == mode {
            return Ok(self);
        }
        match mode {
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => self.with_gray_mode(mode),
            Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => self.with_gray_alpha_mode(mode),
            Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => self.with_rgb_mode(mode),
            Mode::ImageRgba8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
            | Mode::ImageCmyk8
            | Mode::ImageI32
            | Mode::ImageF32 => self.with_four_byte_mode(mode),
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                self.with_luma16_mode(mode)
            }
            Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F => Err(self),
        }
    }

    fn with_gray_mode(self, mode: Mode) -> Result<Self, Self> {
        match (mode, self) {
            (Mode::One, Self::One(image) | Self::ImageLuma8(image) | Self::ImagePalette8(image)) => {
                Ok(Self::One(image))
            }
            (Mode::ImageLuma8, Self::One(image) | Self::ImageLuma8(image) | Self::ImagePalette8(image)) => {
                Ok(Self::ImageLuma8(image))
            }
            (Mode::ImagePalette8, Self::One(image) | Self::ImageLuma8(image) | Self::ImagePalette8(image)) => {
                Ok(Self::ImagePalette8(image))
            }
            (_, image) => Err(image),
        }
    }

    fn with_gray_alpha_mode(self, mode: Mode) -> Result<Self, Self> {
        match (mode, self) {
            (
                Mode::ImageLumaA8,
                Self::ImageLumaA8(image)
                | Self::ImageLaPremultiplied8(image)
                | Self::ImagePaletteAlpha8(image),
            ) => Ok(Self::ImageLumaA8(image)),
            (
                Mode::ImageLaPremultiplied8,
                Self::ImageLumaA8(image)
                | Self::ImageLaPremultiplied8(image)
                | Self::ImagePaletteAlpha8(image),
            ) => Ok(Self::ImageLaPremultiplied8(image)),
            (
                Mode::ImagePaletteAlpha8,
                Self::ImageLumaA8(image)
                | Self::ImageLaPremultiplied8(image)
                | Self::ImagePaletteAlpha8(image),
            ) => Ok(Self::ImagePaletteAlpha8(image)),
            (_, image) => Err(image),
        }
    }

    fn with_rgb_mode(self, mode: Mode) -> Result<Self, Self> {
        match (mode, self) {
            (
                Mode::ImageRgb8,
                Self::ImageRgb8(image)
                | Self::ImageYCbCr8(image)
                | Self::ImageLab8(image)
                | Self::ImageHsv8(image),
            ) => Ok(Self::ImageRgb8(image)),
            (
                Mode::ImageYCbCr8,
                Self::ImageRgb8(image)
                | Self::ImageYCbCr8(image)
                | Self::ImageLab8(image)
                | Self::ImageHsv8(image),
            ) => Ok(Self::ImageYCbCr8(image)),
            (
                Mode::ImageLab8,
                Self::ImageRgb8(image)
                | Self::ImageYCbCr8(image)
                | Self::ImageLab8(image)
                | Self::ImageHsv8(image),
            ) => Ok(Self::ImageLab8(image)),
            (
                Mode::ImageHsv8,
                Self::ImageRgb8(image)
                | Self::ImageYCbCr8(image)
                | Self::ImageLab8(image)
                | Self::ImageHsv8(image),
            ) => Ok(Self::ImageHsv8(image)),
            (_, image) => Err(image),
        }
    }

    fn with_four_byte_mode(self, mode: Mode) -> Result<Self, Self> {
        match (mode, self) {
            (
                Mode::ImageRgba8,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageRgba8(image)),
            (
                Mode::ImageRgbPremultiplied8,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageRgbPremultiplied8(image)),
            (
                Mode::ImageRgbx8,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageRgbx8(image)),
            (
                Mode::ImageCmyk8,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageCmyk8(image)),
            (
                Mode::ImageI32,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageI32(image)),
            (
                Mode::ImageF32,
                Self::ImageRgba8(image)
                | Self::ImageRgbPremultiplied8(image)
                | Self::ImageRgbx8(image)
                | Self::ImageCmyk8(image)
                | Self::ImageI32(image)
                | Self::ImageF32(image),
            ) => Ok(Self::ImageF32(image)),
            (_, image) => Err(image),
        }
    }

    fn with_luma16_mode(self, mode: Mode) -> Result<Self, Self> {
        match (mode, self) {
            (
                Mode::ImageI16,
                Self::ImageI16(image)
                | Self::ImageI16L(image)
                | Self::ImageI16B(image)
                | Self::ImageI16N(image),
            ) => Ok(Self::ImageI16(image)),
            (
                Mode::ImageI16L,
                Self::ImageI16(image)
                | Self::ImageI16L(image)
                | Self::ImageI16B(image)
                | Self::ImageI16N(image),
            ) => Ok(Self::ImageI16L(image)),
            (
                Mode::ImageI16B,
                Self::ImageI16(image)
                | Self::ImageI16L(image)
                | Self::ImageI16B(image)
                | Self::ImageI16N(image),
            ) => Ok(Self::ImageI16B(image)),
            (
                Mode::ImageI16N,
                Self::ImageI16(image)
                | Self::ImageI16L(image)
                | Self::ImageI16B(image)
                | Self::ImageI16N(image),
            ) => Ok(Self::ImageI16N(image)),
            (_, image) => Err(image),
        }
    }

}
