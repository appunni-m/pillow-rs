//! Pillow-owned materialized images with exact mode-bearing variants.
//!
//! Typed buffers carry samples; the enum tag carries Pillow raster identity.
//! Codec translation is delegated to the Pillow-owned image I/O adapters.

use super::buffer::{
    ConvertBuffer, GrayAlphaImage, GrayImage, ImageBuffer, Rgb32FImage, RgbImage, Rgba32FImage,
    RgbaImage,
};
use super::color::{self, ColorType, Luma, LumaA, Rgb, Rgba};
use super::mode::Mode;
use super::traits::{GenericImageView, Pixel, Primitive};

use crate::raster::color::FromColor;

mod mode;
mod transform;

type Luma16Image = ImageBuffer<Luma<u16>, Vec<u16>>;
type LumaA16Image = ImageBuffer<LumaA<u16>, Vec<u16>>;
type Rgb16Image = ImageBuffer<Rgb<u16>, Vec<u16>>;
type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;

macro_rules! for_each_dynamic_variant {
    ($macro:ident) => {
        $macro! {
            /// Pillow's bilevel `1` mode, expanded to one byte per pixel.
            One(GrayImage),
            /// Pillow's 8-bit luma (`L`) mode.
            ImageLuma8(GrayImage),
            /// Pillow's 8-bit luma with alpha (`LA`) mode.
            ImageLumaA8(GrayAlphaImage),
            /// Pillow's premultiplied luma with alpha (`La`) mode.
            ImageLaPremultiplied8(GrayAlphaImage),
            /// Pillow's 8-bit palette-index (`P`) mode.
            ImagePalette8(GrayImage),
            /// Pillow's palette-index with alpha (`PA`) mode.
            ImagePaletteAlpha8(GrayAlphaImage),
            /// Pillow's 8-bit RGB mode.
            ImageRgb8(RgbImage),
            /// Pillow's 8-bit RGBA mode.
            ImageRgba8(RgbaImage),
            /// Pillow's premultiplied RGBA (`RGBa`) mode.
            ImageRgbPremultiplied8(RgbaImage),
            /// Pillow's 8-bit RGB with an unused padding byte (`RGBX`) mode.
            ImageRgbx8(RgbaImage),
            /// Pillow's 8-bit CMYK mode, stored in four raw bytes per pixel.
            ImageCmyk8(RgbaImage),
            /// Pillow's 8-bit YCbCr mode.
            ImageYCbCr8(RgbImage),
            /// Pillow's 8-bit LAB mode.
            ImageLab8(RgbImage),
            /// Pillow's 8-bit HSV mode.
            ImageHsv8(RgbImage),
            /// Pillow's signed 32-bit integer (`I`) samples in the existing raw carrier.
            ImageI32(RgbaImage),
            /// Pillow's 32-bit float (`F`) samples in the existing raw carrier.
            ImageF32(RgbaImage),
            /// Pillow's native-endian unsigned 16-bit (`I;16`) mode.
            ImageI16(Luma16Image),
            /// Pillow's little-endian unsigned 16-bit (`I;16L`) mode.
            ImageI16L(Luma16Image),
            /// Pillow's big-endian unsigned 16-bit (`I;16B`) mode.
            ImageI16B(Luma16Image),
            /// Pillow's native-order unsigned 16-bit (`I;16N`) mode.
            ImageI16N(Luma16Image),
            /// Internal 16-bit luma-alpha storage, not a Pillow mode identity.
            ImageLumaA16(LumaA16Image),
            /// Internal 16-bit RGB storage, not a Pillow mode identity.
            ImageRgb16(Rgb16Image),
            /// Internal 16-bit RGBA storage, not a Pillow mode identity.
            ImageRgba16(Rgba16Image),
            /// Internal 32-bit float RGB storage, not a Pillow mode identity.
            ImageRgb32F(Rgb32FImage),
            /// Internal 32-bit float RGBA storage, not a Pillow mode identity.
            ImageRgba32F(Rgba32FImage),
        }
    };
}

macro_rules! define_dynamic_image {
    ($( $(#[$attribute:meta])* $variant:ident($image:ty), )+) => {
        /// A materialized image whose variant preserves its exact Pillow mode.
        ///
        /// High-depth multichannel variants retain the existing internal
        /// representations that are not Pillow `ImageMode`s.
        #[derive(Debug, PartialEq)]
        #[non_exhaustive]
        pub enum DynamicImage {
            $( $(#[$attribute])* $variant($image), )+
        }
    };
}

for_each_dynamic_variant!(define_dynamic_image);

macro_rules! dynamic_map(
    ($dynimage: expr, $image:pat_param, $action: expr) => (
        match $dynimage {
            DynamicImage::One($image) => $action,
            DynamicImage::ImageLuma8($image) => $action,
            DynamicImage::ImagePalette8($image) => $action,
            DynamicImage::ImageLumaA8($image) => $action,
            DynamicImage::ImageLaPremultiplied8($image) => $action,
            DynamicImage::ImagePaletteAlpha8($image) => $action,
            DynamicImage::ImageRgb8($image) => $action,
            DynamicImage::ImageYCbCr8($image) => $action,
            DynamicImage::ImageLab8($image) => $action,
            DynamicImage::ImageHsv8($image) => $action,
            DynamicImage::ImageRgba8($image) => $action,
            DynamicImage::ImageRgbPremultiplied8($image) => $action,
            DynamicImage::ImageRgbx8($image) => $action,
            DynamicImage::ImageCmyk8($image) => $action,
            DynamicImage::ImageI32($image) => $action,
            DynamicImage::ImageF32($image) => $action,
            DynamicImage::ImageI16($image) => $action,
            DynamicImage::ImageI16L($image) => $action,
            DynamicImage::ImageI16B($image) => $action,
            DynamicImage::ImageI16N($image) => $action,
            DynamicImage::ImageLumaA16($image) => $action,
            DynamicImage::ImageRgb16($image) => $action,
            DynamicImage::ImageRgba16($image) => $action,
            DynamicImage::ImageRgb32F($image) => $action,
            DynamicImage::ImageRgba32F($image) => $action,
        }
    );
);

#[inline]
fn luma16_to_u8(sample: u16) -> u8 {
    sample.min(u16::from(u8::MAX)) as u8
}

macro_rules! impl_dynamic_clone {
    ($( $(#[$attribute:meta])* $variant:ident($image:ty), )+) => {
        impl Clone for DynamicImage {
    fn clone(&self) -> Self {
        match self {
            $( Self::$variant(image) => Self::$variant(image.clone()), )+
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            $( (Self::$variant(p1), Self::$variant(p2)) => p1.clone_from(p2), )+
            (this, source) => *this = source.clone(),
        }
    }
    }
    };
}

for_each_dynamic_variant!(impl_dynamic_clone);

#[inline]
pub(crate) fn crop_buffer<P: Pixel>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> ImageBuffer<P, Vec<P::Subpixel>> {
    let mut output = ImageBuffer::new(width, height);
    for dy in 0..height {
        for dx in 0..width {
            let pixel = *image.get_pixel(x.saturating_add(dx), y.saturating_add(dy));
            output.put_pixel(dx, dy, pixel);
        }
    }
    output
}

impl DynamicImage {
    /// Returns the exact raster identity from this image's existing variant tag.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        match self {
            Self::One(_) => Mode::One,
            Self::ImageLuma8(_) => Mode::ImageLuma8,
            Self::ImageLumaA8(_) => Mode::ImageLumaA8,
            Self::ImageLaPremultiplied8(_) => Mode::ImageLaPremultiplied8,
            Self::ImagePalette8(_) => Mode::ImagePalette8,
            Self::ImagePaletteAlpha8(_) => Mode::ImagePaletteAlpha8,
            Self::ImageRgb8(_) => Mode::ImageRgb8,
            Self::ImageRgba8(_) => Mode::ImageRgba8,
            Self::ImageRgbPremultiplied8(_) => Mode::ImageRgbPremultiplied8,
            Self::ImageRgbx8(_) => Mode::ImageRgbx8,
            Self::ImageCmyk8(_) => Mode::ImageCmyk8,
            Self::ImageYCbCr8(_) => Mode::ImageYCbCr8,
            Self::ImageLab8(_) => Mode::ImageLab8,
            Self::ImageHsv8(_) => Mode::ImageHsv8,
            Self::ImageI32(_) => Mode::ImageI32,
            Self::ImageF32(_) => Mode::ImageF32,
            Self::ImageI16(_) => Mode::ImageI16,
            Self::ImageI16L(_) => Mode::ImageI16L,
            Self::ImageI16B(_) => Mode::ImageI16B,
            Self::ImageI16N(_) => Mode::ImageI16N,
            Self::ImageLumaA16(_) => Mode::ImageLumaA16,
            Self::ImageRgb16(_) => Mode::ImageRgb16,
            Self::ImageRgba16(_) => Mode::ImageRgba16,
            Self::ImageRgb32F(_) => Mode::ImageRgb32F,
            Self::ImageRgba32F(_) => Mode::ImageRgba32F,
        }
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels.
    #[must_use]
    pub fn new_rgba8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::new(w, h))
    }

    /// Returns a copy of this image as an RGB image.
    #[must_use]
    pub fn to_rgb8(&self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x.clone(),
            DynamicImage::ImageI16(image)
            | DynamicImage::ImageI16L(image)
            | DynamicImage::ImageI16B(image)
            | DynamicImage::ImageI16N(image) => {
                // Pillow's I;16-to-byte conversions clip integer samples to
                // the byte range; they do not normalize 0..65535 to 0..255.
                let (width, height) = image.dimensions();
                RgbImage::from_fn(width, height, |x, y| {
                    let sample = luma16_to_u8(image.get_pixel(x, y)[0]);
                    Rgb([sample, sample, sample])
                })
            }
            _x => self.to_generic::<Rgb<u8>>(),
        }
    }

    /// Returns a copy of this image as an RGBA image.
    #[must_use]
    pub fn to_rgba8(&self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x.clone(),
            DynamicImage::ImageI16(image)
            | DynamicImage::ImageI16L(image)
            | DynamicImage::ImageI16B(image)
            | DynamicImage::ImageI16N(image) => {
                let (width, height) = image.dimensions();
                RgbaImage::from_fn(width, height, |x, y| {
                    let sample = luma16_to_u8(image.get_pixel(x, y)[0]);
                    Rgba([sample, sample, sample, u8::MAX])
                })
            }
            _x => self.to_generic::<Rgba<u8>>(),
        }
    }

    /// Returns a copy of this image as a Luma image.
    #[must_use]
    pub fn to_luma8(&self) -> GrayImage {
        match self {
            DynamicImage::One(x)
            | DynamicImage::ImageLuma8(x)
            | DynamicImage::ImagePalette8(x) => x.clone(),
            DynamicImage::ImageI16(image)
            | DynamicImage::ImageI16L(image)
            | DynamicImage::ImageI16B(image)
            | DynamicImage::ImageI16N(image) => {
                let (width, height) = image.dimensions();
                GrayImage::from_fn(width, height, |x, y| {
                    Luma([luma16_to_u8(image.get_pixel(x, y)[0])])
                })
            }
            _x => self.to_generic::<Luma<u8>>(),
        }
    }

    /// Returns a copy of this image as a LumaA image.
    #[must_use]
    pub fn to_luma_alpha8(&self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x)
            | DynamicImage::ImageLaPremultiplied8(x)
            | DynamicImage::ImagePaletteAlpha8(x) => x.clone(),
            DynamicImage::ImageI16(image)
            | DynamicImage::ImageI16L(image)
            | DynamicImage::ImageI16B(image)
            | DynamicImage::ImageI16N(image) => {
                let (width, height) = image.dimensions();
                GrayAlphaImage::from_fn(width, height, |x, y| {
                    LumaA([luma16_to_u8(image.get_pixel(x, y)[0]), u8::MAX])
                })
            }
            _x => self.to_generic::<LumaA<u8>>(),
        }
    }

    /// Returns a copy of this image as a Luma image (16-bit).
    #[must_use]
    pub fn to_luma16(&self) -> ImageBuffer<Luma<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageI16(x)
            | DynamicImage::ImageI16L(x)
            | DynamicImage::ImageI16B(x)
            | DynamicImage::ImageI16N(x) => x.clone(),
            _x => self.to_generic::<Luma<u16>>(),
        }
    }

    /// Internal helper: convert to a generic pixel type using pixel conversion.
    fn to_generic<Px>(&self) -> ImageBuffer<Px, Vec<<Px as Pixel>::Subpixel>>
    where
        Px: Pixel
            + FromColor<color::Rgb<u8>>
            + FromColor<color::Rgba<u8>>
            + FromColor<color::Luma<u8>>
            + FromColor<color::LumaA<u8>>
            + FromColor<color::Rgb<u16>>
            + FromColor<color::Rgba<u16>>
            + FromColor<color::Rgb<f32>>
            + FromColor<color::Rgba<f32>>
            + FromColor<color::Luma<u16>>
            + FromColor<color::LumaA<u16>>,
    {
        dynamic_map!(self, img, { img.convert() })
    }

    /// Consume the image and returns a RGB image.
    #[must_use]
    pub fn into_rgb8(self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x,
            x => x.to_rgb8(),
        }
    }

    /// Consume the image and returns a RGBA image.
    #[must_use]
    pub fn into_rgba8(self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x,
            x => x.to_rgba8(),
        }
    }

    /// Consume the image and returns a Luma image.
    #[must_use]
    pub fn into_luma8(self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x,
            x => x.to_luma8(),
        }
    }

    /// Consume the image and returns a LumaA image.
    #[must_use]
    pub fn into_luma_alpha8(self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x,
            x => x.to_luma_alpha8(),
        }
    }

    // -----------------------------------------------------------------------
    // Accessor methods
    // -----------------------------------------------------------------------

    /// Return a mutable reference to an 8bit RGB image.
    pub fn as_mut_rgb8(&mut self) -> Option<&mut RgbImage> {
        match *self {
            DynamicImage::ImageRgb8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit RGBA image.
    pub fn as_mut_rgba8(&mut self) -> Option<&mut RgbaImage> {
        match *self {
            DynamicImage::ImageRgba8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit Grayscale image.
    pub fn as_mut_luma8(&mut self) -> Option<&mut GrayImage> {
        match *self {
            DynamicImage::ImageLuma8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit Grayscale image with an alpha channel.
    pub fn as_mut_luma_alpha8(&mut self) -> Option<&mut GrayAlphaImage> {
        match *self {
            DynamicImage::ImageLumaA8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a 16bit Grayscale image.
    pub fn as_mut_luma16(&mut self) -> Option<&mut ImageBuffer<Luma<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageI16(ref mut p)
            | DynamicImage::ImageI16L(ref mut p)
            | DynamicImage::ImageI16B(ref mut p)
            | DynamicImage::ImageI16N(ref mut p) => Some(p),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Color & dimensions
    // -----------------------------------------------------------------------

    /// Return this image's pixels as a native endian byte slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        match *self {
            DynamicImage::One(ref img)
            | DynamicImage::ImageLuma8(ref img)
            | DynamicImage::ImagePalette8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageLumaA8(ref img)
            | DynamicImage::ImageLaPremultiplied8(ref img)
            | DynamicImage::ImagePaletteAlpha8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageRgb8(ref img)
            | DynamicImage::ImageYCbCr8(ref img)
            | DynamicImage::ImageLab8(ref img)
            | DynamicImage::ImageHsv8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageRgba8(ref img)
            | DynamicImage::ImageRgbPremultiplied8(ref img)
            | DynamicImage::ImageRgbx8(ref img)
            | DynamicImage::ImageCmyk8(ref img)
            | DynamicImage::ImageI32(ref img)
            | DynamicImage::ImageF32(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageI16(ref img)
            | DynamicImage::ImageI16L(ref img)
            | DynamicImage::ImageI16B(ref img)
            | DynamicImage::ImageI16N(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageLumaA16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgb16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgba16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgb32F(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgba32F(ref img) => bytemuck::cast_slice(img.as_raw()),
        }
    }

    /// Return a mutable view of native image storage when it is byte-addressable.
    ///
    /// Compute backends use this only after taking ownership of an intermediate
    /// pipeline result.  Keeping the method limited to 8-bit rasters prevents a
    /// byte-oriented SIMD kernel from accidentally reinterpreting typed samples.
    pub(crate) fn as_bytes_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            DynamicImage::One(image)
            | DynamicImage::ImageLuma8(image)
            | DynamicImage::ImagePalette8(image) => Some(image.as_mut()),
            DynamicImage::ImageLumaA8(image)
            | DynamicImage::ImageLaPremultiplied8(image)
            | DynamicImage::ImagePaletteAlpha8(image) => Some(image.as_mut()),
            DynamicImage::ImageRgb8(image)
            | DynamicImage::ImageYCbCr8(image)
            | DynamicImage::ImageLab8(image)
            | DynamicImage::ImageHsv8(image) => Some(image.as_mut()),
            DynamicImage::ImageRgba8(image)
            | DynamicImage::ImageRgbPremultiplied8(image)
            | DynamicImage::ImageRgbx8(image)
            | DynamicImage::ImageCmyk8(image)
            | DynamicImage::ImageI32(image)
            | DynamicImage::ImageF32(image) => Some(image.as_mut()),
            _ => None,
        }
    }

    /// Return this image's color type.
    #[must_use]
    pub fn color(&self) -> ColorType {
        match *self {
            DynamicImage::One(_)
            | DynamicImage::ImageLuma8(_)
            | DynamicImage::ImagePalette8(_) => ColorType::L8,
            DynamicImage::ImageLumaA8(_)
            | DynamicImage::ImageLaPremultiplied8(_)
            | DynamicImage::ImagePaletteAlpha8(_) => ColorType::La8,
            DynamicImage::ImageRgb8(_)
            | DynamicImage::ImageYCbCr8(_)
            | DynamicImage::ImageLab8(_)
            | DynamicImage::ImageHsv8(_) => ColorType::Rgb8,
            DynamicImage::ImageRgba8(_)
            | DynamicImage::ImageRgbPremultiplied8(_)
            | DynamicImage::ImageRgbx8(_) => ColorType::Rgba8,
            DynamicImage::ImageCmyk8(_) => ColorType::Cmyk8,
            DynamicImage::ImageI32(_) => ColorType::L32I,
            DynamicImage::ImageF32(_) => ColorType::L32F,
            DynamicImage::ImageI16(_)
            | DynamicImage::ImageI16L(_)
            | DynamicImage::ImageI16B(_)
            | DynamicImage::ImageI16N(_) => ColorType::L16,
            DynamicImage::ImageLumaA16(_) => ColorType::La16,
            DynamicImage::ImageRgb16(_) => ColorType::Rgb16,
            DynamicImage::ImageRgba16(_) => ColorType::Rgba16,
            DynamicImage::ImageRgb32F(_) => ColorType::Rgb32F,
            DynamicImage::ImageRgba32F(_) => ColorType::Rgba32F,
        }
    }

    /// Returns the width of the underlying image.
    #[must_use]
    pub fn width(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.width() })
    }

    /// Returns the height of the underlying image.
    #[must_use]
    pub fn height(&self) -> u32 {
        dynamic_map!(*self, ref p, { p.height() })
    }

    /// Whether the image contains an alpha channel.
    #[must_use]
    pub fn has_alpha(&self) -> bool {
        self.mode().has_alpha()
    }

    /// Return a cropped copy of the image.
    #[must_use]
    pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        match self {
            DynamicImage::One(image) => {
                DynamicImage::One(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLuma8(image) => {
                DynamicImage::ImageLuma8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImagePalette8(image) => {
                DynamicImage::ImagePalette8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLumaA8(image) => {
                DynamicImage::ImageLumaA8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLaPremultiplied8(image) => DynamicImage::ImageLaPremultiplied8(
                crop_buffer(image, x, y, width, height),
            ),
            DynamicImage::ImagePaletteAlpha8(image) => DynamicImage::ImagePaletteAlpha8(
                crop_buffer(image, x, y, width, height),
            ),
            DynamicImage::ImageRgb8(image) => {
                DynamicImage::ImageRgb8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageYCbCr8(image) => {
                DynamicImage::ImageYCbCr8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLab8(image) => {
                DynamicImage::ImageLab8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageHsv8(image) => {
                DynamicImage::ImageHsv8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgba8(image) => {
                DynamicImage::ImageRgba8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgbPremultiplied8(image) => DynamicImage::ImageRgbPremultiplied8(
                crop_buffer(image, x, y, width, height),
            ),
            DynamicImage::ImageRgbx8(image) => {
                DynamicImage::ImageRgbx8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageCmyk8(image) => {
                DynamicImage::ImageCmyk8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageI32(image) => {
                DynamicImage::ImageI32(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageF32(image) => {
                DynamicImage::ImageF32(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageI16(image) => {
                DynamicImage::ImageI16(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageI16L(image) => {
                DynamicImage::ImageI16L(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageI16B(image) => {
                DynamicImage::ImageI16B(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageI16N(image) => {
                DynamicImage::ImageI16N(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLumaA16(image) => {
                DynamicImage::ImageLumaA16(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgb16(image) => {
                DynamicImage::ImageRgb16(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgba16(image) => {
                DynamicImage::ImageRgba16(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgb32F(image) => {
                DynamicImage::ImageRgb32F(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgba32F(image) => {
                DynamicImage::ImageRgba32F(crop_buffer(image, x, y, width, height))
            }
        }
    }

    /// Converts pixels to a codec image through the Pillow-owned I/O adapter.
    #[must_use]
    pub fn into_decoded(self) -> image_slash_star::DecodedImage {
        crate::image::codec_adapter::raster_to_decoded(self)
    }

    /// Adapts an unpacked codec image without changing its sample representation.
    #[must_use]
    pub fn from_decoded(d: &image_slash_star::DecodedImage) -> Option<DynamicImage> {
        crate::image::codec_adapter::decoded_pixels_to_raster(d)
    }

}

// -----------------------------------------------------------------------
// From implementations for DynamicImage -> specific buffer types
// -----------------------------------------------------------------------

impl From<DynamicImage> for RgbImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgb8()
    }
}

impl From<DynamicImage> for RgbaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_rgba8()
    }
}

impl From<DynamicImage> for GrayImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma8()
    }
}

impl From<DynamicImage> for GrayAlphaImage {
    fn from(value: DynamicImage) -> Self {
        value.into_luma_alpha8()
    }
}

impl From<RgbImage> for DynamicImage {
    fn from(value: RgbImage) -> Self {
        DynamicImage::ImageRgb8(value)
    }
}

impl From<RgbaImage> for DynamicImage {
    fn from(value: RgbaImage) -> Self {
        DynamicImage::ImageRgba8(value)
    }
}

impl From<GrayImage> for DynamicImage {
    fn from(value: GrayImage) -> Self {
        DynamicImage::ImageLuma8(value)
    }
}

impl From<GrayAlphaImage> for DynamicImage {
    fn from(value: GrayAlphaImage) -> Self {
        DynamicImage::ImageLumaA8(value)
    }
}

// Helper sealed trait for color conversion
trait IntoColor<Other> {
    fn to_color(&self) -> Other;
}

impl<O, S> IntoColor<O> for S
where
    O: Pixel + FromColor<S>,
{
    fn to_color(&self) -> O {
        // All raster layouts have one to four channels. Initialize exactly
        // that layout before the conversion fills every output channel.
        let channels = [O::Subpixel::DEFAULT_MIN_VALUE; 4];
        let mut pix = *O::from_slice(&channels[..usize::from(O::CHANNEL_COUNT)]);
        pix.copy_from_color(self);
        pix
    }
}

// -----------------------------------------------------------------------
// GenericImageView for DynamicImage
// -----------------------------------------------------------------------

impl GenericImageView for DynamicImage {
    type Pixel = Rgba<u8>;

    fn dimensions(&self) -> (u32, u32) {
        dynamic_map!(*self, ref p, p.dimensions())
    }

    fn get_pixel(&self, x: u32, y: u32) -> Rgba<u8> {
        dynamic_map!(*self, ref p, p.get_pixel(x, y).to_rgba().to_color())
    }
}

// -----------------------------------------------------------------------
// GenericImage for DynamicImage
// -----------------------------------------------------------------------

use super::traits::GenericImage as GenericImageTrait;

impl GenericImageTrait for DynamicImage {
    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        match self {
            DynamicImage::One(img)
            | DynamicImage::ImageLuma8(img)
            | DynamicImage::ImagePalette8(img) => {
                let p = pixel.to_luma();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageLumaA8(img)
            | DynamicImage::ImageLaPremultiplied8(img)
            | DynamicImage::ImagePaletteAlpha8(img) => {
                let p = pixel.to_luma_alpha();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageRgb8(img)
            | DynamicImage::ImageYCbCr8(img)
            | DynamicImage::ImageLab8(img)
            | DynamicImage::ImageHsv8(img) => {
                let p = pixel.to_rgb();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageRgba8(img)
            | DynamicImage::ImageRgbPremultiplied8(img)
            | DynamicImage::ImageRgbx8(img)
            | DynamicImage::ImageCmyk8(img)
            | DynamicImage::ImageI32(img)
            | DynamicImage::ImageF32(img) => {
                img.put_pixel(x, y, pixel);
            }
            DynamicImage::ImageI16(img)
            | DynamicImage::ImageI16L(img)
            | DynamicImage::ImageI16B(img)
            | DynamicImage::ImageI16N(img) => {
                let p = pixel.to_luma();
                let p16 = Luma([u16::from(p[0]).saturating_mul(257)]);
                img.put_pixel(x, y, p16);
            }
            DynamicImage::ImageLumaA16(img) => {
                let p = pixel.to_luma_alpha();
                let pa16 = LumaA([
                    u16::from(p[0]).saturating_mul(257),
                    u16::from(p[1]).saturating_mul(257),
                ]);
                img.put_pixel(x, y, pa16);
            }
            DynamicImage::ImageRgb16(img) => {
                let p = pixel.to_rgb();
                let pr16 = Rgb([
                    u16::from(p[0]).saturating_mul(257),
                    u16::from(p[1]).saturating_mul(257),
                    u16::from(p[2]).saturating_mul(257),
                ]);
                img.put_pixel(x, y, pr16);
            }
            DynamicImage::ImageRgba16(img) => {
                let p16 = Rgba([
                    u16::from(pixel[0]).saturating_mul(257),
                    u16::from(pixel[1]).saturating_mul(257),
                    u16::from(pixel[2]).saturating_mul(257),
                    u16::from(pixel[3]).saturating_mul(257),
                ]);
                img.put_pixel(x, y, p16);
            }
            DynamicImage::ImageRgb32F(img) => {
                let p = pixel.to_rgb();
                let pf = Rgb([
                    p[0] as f32 / 255.0,
                    p[1] as f32 / 255.0,
                    p[2] as f32 / 255.0,
                ]);
                img.put_pixel(x, y, pf);
            }
            DynamicImage::ImageRgba32F(img) => {
                let pf = Rgba([
                    pixel[0] as f32 / 255.0,
                    pixel[1] as f32 / 255.0,
                    pixel[2] as f32 / 255.0,
                    pixel[3] as f32 / 255.0,
                ]);
                img.put_pixel(x, y, pf);
            }
        }
    }
}
