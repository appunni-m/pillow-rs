//! Monomorphized native-pixel conversions for internal high-depth modes.

use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{
    DynamicImage, FromColor, GrayAlphaImage, GrayImage, ImageBuffer, Luma, LumaA, Mode, Pixel,
    Primitive, Rgb, RgbImage, Rgba, RgbaImage,
};

pub(crate) trait HighDepthPixel: Pixel {
    fn luma16(&self) -> Luma<u16>;
    fn luma8(&self) -> Luma<u8>;
    fn luma_alpha8(&self) -> LumaA<u8>;
    fn rgb8(&self) -> Rgb<u8>;
    fn rgba8(&self) -> Rgba<u8>;
}

macro_rules! declare_native_binary_strategy {
    ($mode:expr, $variant:ident) => {
        pub(crate) fn execute_binary(
            inputs: $crate::compute::registry::support::OperationInputs<'_>,
            dither: Option<&$crate::pipeline::DitherMethod>,
            selection: $crate::compute::registry::support::StrategySelection,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            if selection.implementation.architecture != $crate::compute::Architecture::Default {
                return Err($crate::error::PilError::InternalError(
                    "Convert selected an undeclared high-depth binary architecture".into(),
                ));
            }
            if inputs.mode() != $mode {
                return Err($crate::error::PilError::InternalError(
                    "Convert high-depth binary strategy received a different source mode".into(),
                ));
            }
            let $crate::raster::DynamicImage::$variant(source) = inputs.image else {
                return Err($crate::error::PilError::InternalError(
                    "Convert high-depth binary strategy received a different image carrier".into(),
                ));
            };
            $crate::compute::pool_cpu::ops::color::convert::helpers::high_depth::execute_binary(
                source, dither,
            )
        }
    };
}

pub(crate) use declare_native_binary_strategy;

// TODO(pillow-rs#3ptb): freeze CPU parity for all five CoreOnly high-depth
// source modes across declared targets, including float limits and alpha edges.
// TODO(pillow-rs#3ptb): decide whether cross-high-depth target pairs belong in
// the Rust-only contract and whether typed architecture kernels are warranted.

impl<P> HighDepthPixel for P
where
    P: Pixel,
    P::Subpixel: Primitive,
    Luma<u16>: FromColor<P>,
    Luma<u8>: FromColor<P>,
    LumaA<u8>: FromColor<P>,
    Rgb<u8>: FromColor<P>,
    Rgba<u8>: FromColor<P>,
{
    #[inline(always)]
    fn luma16(&self) -> Luma<u16> {
        let mut output = Luma([0]);
        output.copy_from_color(self);
        output
    }

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
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(rgbx(source))),
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(rgba(source))),
        Mode::ImageLaPremultiplied8 => Ok(DynamicImage::ImageLaPremultiplied8(
            premultiplied_luma_alpha(source),
        )),
        Mode::ImageRgbPremultiplied8 => Ok(DynamicImage::ImageRgbPremultiplied8(
            premultiplied_rgba(source),
        )),
        Mode::ImageCmyk8 => Ok(DynamicImage::ImageCmyk8(cmyk(source))),
        Mode::ImageYCbCr8 => Ok(DynamicImage::ImageYCbCr8(ycbcr(source))),
        Mode::ImageHsv8 => Ok(DynamicImage::ImageHsv8(hsv(source))),
        Mode::ImageI32 => Ok(DynamicImage::ImageI32(to_i32(source))),
        Mode::ImageF32 => Ok(DynamicImage::ImageF32(to_f32(source))),
        _ => Err(PilError::ValueError("image has wrong mode".into())),
    }
}

fn premultiplied_luma_alpha<P: HighDepthPixel>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
) -> GrayAlphaImage
where
    P::Subpixel: Primitive,
{
    let mut output = GrayAlphaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let LumaA([luma, alpha]) = pixel.luma_alpha8();
        let luma = crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
            luma, alpha,
        );
        *destination = LumaA([luma, alpha]);
    }
    output
}

fn premultiplied_rgba<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let Rgba([red, green, blue, alpha]) = pixel.rgba8();
        *destination = Rgba([
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                red, alpha,
            ),
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                green, alpha,
            ),
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::premultiply_byte(
                blue, alpha,
            ),
            alpha,
        ]);
    }
    output
}

// NO_PILLOW_SUPPORT: Pillow has no I;16 target for these internal high-depth
// sources. Convert each native pixel directly to its u16 luma sample; this
// keeps LA16 values intact and computes RGB luma before any byte projection.
// TODO(pillow-rs#3ptb): add focused CoreOnly parity for LA16/RGB16/RGBA16 and
// RGB32F/RGBA32F I;16 targets, including all four byte-order identities.
pub(crate) fn execute_i16<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    target: Mode,
) -> Result<DynamicImage, PilError>
where
    P: HighDepthPixel,
    P::Subpixel: Primitive,
{
    super::luma16::from_u16_samples(
        source.width(),
        source.height(),
        target,
        source.pixels().map(|pixel| pixel.luma16().0[0]),
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

// Convert each typed pixel through the same RGB8 and truncating grayscale
// formulas as the old fallback, without materializing an intermediate raster.
pub(crate) fn execute_binary<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    dither: Option<&DitherMethod>,
) -> Result<DynamicImage, PilError>
where
    P: HighDepthPixel,
    P::Subpixel: Primitive,
{
    let samples = source
        .pixels()
        .map(|pixel| crate::color::grayscale_rgb_pixel(&pixel.rgb8().0, 0));
    let mut output = GrayImage::new(source.width(), source.height());
    if dither.is_some_and(|method| matches!(method, &DitherMethod::None)) {
        for (destination, sample) in output.pixels_mut().zip(samples) {
            destination[0] = if sample >= 128 { u8::MAX } else { 0 };
        }
    } else {
        crate::compute::pool_cpu::ops::color::convert::helpers::binary::diffuse_gray_samples(
            samples,
            &mut output,
        )?;
    }
    Ok(DynamicImage::One(output))
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

fn rgbx<P: HighDepthPixel>(source: &ImageBuffer<P, Vec<P::Subpixel>>) -> RgbaImage
where
    P::Subpixel: Primitive,
{
    let mut output = RgbaImage::new(source.width(), source.height());
    for (destination, pixel) in output.pixels_mut().zip(source.pixels()) {
        let [red, green, blue] = pixel.rgb8().0;
        *destination = Rgba([red, green, blue, u8::MAX]);
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
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_i32_rgb(
                red, green, blue,
            ),
        );
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
        *destination = Rgba(
            crate::compute::pool_cpu::ops::color::convert::helpers::pixel::scalar_f32_rgb(
                red, green, blue,
            ),
        );
    }
    output
}
