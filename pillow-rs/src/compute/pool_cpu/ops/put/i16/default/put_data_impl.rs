//! Original byte-prefix algorithm specialized for source `ImageI16`.

use crate::error::PilError;
use crate::raster::DynamicImage;
use super::super::super::helpers::put_data::{self as helpers, DataInput, PayloadLayout};

pub(crate) type Input = crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
pub(crate) type Kernel<'a> = DataInput<'a, crate::raster::Luma<u16>>;

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
        PayloadLayout::Luma | PayloadLayout::Index => helpers::converted(width, height, data, || helpers::clipped(image, |sample| crate::raster::Luma([sample])))
            .map(DynamicImage::ImageLuma8),
        PayloadLayout::LumaAlpha | PayloadLayout::IndexAlpha => helpers::converted(width, height, data, || helpers::clipped(image, |sample| crate::raster::LumaA([sample, u8::MAX])))
            .map(DynamicImage::ImageLumaA8),
        PayloadLayout::Rgb | PayloadLayout::Color3 => helpers::converted(width, height, data, || helpers::clipped(image, |sample| crate::raster::Rgb([sample, sample, sample])))
            .map(DynamicImage::ImageRgb8),
        PayloadLayout::Rgba | PayloadLayout::Raw4 => helpers::converted(width, height, data, || helpers::clipped(image, |sample| crate::raster::Rgba([sample, sample, sample, u8::MAX])))
            .map(DynamicImage::ImageRgba8),
    }
}
