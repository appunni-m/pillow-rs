//! Dynamic image type — an enum over supported image buffer types.
//!
//! Matches the `image` crate's `DynamicImage` API.

use super::buffer::{
    ConvertBuffer, GrayAlphaImage, GrayImage, ImageBuffer, Rgb32FImage, RgbImage, Rgba32FImage,
    RgbaImage,
};
use super::color::{self, ColorType, Luma, LumaA, Rgb, Rgba};
use super::traits::{GenericImageView, Pixel, Primitive};
use image_slash_star::DecodedImage;

use crate::raster::color::FromColor;

mod transform;

macro_rules! dynamic_map(
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

#[inline]
fn luma16_to_u8(sample: u16) -> u8 {
    sample.min(u16::from(u8::MAX)) as u8
}

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
        match self {
            Self::ImageLuma8(p) => Self::ImageLuma8(p.clone()),
            Self::ImageLumaA8(p) => Self::ImageLumaA8(p.clone()),
            Self::ImageRgb8(p) => Self::ImageRgb8(p.clone()),
            Self::ImageRgba8(p) => Self::ImageRgba8(p.clone()),
            Self::ImageLuma16(p) => Self::ImageLuma16(p.clone()),
            Self::ImageLumaA16(p) => Self::ImageLumaA16(p.clone()),
            Self::ImageRgb16(p) => Self::ImageRgb16(p.clone()),
            Self::ImageRgba16(p) => Self::ImageRgba16(p.clone()),
            Self::ImageRgb32F(p) => Self::ImageRgb32F(p.clone()),
            Self::ImageRgba32F(p) => Self::ImageRgba32F(p.clone()),
        }
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

#[inline]
fn crop_buffer<P: Pixel>(
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
            DynamicImage::ImageLuma16(image) => {
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
            DynamicImage::ImageLuma16(image) => {
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
            DynamicImage::ImageLuma8(x) => x.clone(),
            DynamicImage::ImageLuma16(image) => {
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
            DynamicImage::ImageLumaA8(x) => x.clone(),
            DynamicImage::ImageLuma16(image) => {
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
            DynamicImage::ImageLuma16(x) => x.clone(),
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
        match self {
            DynamicImage::ImageLuma8(img) => img.convert(),
            DynamicImage::ImageLumaA8(img) => img.convert(),
            DynamicImage::ImageRgb8(img) => img.convert(),
            DynamicImage::ImageRgba8(img) => img.convert(),
            DynamicImage::ImageLuma16(img) => img.convert(),
            DynamicImage::ImageLumaA16(img) => img.convert(),
            DynamicImage::ImageRgb16(img) => img.convert(),
            DynamicImage::ImageRgba16(img) => img.convert(),
            DynamicImage::ImageRgb32F(img) => img.convert(),
            DynamicImage::ImageRgba32F(img) => img.convert(),
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
            DynamicImage::ImageLuma16(ref mut p) => Some(p),
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

    /// Return a mutable view of native image storage when it is byte-addressable.
    ///
    /// Compute backends use this only after taking ownership of an intermediate
    /// pipeline result.  Keeping the method limited to 8-bit rasters prevents a
    /// byte-oriented SIMD kernel from accidentally reinterpreting typed samples.
    pub(crate) fn as_bytes_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            DynamicImage::ImageLuma8(image) => Some(image.as_mut()),
            DynamicImage::ImageLumaA8(image) => Some(image.as_mut()),
            DynamicImage::ImageRgb8(image) => Some(image.as_mut()),
            DynamicImage::ImageRgba8(image) => Some(image.as_mut()),
            _ => None,
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

    /// Return a cropped copy of the image.
    #[must_use]
    pub fn crop_imm(&self, x: u32, y: u32, width: u32, height: u32) -> DynamicImage {
        match self {
            DynamicImage::ImageLuma8(image) => {
                DynamicImage::ImageLuma8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLumaA8(image) => {
                DynamicImage::ImageLumaA8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgb8(image) => {
                DynamicImage::ImageRgb8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageRgba8(image) => {
                DynamicImage::ImageRgba8(crop_buffer(image, x, y, width, height))
            }
            DynamicImage::ImageLuma16(image) => {
                DynamicImage::ImageLuma16(crop_buffer(image, x, y, width, height))
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

    /// Convert this DynamicImage into a DecodedImage (flat pixel buffer + ColorType).
    #[must_use]
    pub fn into_decoded(self) -> DecodedImage {
        DecodedImage::new(
            self.width(),
            self.height(),
            self.as_bytes().to_vec(),
            self.color(),
        )
    }

    /// Create a DynamicImage from a DecodedImage reference.
    #[must_use]
    pub fn from_decoded(d: &DecodedImage) -> Option<DynamicImage> {
        if d.mode != d.color.into() || d.palette.is_some() {
            return None;
        }
        let img = match d.color {
            ColorType::L8 => {
                DynamicImage::ImageLuma8(GrayImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            ColorType::La8 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
                d.width,
                d.height,
                d.pixels.clone(),
            )?),
            ColorType::Rgb8 => {
                DynamicImage::ImageRgb8(RgbImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            ColorType::Rgba8 => {
                DynamicImage::ImageRgba8(RgbaImage::from_raw(d.width, d.height, d.pixels.clone())?)
            }
            ColorType::Cmyk8 => return None,
            ColorType::L16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageLuma16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            ColorType::La16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageLumaA16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            ColorType::Rgb16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageRgb16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            ColorType::Rgba16 => {
                let u16_data: Vec<u16> = d
                    .pixels
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                DynamicImage::ImageRgba16(ImageBuffer::from_raw(d.width, d.height, u16_data)?)
            }
            ColorType::Rgb32F => {
                let f32_data: Vec<f32> = d
                    .pixels
                    .chunks_exact(4)
                    .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                DynamicImage::ImageRgb32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
            }
            ColorType::Rgba32F => {
                let f32_data: Vec<f32> = d
                    .pixels
                    .chunks_exact(4)
                    .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                DynamicImage::ImageRgba32F(ImageBuffer::from_raw(d.width, d.height, f32_data)?)
            }
            ColorType::L32F | ColorType::L32I => return None,
            _ => return None,
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
