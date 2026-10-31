//! Monomorphized native-pixel conversions for internal high-depth modes.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, FromColor, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, Luma,
    LumaA, Mode, Pixel, Primitive, Rgb, RgbImage, Rgba, RgbaImage,
};
use crate::compute::pool_cpu::ops::grayscale::helpers::luma::ToGrayByte;

pub(crate) trait HighDepthPixel: Pixel
where
    Self::Subpixel: Primitive,
{
    fn luma8(&self) -> Luma<u8>;
    fn luma_alpha8(&self) -> LumaA<u8>;
    fn rgb8(&self) -> Rgb<u8>;
    fn rgba8(&self) -> Rgba<u8>;
}

impl<P> HighDepthPixel for P
where
    P: Pixel,
    P::Subpixel: Primitive,
    Luma<u8>: FromColor<P>,
    LumaA<u8>: FromColor<P>,
    Rgb<u8>: FromColor<P>,
    Rgba<u8>: FromColor<P>,
{
    #[inline(always)]
    fn luma8(&self) -> Luma<u8> {
        let mut output = Luma([0]);
        output.copy_from_color(self);
        output
    }

    #[inline(always)]
    fn luma_alpha8(&self) -> LumaA<u8> {
        let mut output = LumaA([0; 2]);
        output.copy_from_color(self);
        output
    }

    #[inline(always)]
    fn rgb8(&self) -> Rgb<u8> {
        let mut output = Rgb([0; 3]);
        output.copy_from_color(self);
        output
    }

    #[inline(always)]
    fn rgba8(&self) -> Rgba<u8> {
        let mut output = Rgba([0; 4]);
        output.copy_from_color(self);
        output
    }
}

pub(crate) fn execute<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target: Mode,
) -> Result<DynamicImage, PilError>
where
    P: HighDepthPixel,
    P::Subpixel: Primitive,
{
    match target {
        Mode::ImageLuma8 => Ok(DynamicImage::ImageLuma8(luma(source))),
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(luma_alpha(source))),
        Mode::ImageRgb8 => Ok(DynamicImage::ImageRgb8(rgb(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

// NO_PILLOW_SUPPORT: typed I;16 targets from internal high-depth modes are
// core extensions; convert native samples to the same luma bytes directly.
pub(crate) fn execute_i16_from_gray<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target: Mode,
) -> Result<DynamicImage, PilError>
where
    P: Pixel,
    P::Subpixel: ToGrayByte,
{
    super::luma16::from_luma8_samples(
        source.width(),
        source.height(),
        target,
        source
            .pixels()
            .map(|pixel| pixel.channels()[0].to_gray_byte()),
    )
}

// NO_PILLOW_SUPPORT: typed I;16 targets from internal high-depth modes are
// core extensions; match the native RGB grayscale loop without an L raster.
pub(crate) fn execute_i16_from_rgb<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target: Mode,
) -> Result<DynamicImage, PilError>
where
    P: Pixel,
    P::Subpixel: ToGrayByte,
{
    super::luma16::from_luma8_samples(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| {
            let channels = pixel.channels();
            let rgb = [
                channels[0].to_gray_byte(),
                channels[1].to_gray_byte(),
                channels[2].to_gray_byte(),
            ];
            crate::color::grayscale_rgb_pixel(&rgb, 32768)
        }),
    )
}

// NO_PILLOW_SUPPORT: native high-depth source pixels converted to P/PA are a
// Rust-only extension; Pillow has no public mode identity for these sources.
// TODO(pillow-rs#3ptb): add target-only parity coverage across CPU Architecture
// requests and assert the exact P/PA result identity and Default fallback.
pub(crate) fn execute_palette_target<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target: Mode,
    dither: Option<DitherMethod>,
) -> Result<super::super::ops::PreparedConvertOutput, PilError>
where
    P: HighDepthPixel,
    P::Subpixel: Primitive,
{
    if !matches!(target, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let metadata = if target == Mode::ImagePaletteAlpha8 {
        super::super::ops::ConvertWrapperMetadata::ExifOnly
    } else {
        super::super::ops::ConvertWrapperMetadata::PreserveSource
    };
    crate::compute::pool_cpu::ops::color::convert::helpers::palette::web_palette_from_pixels(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel.rgb8().0),
        !matches!(dither, Some(DitherMethod::None)),
        target,
        metadata,
    )
}

fn luma<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> GrayImage
where
    P::Subpixel: Primitive,
{
    let mut output = GrayImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = pixel.luma8();
    }
    output
}

fn luma_alpha<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> GrayAlphaImage
where
    P::Subpixel: Primitive,
{
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = pixel.luma_alpha8();
    }
    output
}

fn rgb<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = pixel.rgb8();
    }
    output
}

fn rgba<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        *destination = pixel.rgba8();
    }
    output
}

fn cmyk<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = pixel.rgb8().0;
        *destination = Rgba([u8::MAX - red, u8::MAX - green, u8::MAX - blue, 0]);
    }
    output
}

fn ycbcr<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbImage
where
    P::Subpixel: Primitive,
{
    crate::color::rgb_pixels_to_ycbcr(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel.rgb8().0),
    )
}

fn hsv<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbImage
where
    P::Subpixel: Primitive,
{
    crate::color::rgb_pixels_to_hsv(
        source.width(),
        source.height(),
        source.pixels().map(|pixel| pixel.rgb8().0),
    )
}

fn to_i32<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = pixel.rgb8().0;
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
            red, green, blue,
        ));
    }
    output
}

fn to_f32<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = pixel.rgb8().0;
        *destination = Rgba(crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
            red, green, blue,
        ));
    }
    output
}
