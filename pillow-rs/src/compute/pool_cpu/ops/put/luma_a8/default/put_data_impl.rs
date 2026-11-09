//! Original byte-prefix algorithm specialized for source `ImageLumaA8`.

use super::super::super::helpers::put_data::{self as helpers, DataInput, PayloadLayout};
use crate::error::PilError;
use crate::raster::ConvertBuffer;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = DataInput<'a, crate::raster::LumaA<u8>>;

#[inline]
pub(crate) fn prepare<'a>(input: &'a Input, data: &'a [u8], payload: PayloadLayout) -> Kernel<'a> {
    DataInput {
        image: input,
        data,
        payload,
    }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let image = input.image;
    let data = input.data;
    let (width, height) = image.dimensions();
    match input.payload {
        PayloadLayout::Luma | PayloadLayout::Index => {
            helpers::converted(width, height, data, || image.convert())
                .map(DynamicImage::ImageLuma8)
        }
        PayloadLayout::LumaAlpha => helpers::native(image, data).map(DynamicImage::ImageLumaA8),
        PayloadLayout::IndexAlpha => helpers::native(image, data).map(DynamicImage::ImageLumaA8),
        PayloadLayout::Rgb | PayloadLayout::Color3 => {
            helpers::converted(width, height, data, || image.convert()).map(DynamicImage::ImageRgb8)
        }
        PayloadLayout::Rgba | PayloadLayout::Raw4 => {
            helpers::converted(width, height, data, || image.convert())
                .map(DynamicImage::ImageRgba8)
        }
    }
}

pub(crate) fn execute_value_at(
    image: &mut Input,
    pixel_index: usize,
    bytes: &[u8],
    hidden_lanes: Option<[u8; 2]>,
) -> Result<(), PilError> {
    super::super::super::helpers::put_data_value_at::write_compact_bytes(
        image,
        pixel_index,
        bytes,
        2,
        2,
        hidden_lanes,
        "putdata LA storage mismatch",
        "putdata LA hidden lanes were missing",
        |bytes| [bytes[0], bytes[0]],
        |pixel| [pixel.0[0], pixel.0[0]],
    )
}
