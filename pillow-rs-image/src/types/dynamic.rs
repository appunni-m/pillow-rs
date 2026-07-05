//! Dynamic image type — an enum over supported image buffer types.
//!
//! Matches the `image` crate's `DynamicImage` API.

use super::buffer::{
    ConvertBuffer, GrayAlphaImage, GrayImage, ImageBuffer, Rgb32FImage, RgbImage, Rgba32FImage,
    RgbaImage,
};
use super::color::{self, ColorType, Luma, LumaA, Rgb, Rgba};
use super::traits::{GenericImageView, Pixel, Primitive};
use crate::types::DecodedImage;
use crate::types::color::FromColor;

macro_rules! dynamic_map(
    ($dynimage: expr, $image: pat => $action: expr) => ({
        use DynamicImage::*;
        match $dynimage {
            ImageLuma8($image) => ImageLuma8($action),
            ImageLumaA8($image) => ImageLumaA8($action),
            ImageRgb8($image) => ImageRgb8($action),
            ImageRgba8($image) => ImageRgba8($action),
            ImageLuma16($image) => ImageLuma16($action),
            ImageLumaA16($image) => ImageLumaA16($action),
            ImageRgb16($image) => ImageRgb16($action),
            ImageRgba16($image) => ImageRgba16($action),
            ImageRgb32F($image) => ImageRgb32F($action),
            ImageRgba32F($image) => ImageRgba32F($action),
        }
    });

    ($dynimage: expr, $image:pat_param, $action: expr) => (
        match $dynimage {
            DynamicImage::ImageLuma8($image) => $action,
            DynamicImage::ImageLumaA8($image) => $action,
            DynamicImage::ImageRgb8($image) => $action,
            DynamicImage::ImageRgba8($image) => $action,
            DynamicImage::ImageLuma16($image) => $action,
            DynamicImage::ImageLumaA16($image) => $action,
            DynamicImage::ImageRgb16($image) => $action,
            DynamicImage::ImageRgba16($image) => $action,
            DynamicImage::ImageRgb32F($image) => $action,
            DynamicImage::ImageRgba32F($image) => $action,
        }
    );
);

/// A Dynamic Image
///
/// This represents a matrix of pixels which are convertible from and to an RGBA
/// representation.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum DynamicImage {
    /// Each pixel in this image is 8-bit Luma
    ImageLuma8(GrayImage),

    /// Each pixel in this image is 8-bit Luma with alpha
    ImageLumaA8(GrayAlphaImage),

    /// Each pixel in this image is 8-bit Rgb
    ImageRgb8(RgbImage),

    /// Each pixel in this image is 8-bit Rgb with alpha
    ImageRgba8(RgbaImage),

    /// Each pixel in this image is 16-bit Luma
    ImageLuma16(ImageBuffer<Luma<u16>, Vec<u16>>),

    /// Each pixel in this image is 16-bit Luma with alpha
    ImageLumaA16(ImageBuffer<LumaA<u16>, Vec<u16>>),

    /// Each pixel in this image is 16-bit Rgb
    ImageRgb16(ImageBuffer<Rgb<u16>, Vec<u16>>),

    /// Each pixel in this image is 16-bit Rgb with alpha
    ImageRgba16(ImageBuffer<Rgba<u16>, Vec<u16>>),

    /// Each pixel in this image is 32-bit float Rgb
    ImageRgb32F(Rgb32FImage),

    /// Each pixel in this image is 32-bit float Rgb with alpha
    ImageRgba32F(Rgba32FImage),
}

impl Clone for DynamicImage {
    fn clone(&self) -> Self {
        dynamic_map!(*self, ref p => p.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Self::ImageLuma8(p1), Self::ImageLuma8(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA8(p1), Self::ImageLumaA8(p2)) => p1.clone_from(p2),
            (Self::ImageRgb8(p1), Self::ImageRgb8(p2)) => p1.clone_from(p2),
            (Self::ImageRgba8(p1), Self::ImageRgba8(p2)) => p1.clone_from(p2),
            (Self::ImageLuma16(p1), Self::ImageLuma16(p2)) => p1.clone_from(p2),
            (Self::ImageLumaA16(p1), Self::ImageLumaA16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb16(p1), Self::ImageRgb16(p2)) => p1.clone_from(p2),
            (Self::ImageRgba16(p1), Self::ImageRgba16(p2)) => p1.clone_from(p2),
            (Self::ImageRgb32F(p1), Self::ImageRgb32F(p2)) => p1.clone_from(p2),
            (Self::ImageRgba32F(p1), Self::ImageRgba32F(p2)) => p1.clone_from(p2),
            (this, source) => *this = source.clone(),
        }
    }
}

impl DynamicImage {
    /// Creates a dynamic image backed by a buffer depending on the color type given.
    #[must_use]
    pub fn new(w: u32, h: u32, color: ColorType) -> DynamicImage {
        use ColorType::*;
        match color {
            L8 => Self::new_luma8(w, h),
            La8 => Self::new_luma_a8(w, h),
            Rgb8 => Self::new_rgb8(w, h),
            Rgba8 => Self::new_rgba8(w, h),
            L16 => Self::new_luma16(w, h),
            La16 => Self::new_luma_a16(w, h),
            Rgb16 => Self::new_rgb16(w, h),
            Rgba16 => Self::new_rgba16(w, h),
            Rgb32F => Self::new_rgb32f(w, h),
            Rgba32F => Self::new_rgba32f(w, h),
        }
    }

    /// Creates a dynamic image backed by a buffer of gray pixels.
    #[must_use]
    pub fn new_luma8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray pixels with transparency.
    #[must_use]
    pub fn new_luma_a8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels.
    #[must_use]
    pub fn new_rgb8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels.
    #[must_use]
    pub fn new_rgba8(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray pixels (16-bit).
    #[must_use]
    pub fn new_luma16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLuma16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of gray pixels with transparency (16-bit).
    #[must_use]
    pub fn new_luma_a16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageLumaA16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels (16-bit).
    #[must_use]
    pub fn new_rgb16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels (16-bit).
    #[must_use]
    pub fn new_rgba16(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba16(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGB pixels (f32).
    #[must_use]
    pub fn new_rgb32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb32F(ImageBuffer::new(w, h))
    }

    /// Creates a dynamic image backed by a buffer of RGBA pixels (f32).
    #[must_use]
    pub fn new_rgba32f(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba32F(ImageBuffer::new(w, h))
    }

    /// Generic conversion to any pixel type using the ConvertBuffer trait.
    #[inline]
    #[must_use]
    pub fn to<
        T: Pixel
            + FromColor<color::Rgb<u8>>
            + FromColor<color::Rgb<f32>>
            + FromColor<color::Rgba<u8>>
            + FromColor<color::Rgba<u16>>
            + FromColor<color::Rgba<f32>>
            + FromColor<color::Rgb<u16>>
            + FromColor<Luma<u8>>
            + FromColor<Luma<u16>>
            + FromColor<LumaA<u16>>
            + FromColor<LumaA<u8>>,
    >(
        &self,
    ) -> ImageBuffer<T, Vec<T::Subpixel>> {
        dynamic_map!(*self, ref p, p.convert())
    }

    /// Returns a copy of this image as an RGB image.
    #[must_use]
    pub fn to_rgb8(&self) -> RgbImage {
        match self {
            DynamicImage::ImageRgb8(x) => x.clone(),
            _x => self.to_generic::<Rgb<u8>, Vec<u8>>(),
        }
    }

    /// Returns a copy of this image as an RGBA image.
    #[must_use]
    pub fn to_rgba8(&self) -> RgbaImage {
        match self {
            DynamicImage::ImageRgba8(x) => x.clone(),
            _x => self.to_generic::<Rgba<u8>, Vec<u8>>(),
        }
    }

    /// Returns a copy of this image as a Luma image.
    #[must_use]
    pub fn to_luma8(&self) -> GrayImage {
        match self {
            DynamicImage::ImageLuma8(x) => x.clone(),
            _x => self.to_generic::<Luma<u8>, Vec<u8>>(),
        }
    }

    /// Returns a copy of this image as a LumaA image.
    #[must_use]
    pub fn to_luma_alpha8(&self) -> GrayAlphaImage {
        match self {
            DynamicImage::ImageLumaA8(x) => x.clone(),
            _x => self.to_generic::<LumaA<u8>, Vec<u8>>(),
        }
    }

    /// Returns a copy of this image as an RGB image (16-bit).
    #[must_use]
    pub fn to_rgb16(&self) -> ImageBuffer<Rgb<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageRgb16(x) => x.clone(),
            _x => self.to_generic::<Rgb<u16>, Vec<u16>>(),
        }
    }

    /// Returns a copy of this image as an RGBA image (16-bit).
    #[must_use]
    pub fn to_rgba16(&self) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageRgba16(x) => x.clone(),
            _x => self.to_generic::<Rgba<u16>, Vec<u16>>(),
        }
    }

    /// Returns a copy of this image as a Luma image (16-bit).
    #[must_use]
    pub fn to_luma16(&self) -> ImageBuffer<Luma<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageLuma16(x) => x.clone(),
            _x => self.to_generic::<Luma<u16>, Vec<u16>>(),
        }
    }

    /// Returns a copy of this image as a LumaA image (16-bit).
    #[must_use]
    pub fn to_luma_alpha16(&self) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageLumaA16(x) => x.clone(),
            _x => self.to_generic::<LumaA<u16>, Vec<u16>>(),
        }
    }

    /// Returns a copy of this image as an RGB image (f32).
    #[must_use]
    pub fn to_rgb32f(&self) -> Rgb32FImage {
        match self {
            DynamicImage::ImageRgb32F(x) => x.clone(),
            _x => self.to_generic::<Rgb<f32>, Vec<f32>>(),
        }
    }

    /// Returns a copy of this image as an RGBA image (f32).
    #[must_use]
    pub fn to_rgba32f(&self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x.clone(),
            _x => self.to_generic::<Rgba<f32>, Vec<f32>>(),
        }
    }

    /// Internal helper: convert to a generic pixel type using pixel conversion.
    fn to_generic<Px: Pixel, C>(&self) -> ImageBuffer<Px, Vec<<Px as Pixel>::Subpixel>>
    where
        Px: FromColor<color::Rgb<u8>>
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
        use ColorType::*;
        match self.color() {
            L8 => {
                let Some(img) = self.as_luma8() else {
                    panic!("BUG: as_luma8 returned None for L8 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            La8 => {
                let Some(img) = self.as_luma_alpha8() else {
                    panic!("BUG: as_luma_alpha8 returned None for La8 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgb8 => {
                let Some(img) = self.as_rgb8() else {
                    panic!("BUG: as_rgb8 returned None for Rgb8 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgba8 => {
                let Some(img) = self.as_rgba8() else {
                    panic!("BUG: as_rgba8 returned None for Rgba8 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            L16 => {
                let Some(img) = self.as_luma16() else {
                    panic!("BUG: as_luma16 returned None for L16 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            La16 => {
                let Some(img) = self.as_luma_alpha16() else {
                    panic!("BUG: as_luma_alpha16 returned None for La16 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgb16 => {
                let Some(img) = self.as_rgb16() else {
                    panic!("BUG: as_rgb16 returned None for Rgb16 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgba16 => {
                let Some(img) = self.as_rgba16() else {
                    panic!("BUG: as_rgba16 returned None for Rgba16 image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgb32F => {
                let Some(img) = self.as_rgb32f() else {
                    panic!("BUG: as_rgb32f returned None for Rgb32F image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
            Rgba32F => {
                let Some(img) = self.as_rgba32f() else {
                    panic!("BUG: as_rgba32f returned None for Rgba32F image");
                };
                let buf: ImageBuffer<Px, Vec<Px::Subpixel>> = img.convert();
                buf
            }
        }
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

    /// Consume the image and returns a RGB image (16-bit).
    #[must_use]
    pub fn into_rgb16(self) -> ImageBuffer<Rgb<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageRgb16(x) => x,
            x => x.to_rgb16(),
        }
    }

    /// Consume the image and returns a RGBA image (16-bit).
    #[must_use]
    pub fn into_rgba16(self) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageRgba16(x) => x,
            x => x.to_rgba16(),
        }
    }

    /// Consume the image and returns a Luma image (16-bit).
    #[must_use]
    pub fn into_luma16(self) -> ImageBuffer<Luma<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageLuma16(x) => x,
            x => x.to_luma16(),
        }
    }

    /// Consume the image and returns a LumaA image (16-bit).
    #[must_use]
    pub fn into_luma_alpha16(self) -> ImageBuffer<LumaA<u16>, Vec<u16>> {
        match self {
            DynamicImage::ImageLumaA16(x) => x,
            x => x.to_luma_alpha16(),
        }
    }

    /// Consume the image and returns a RGB image (f32).
    #[must_use]
    pub fn into_rgb32f(self) -> Rgb32FImage {
        match self {
            DynamicImage::ImageRgb32F(x) => x,
            x => x.to_rgb32f(),
        }
    }

    /// Consume the image and returns a RGBA image (f32).
    #[must_use]
    pub fn into_rgba32f(self) -> Rgba32FImage {
        match self {
            DynamicImage::ImageRgba32F(x) => x,
            x => x.to_rgba32f(),
        }
    }

    // -----------------------------------------------------------------------
    // Accessor methods
    // -----------------------------------------------------------------------

    /// Return a reference to an 8bit RGB image.
    #[must_use]
    pub fn as_rgb8(&self) -> Option<&RgbImage> {
        match *self {
            DynamicImage::ImageRgb8(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to an 8bit RGB image.
    pub fn as_mut_rgb8(&mut self) -> Option<&mut RgbImage> {
        match *self {
            DynamicImage::ImageRgb8(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to an 8bit RGBA image.
    #[must_use]
    pub fn as_rgba8(&self) -> Option<&RgbaImage> {
        match *self {
            DynamicImage::ImageRgba8(ref p) => Some(p),
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

    /// Return a reference to an 8bit Grayscale image.
    #[must_use]
    pub fn as_luma8(&self) -> Option<&GrayImage> {
        match *self {
            DynamicImage::ImageLuma8(ref p) => Some(p),
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

    /// Return a reference to an 8bit Grayscale image with an alpha channel.
    #[must_use]
    pub fn as_luma_alpha8(&self) -> Option<&GrayAlphaImage> {
        match *self {
            DynamicImage::ImageLumaA8(ref p) => Some(p),
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

    /// Return a reference to a 16bit RGB image.
    #[must_use]
    pub fn as_rgb16(&self) -> Option<&ImageBuffer<Rgb<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageRgb16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a 16bit RGB image.
    pub fn as_mut_rgb16(&mut self) -> Option<&mut ImageBuffer<Rgb<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageRgb16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to a 16bit RGBA image.
    #[must_use]
    pub fn as_rgba16(&self) -> Option<&ImageBuffer<Rgba<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageRgba16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a 16bit RGBA image.
    pub fn as_mut_rgba16(&mut self) -> Option<&mut ImageBuffer<Rgba<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageRgba16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to a 16bit Grayscale image.
    #[must_use]
    pub fn as_luma16(&self) -> Option<&ImageBuffer<Luma<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageLuma16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a 16bit Grayscale image.
    pub fn as_mut_luma16(&mut self) -> Option<&mut ImageBuffer<Luma<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageLuma16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to a 16bit Grayscale image with an alpha channel.
    #[must_use]
    pub fn as_luma_alpha16(&self) -> Option<&ImageBuffer<LumaA<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageLumaA16(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a 16bit Grayscale image with an alpha channel.
    pub fn as_mut_luma_alpha16(&mut self) -> Option<&mut ImageBuffer<LumaA<u16>, Vec<u16>>> {
        match *self {
            DynamicImage::ImageLumaA16(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to a f32 RGB image.
    #[must_use]
    pub fn as_rgb32f(&self) -> Option<&Rgb32FImage> {
        match *self {
            DynamicImage::ImageRgb32F(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a f32 RGB image.
    pub fn as_mut_rgb32f(&mut self) -> Option<&mut Rgb32FImage> {
        match *self {
            DynamicImage::ImageRgb32F(ref mut p) => Some(p),
            _ => None,
        }
    }

    /// Return a reference to a f32 RGBA image.
    #[must_use]
    pub fn as_rgba32f(&self) -> Option<&Rgba32FImage> {
        match *self {
            DynamicImage::ImageRgba32F(ref p) => Some(p),
            _ => None,
        }
    }

    /// Return a mutable reference to a f32 RGBA image.
    pub fn as_mut_rgba32f(&mut self) -> Option<&mut Rgba32FImage> {
        match *self {
            DynamicImage::ImageRgba32F(ref mut p) => Some(p),
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
            DynamicImage::ImageLuma8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageLumaA8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageRgb8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageRgba8(ref img) => {
                let raw: &[u8] = img.as_raw();
                raw
            }
            DynamicImage::ImageLuma16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageLumaA16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgb16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgba16(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgb32F(ref img) => bytemuck::cast_slice(img.as_raw()),
            DynamicImage::ImageRgba32F(ref img) => bytemuck::cast_slice(img.as_raw()),
        }
    }

    /// Return this image's color type.
    #[must_use]
    pub fn color(&self) -> ColorType {
        match *self {
            DynamicImage::ImageLuma8(_) => ColorType::L8,
            DynamicImage::ImageLumaA8(_) => ColorType::La8,
            DynamicImage::ImageRgb8(_) => ColorType::Rgb8,
            DynamicImage::ImageRgba8(_) => ColorType::Rgba8,
            DynamicImage::ImageLuma16(_) => ColorType::L16,
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
        self.color().has_alpha()
    }

    /// Flip the image horizontally (mirror).
    #[must_use]
    pub fn fliph(&self) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, y)
                }))
            }
            DynamicImage::ImageLumaA8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, y)
                }))
            }
            DynamicImage::ImageRgb8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| *p.get_pixel(w - 1 - x, y)))
            }
            DynamicImage::ImageRgba8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, y)
                }))
            }
            _ => {
                let rgba = self.to_rgba8();
                let (w, h) = rgba.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *rgba.get_pixel(w - 1 - x, y)
                }))
            }
        }
    }

    /// Flip the image vertically.
    #[must_use]
    pub fn flipv(&self) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(x, h - 1 - y)
                }))
            }
            DynamicImage::ImageLumaA8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(x, h - 1 - y)
                }))
            }
            DynamicImage::ImageRgb8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| *p.get_pixel(x, h - 1 - y)))
            }
            DynamicImage::ImageRgba8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(x, h - 1 - y)
                }))
            }
            _ => {
                let rgba = self.to_rgba8();
                let (w, h) = rgba.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *rgba.get_pixel(x, h - 1 - y)
                }))
            }
        }
    }

    /// Rotate the image 90 degrees clockwise.
    #[must_use]
    pub fn rotate90(&self) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLuma8(GrayImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(y, h - 1 - x)
                }))
            }
            DynamicImage::ImageLumaA8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(y, h - 1 - x)
                }))
            }
            DynamicImage::ImageRgb8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgb8(RgbImage::from_fn(h, w, |x, y| *p.get_pixel(y, h - 1 - x)))
            }
            DynamicImage::ImageRgba8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(y, h - 1 - x)
                }))
            }
            _ => {
                let rgba = self.to_rgba8();
                let (w, h) = rgba.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(h, w, |x, y| {
                    *rgba.get_pixel(y, h - 1 - x)
                }))
            }
        }
    }

    /// Rotate the image 180 degrees.
    #[must_use]
    pub fn rotate180(&self) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, h - 1 - y)
                }))
            }
            DynamicImage::ImageLumaA8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, h - 1 - y)
                }))
            }
            DynamicImage::ImageRgb8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, h - 1 - y)
                }))
            }
            DynamicImage::ImageRgba8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *p.get_pixel(w - 1 - x, h - 1 - y)
                }))
            }
            _ => {
                let rgba = self.to_rgba8();
                let (w, h) = rgba.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
                    *rgba.get_pixel(w - 1 - x, h - 1 - y)
                }))
            }
        }
    }

    /// Rotate the image 270 degrees clockwise (90 degrees counter-clockwise).
    #[must_use]
    pub fn rotate270(&self) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLuma8(GrayImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(w - 1 - y, x)
                }))
            }
            DynamicImage::ImageLumaA8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(w - 1 - y, x)
                }))
            }
            DynamicImage::ImageRgb8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgb8(RgbImage::from_fn(h, w, |x, y| *p.get_pixel(w - 1 - y, x)))
            }
            DynamicImage::ImageRgba8(p) => {
                let (w, h) = p.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(h, w, |x, y| {
                    *p.get_pixel(w - 1 - y, x)
                }))
            }
            _ => {
                let rgba = self.to_rgba8();
                let (w, h) = rgba.dimensions();
                DynamicImage::ImageRgba8(RgbaImage::from_fn(h, w, |x, y| {
                    *rgba.get_pixel(w - 1 - y, x)
                }))
            }
        }
    }

    /// Return a cropped copy of the image.
    #[must_use]
    pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(p) => {
                let mut buf = GrayImage::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageLuma8(buf)
            }
            DynamicImage::ImageLumaA8(p) => {
                let mut buf = GrayAlphaImage::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageLumaA8(buf)
            }
            DynamicImage::ImageRgb8(p) => {
                let mut buf = RgbImage::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgb8(buf)
            }
            DynamicImage::ImageRgba8(p) => {
                let mut buf = RgbaImage::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgba8(buf)
            }
            DynamicImage::ImageLuma16(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageLuma16(buf)
            }
            DynamicImage::ImageLumaA16(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageLumaA16(buf)
            }
            DynamicImage::ImageRgb16(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgb16(buf)
            }
            DynamicImage::ImageRgba16(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgba16(buf)
            }
            DynamicImage::ImageRgb32F(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgb32F(buf)
            }
            DynamicImage::ImageRgba32F(p) => {
                let mut buf = ImageBuffer::new(width, height);
                for dy in 0..height {
                    for dx in 0..width {
                        let px = *p.get_pixel(x + dx, y + dy);
                        buf.put_pixel(dx, dy, px);
                    }
                }
                DynamicImage::ImageRgba32F(buf)
            }
        }
    }

    /// Convert this DynamicImage into a DecodedImage (flat pixel buffer + ColorType).
    #[must_use]
    pub fn into_decoded(self) -> DecodedImage {
        DecodedImage {
            width: self.width(),
            height: self.height(),
            pixels: self.as_bytes().to_vec(),
            color: self.color(),
        }
    }

    /// Create a DynamicImage from a DecodedImage reference.
    #[must_use]
    pub fn from_decoded(d: &DecodedImage) -> Option<DynamicImage> {
        use ColorType::*;
        let img = match d.color {
            L8 => {
                DynamicImage::ImageLuma8(GrayImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            La8 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
                d.width,
                d.height,
                d.pixels.clone(),
            )?),
            Rgb8 => {
                DynamicImage::ImageRgb8(RgbImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            Rgba8 => {
                DynamicImage::ImageRgba8(RgbaImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            L16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageLuma16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            La16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageLumaA16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            Rgb16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageRgb16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            Rgba16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageRgba16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            Rgb32F => {
                let f32_data: Vec<f32> = d
                    .pixels
                    .chunks_exact(4)
                    .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                DynamicImage::ImageRgb32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
            }
            Rgba32F => {
                let f32_data: Vec<f32> = d
                    .pixels
                    .chunks_exact(4)
                    .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                DynamicImage::ImageRgba32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
            }
        };
        Some(img)
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
    fn into_color(&self) -> Other;
}

impl<O, S> IntoColor<O> for S
where
    O: Pixel + FromColor<S>,
{
    #[allow(deprecated)]
    fn into_color(&self) -> O {
        let mut pix = O::from_channels(
            O::Subpixel::DEFAULT_MIN_VALUE,
            O::Subpixel::DEFAULT_MIN_VALUE,
            O::Subpixel::DEFAULT_MIN_VALUE,
            O::Subpixel::DEFAULT_MIN_VALUE,
        );
        pix.from_color(self);
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
        dynamic_map!(*self, ref p, p.get_pixel(x, y).to_rgba().into_color())
    }
}

// -----------------------------------------------------------------------
// GenericImage for DynamicImage
// -----------------------------------------------------------------------

use super::traits::GenericImage as GenericImageTrait;

impl GenericImageTrait for DynamicImage {
    #[allow(deprecated)]
    fn get_pixel_mut(&mut self, _x: u32, _y: u32) -> &mut Self::Pixel {
        panic!("get_pixel_mut not supported on DynamicImage")
    }

    fn put_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        match self {
            DynamicImage::ImageLuma8(img) => {
                let p = pixel.to_luma();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageLumaA8(img) => {
                let p = pixel.to_luma_alpha();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageRgb8(img) => {
                let p = pixel.to_rgb();
                img.put_pixel(x, y, p);
            }
            DynamicImage::ImageRgba8(img) => {
                img.put_pixel(x, y, pixel);
            }
            DynamicImage::ImageLuma16(img) => {
                let p = pixel.to_luma();
                let p16 = Luma([(p[0] as u16 * 257)]);
                img.put_pixel(x, y, p16);
            }
            DynamicImage::ImageLumaA16(img) => {
                let p = pixel.to_luma_alpha();
                let pa16 = LumaA([(p[0] as u16 * 257), (p[1] as u16 * 257)]);
                img.put_pixel(x, y, pa16);
            }
            DynamicImage::ImageRgb16(img) => {
                let p = pixel.to_rgb();
                let pr16 = Rgb([
                    (p[0] as u16 * 257),
                    (p[1] as u16 * 257),
                    (p[2] as u16 * 257),
                ]);
                img.put_pixel(x, y, pr16);
            }
            DynamicImage::ImageRgba16(img) => {
                let p16 = Rgba([
                    (pixel[0] as u16 * 257),
                    (pixel[1] as u16 * 257),
                    (pixel[2] as u16 * 257),
                    (pixel[3] as u16 * 257),
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

    #[allow(deprecated)]
    fn blend_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
        // Simple alpha blend using the current pixel
        let current = self.get_pixel(x, y);
        let a = pixel[3] as f32 / 255.0;
        let inv_a = 1.0 - a;
        let blended = Rgba([
            (pixel[0] as f32 * a + current[0] as f32 * inv_a) as u8,
            (pixel[1] as f32 * a + current[1] as f32 * inv_a) as u8,
            (pixel[2] as f32 * a + current[2] as f32 * inv_a) as u8,
            (pixel[3] as f32 * a + current[3] as f32 * inv_a) as u8,
        ]);
        self.put_pixel(x, y, blended);
    }
}
