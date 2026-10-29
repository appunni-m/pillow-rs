//! Original byte-prefix algorithm specialized for source `ImageRgb32F`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use crate::raster::ConvertBuffer;
use super::super::super::helpers::put_data::{self as helpers, DataInput, PayloadLayout};

pub(crate) type Input = crate::raster::Rgb32FImage;
pub(crate) type Kernel<'a> = DataInput<'a, crate::raster::Rgb<f32>>;

#[inline]
pub(crate) fn prepare<'a>(input: &'a Input, data: &'a [u8], payload: PayloadLayout) -> Kernel<'a> {
    DataInput { image: input, data, payload }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let image = input.image;
    let data = input.data;
    let (width, height) = image.dimensions();
    match input.payload {
        PayloadLayout::Luma | PayloadLayout::Index => helpers::converted(width, height, data, || image.convert())
            .map(DynamicImage::ImageLuma8),
        PayloadLayout::LumaAlpha | PayloadLayout::IndexAlpha => helpers::converted(width, height, data, || image.convert())
            .map(DynamicImage::ImageLumaA8),
        PayloadLayout::Rgb | PayloadLayout::Color3 => helpers::converted(width, height, data, || image.convert())
            .map(DynamicImage::ImageRgb8),
        PayloadLayout::Rgba | PayloadLayout::Raw4 => helpers::converted(width, height, data, || image.convert())
            .map(DynamicImage::ImageRgba8),
    }
}
