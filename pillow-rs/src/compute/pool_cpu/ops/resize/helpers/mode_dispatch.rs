//! Binds exact Resize mode variants to their declared mode owners.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(crate) fn execute_byte_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::One(input) => Ok(DynamicImage::One(super::super::one::ops::resize(
            context, selection, input, width, height, filter, bounds,
        )?)),
        DynamicImage::ImageLuma8(input) => Ok(DynamicImage::ImageLuma8(
            super::super::luma8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLumaA8(input) => Ok(DynamicImage::ImageLumaA8(
            super::super::luma_a8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLaPremultiplied8(input) => Ok(DynamicImage::ImageLaPremultiplied8(
            super::super::la_premultiplied8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImagePalette8(input) => Ok(DynamicImage::ImagePalette8(
            super::super::palette8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImagePaletteAlpha8(input) => Ok(DynamicImage::ImagePaletteAlpha8(
            super::super::palette_alpha8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgb8(input) => Ok(DynamicImage::ImageRgb8(
            super::super::rgb8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgba8(input) => Ok(DynamicImage::ImageRgba8(
            super::super::rgba8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgbPremultiplied8(input) => Ok(DynamicImage::ImageRgbPremultiplied8(
            super::super::rgb_premultiplied8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageRgbx8(input) => Ok(DynamicImage::ImageRgbx8(
            super::super::rgbx8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageCmyk8(input) => Ok(DynamicImage::ImageCmyk8(
            super::super::cmyk8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageYCbCr8(input) => Ok(DynamicImage::ImageYCbCr8(
            super::super::ycbcr8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageLab8(input) => Ok(DynamicImage::ImageLab8(
            super::super::lab8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        DynamicImage::ImageHsv8(input) => Ok(DynamicImage::ImageHsv8(
            super::super::hsv8::ops::resize(context, selection, input, width, height, filter, bounds)?,
        )),
        _ => Err(PilError::InternalError(
            "byte Resize mode dispatch received a non-byte image".into(),
        )),
    }
}

pub(crate) fn execute_luma16_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageI16(input) => super::super::i16::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageI16L(input) => super::super::i16l::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageI16B(input) => super::super::i16b::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageI16N(input) => super::super::i16n::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        _ => Err(PilError::InternalError(
            "I;16 Resize mode dispatch received a different image".into(),
        )),
    }
}

pub(crate) fn execute_typed_mode(
    image: &DynamicImage,
    context: &ExecutionContext,
    selection: &StrategySelection,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLumaA16(input) => super::super::luma_a16::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageRgb16(input) => super::super::rgb16::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageRgba16(input) => super::super::rgba16::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageRgb32F(input) => super::super::rgb32f::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        DynamicImage::ImageRgba32F(input) => super::super::rgba32f::ops::resize_selected(
            context, selection, input, width, height, filter, bounds,
        ),
        _ => Err(PilError::InternalError(
            "typed Resize mode dispatch received a different image".into(),
        )),
    }
}
