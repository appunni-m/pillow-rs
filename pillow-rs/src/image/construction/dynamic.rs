use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::{Image, initialize_new_hidden_lanes};

impl Image {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    pub(super) fn new_dynamic_image(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
        color_provided: bool,
    ) -> Result<DynamicImage, PilError> {
        let mut image = match mode {
            Mode::ImageRgb8 => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            Mode::ImageRgba8 => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageRgbPremultiplied8 => {
                DynamicImage::ImageRgbPremultiplied8(crate::raster::RgbaImage::from_pixel(
                    width,
                    height,
                    crate::raster::Rgba([color.0, color.1, color.2, color.3]),
                ))
            }
            Mode::ImageRgbx8 => DynamicImage::ImageRgbx8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageLuma8 => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_pixel(
                width,
                height,
                crate::raster::Luma([color.0]),
            )),
            Mode::One => DynamicImage::One(crate::raster::GrayImage::from_pixel(
                width,
                height,
                // Pillow stores the supplied Image.new color byte verbatim;
                // this constructor does not threshold it to a bit value.
                crate::raster::Luma([color.0]),
            )),
            Mode::ImageLumaA8 => {
                DynamicImage::ImageLumaA8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            Mode::ImageLaPremultiplied8 => {
                DynamicImage::ImageLaPremultiplied8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            // The binding packs the low 16 bits of an Image.new scalar into
            // color.0/color.1 in little-endian order. Keep the logical sample
            // native in the raster and apply the requested raw byte order only
            // when the public byte representation is requested.
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
                Self::new_i16_dynamic_image(width, height, mode, color)
            }
            Mode::ImagePaletteAlpha8 => {
                DynamicImage::ImagePaletteAlpha8(crate::raster::GrayAlphaImage::from_pixel(
                    width,
                    height,
                    crate::raster::LumaA([color.0, color.3]),
                ))
            }
            Mode::ImageCmyk8 | Mode::ImageYCbCr8 | Mode::ImageHsv8 => {
                Self::new_color_space_dynamic_image(width, height, mode, color)
            }
            Mode::ImageLab8 => Self::new_lab8_dynamic_image(width, height, color),
            Mode::ImageI32 => DynamicImage::ImageI32(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageF32 => DynamicImage::ImageF32(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImagePalette8 => unreachable!("palette mode is handled by Image::new"),
            Mode::ImageLumaA16
            | Mode::ImageRgb16
            | Mode::ImageRgba16
            | Mode::ImageRgb32F
            | Mode::ImageRgba32F => {
                // NO_PILLOW_SUPPORT: these internal storage identities have
                // no Pillow Image.new spelling or scalar-fill contract.
                return Err(PilError::ValueError(
                    "unsupported image mode for Image.new".into(),
                ));
            }
        };
        initialize_new_hidden_lanes(&mut image, mode, color, color_provided)?;
        Ok(image)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn new_i16_dynamic_image(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
    ) -> DynamicImage {
        let image = crate::raster::ImageBuffer::from_pixel(
            width,
            height,
            crate::raster::Luma([u16::from_le_bytes([color.0, color.1])]),
        );
        match mode {
            Mode::ImageI16 => DynamicImage::ImageI16(image),
            Mode::ImageI16L => DynamicImage::ImageI16L(image),
            Mode::ImageI16B => DynamicImage::ImageI16B(image),
            Mode::ImageI16N => DynamicImage::ImageI16N(image),
            _ => unreachable!("matched 16-bit luma mode"),
        }
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn new_lab8_dynamic_image(width: u32, height: u32, color: (u8, u8, u8, u8)) -> DynamicImage {
        DynamicImage::ImageLab8(crate::raster::RgbImage::from_pixel(
            width,
            height,
            crate::raster::Rgb([
                color.0,
                color.1.wrapping_add(128),
                color.2.wrapping_add(128),
            ]),
        ))
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn new_color_space_dynamic_image(
        width: u32,
        height: u32,
        mode: Mode,
        color: (u8, u8, u8, u8),
    ) -> DynamicImage {
        match mode {
            Mode::ImageCmyk8 => DynamicImage::ImageCmyk8(crate::raster::RgbaImage::from_pixel(
                width,
                height,
                crate::raster::Rgba([color.0, color.1, color.2, color.3]),
            )),
            Mode::ImageYCbCr8 => DynamicImage::ImageYCbCr8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            Mode::ImageHsv8 => DynamicImage::ImageHsv8(crate::raster::RgbImage::from_pixel(
                width,
                height,
                crate::raster::Rgb([color.0, color.1, color.2]),
            )),
            _ => unreachable!("matched a raw color-space mode"),
        }
    }
}
