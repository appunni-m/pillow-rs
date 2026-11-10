//! Mode-typed empty image construction used by geometry operations.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::Image;

impl Image {
    /// Creates a new mode-typed image with Pillow's default zero fill.
    ///
    /// Unlike `Image::new`, this also supports internal raster identities that
    /// do not have a Pillow-facing `Image.new` mode spelling. Palette mode
    /// starts without an attached palette, matching `Image.new("P", size)`.
    pub(crate) fn new_empty_with_mode(
        width: u32,
        height: u32,
        mode: Mode,
    ) -> Result<Self, PilError> {
        if mode == Mode::ImagePalette8 {
            return Ok(Self::new_palette_index(width, height, 0));
        }
        let mut image = match mode {
            Mode::One => DynamicImage::One(crate::raster::GrayImage::new(width, height)),
            Mode::ImageLuma8 => {
                DynamicImage::ImageLuma8(crate::raster::GrayImage::new(width, height))
            }
            Mode::ImageLumaA8 => {
                DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::new(width, height))
            }
            Mode::ImageLaPremultiplied8 => DynamicImage::ImageLaPremultiplied8(
                crate::raster::GrayAlphaImage::new(width, height),
            ),
            Mode::ImagePaletteAlpha8 => {
                DynamicImage::ImagePaletteAlpha8(crate::raster::GrayAlphaImage::new(width, height))
            }
            Mode::ImageRgb8 => DynamicImage::ImageRgb8(crate::raster::RgbImage::new(width, height)),
            Mode::ImageYCbCr8 => {
                DynamicImage::ImageYCbCr8(crate::raster::RgbImage::new(width, height))
            }
            Mode::ImageLab8 => DynamicImage::ImageLab8(crate::raster::RgbImage::new(width, height)),
            Mode::ImageHsv8 => DynamicImage::ImageHsv8(crate::raster::RgbImage::new(width, height)),
            Mode::ImageRgba8 => {
                DynamicImage::ImageRgba8(crate::raster::RgbaImage::new(width, height))
            }
            Mode::ImageRgbPremultiplied8 => {
                DynamicImage::ImageRgbPremultiplied8(crate::raster::RgbaImage::new(width, height))
            }
            Mode::ImageRgbx8 => {
                DynamicImage::ImageRgbx8(crate::raster::RgbaImage::new(width, height))
            }
            Mode::ImageCmyk8 => {
                DynamicImage::ImageCmyk8(crate::raster::RgbaImage::new(width, height))
            }
            Mode::ImageI32 => DynamicImage::ImageI32(crate::raster::RgbaImage::new(width, height)),
            Mode::ImageF32 => DynamicImage::ImageF32(crate::raster::RgbaImage::new(width, height)),
            Mode::ImageI16 => {
                DynamicImage::ImageI16(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageI16L => {
                DynamicImage::ImageI16L(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageI16B => {
                DynamicImage::ImageI16B(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageI16N => {
                DynamicImage::ImageI16N(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageLumaA16 => {
                DynamicImage::ImageLumaA16(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageRgb16 => {
                DynamicImage::ImageRgb16(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageRgba16 => {
                DynamicImage::ImageRgba16(crate::raster::ImageBuffer::new(width, height))
            }
            Mode::ImageRgb32F => {
                DynamicImage::ImageRgb32F(crate::raster::Rgb32FImage::new(width, height))
            }
            Mode::ImageRgba32F => {
                DynamicImage::ImageRgba32F(crate::raster::Rgba32FImage::new(width, height))
            }
            Mode::ImagePalette8 => unreachable!("palette mode returned above"),
        };
        super::initialize_new_hidden_lanes(&mut image, mode, (0, 0, 0, 0), false)?;
        crate::compute::record_pipeline_allocation(image.as_bytes().len());
        Ok(Self::from_dynamic(image))
    }
}
