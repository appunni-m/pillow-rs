//! Exact secondary-mask strategies selected once before alpha execution.

use crate::raster::{ConvertBuffer, DynamicImage, GrayImage, Mode};
use crate::compute::registry::support::SecondaryInputFacts;

macro_rules! declare_masks {
    ($($mode:ident($input:ty) => |$image:ident| $normalize:expr;)+) => {
        #[derive(Clone, Copy)]
        pub(crate) enum MaskStrategy<'a> { $($mode(&'a $input),)+ }

        impl<'a> MaskStrategy<'a> {
            pub(crate) fn new(image: &'a DynamicImage) -> Self {
                match image { $(DynamicImage::$mode(image) => Self::$mode(image),)+ }
            }

            pub(crate) fn facts(&self) -> SecondaryInputFacts {
                match self { $(Self::$mode(image) => SecondaryInputFacts {
                    mode: Mode::$mode, width: image.width(), height: image.height(),
                    byte_len: Some(std::mem::size_of_val(image.as_raw().as_slice())),
                },)+ }
            }

            pub(crate) fn to_luma8(self) -> GrayImage {
                match self { $(Self::$mode($image) => $normalize,)+ }
            }

            pub(crate) fn native_luma(self) -> Option<&'a GrayImage> {
                match self { Self::ImageLuma8(image) => Some(image), _ => None }
            }
        }
    };
}

declare_masks! {
    One(crate::raster::GrayImage) => |image| image.clone();
    ImageLuma8(crate::raster::GrayImage) => |image| image.clone();
    ImageLumaA8(crate::raster::GrayAlphaImage) => |image| image.convert();
    ImageLaPremultiplied8(crate::raster::GrayAlphaImage) => |image| image.convert();
    ImagePalette8(crate::raster::GrayImage) => |image| image.clone();
    ImagePaletteAlpha8(crate::raster::GrayAlphaImage) => |image| image.convert();
    ImageRgb8(crate::raster::RgbImage) => |image| image.convert();
    ImageRgba8(crate::raster::RgbaImage) => |image| image.convert();
    ImageRgbPremultiplied8(crate::raster::RgbaImage) => |image| image.convert();
    ImageRgbx8(crate::raster::RgbaImage) => |image| image.convert();
    ImageCmyk8(crate::raster::RgbaImage) => |image| image.convert();
    ImageYCbCr8(crate::raster::RgbImage) => |image| image.convert();
    ImageLab8(crate::raster::RgbImage) => |image| image.convert();
    ImageHsv8(crate::raster::RgbImage) => |image| image.convert();
    ImageI32(crate::raster::RgbaImage) => |image| image.convert();
    ImageF32(crate::raster::RgbaImage) => |image| image.convert();
    ImageI16(crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>) => |image| super::put_data::clipped(image, |sample| crate::raster::Luma([sample]));
    ImageI16L(crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>) => |image| super::put_data::clipped(image, |sample| crate::raster::Luma([sample]));
    ImageI16B(crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>) => |image| super::put_data::clipped(image, |sample| crate::raster::Luma([sample]));
    ImageI16N(crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>) => |image| super::put_data::clipped(image, |sample| crate::raster::Luma([sample]));
    ImageLumaA16(crate::raster::ImageBuffer<crate::raster::LumaA<u16>, Vec<u16>>) => |image| image.convert();
    ImageRgb16(crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>) => |image| image.convert();
    ImageRgba16(crate::raster::ImageBuffer<crate::raster::Rgba<u16>, Vec<u16>>) => |image| image.convert();
    ImageRgb32F(crate::raster::Rgb32FImage) => |image| image.convert();
    ImageRgba32F(crate::raster::Rgba32FImage) => |image| image.convert();
}
