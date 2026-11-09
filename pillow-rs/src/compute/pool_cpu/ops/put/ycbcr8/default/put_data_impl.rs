//! Original byte-prefix algorithm specialized for source `ImageYCbCr8`.

use super::super::super::helpers::put_data::{self as helpers, DataInput, PayloadLayout};
use crate::error::PilError;
use crate::raster::ConvertBuffer;
use crate::raster::DynamicImage;

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = DataInput<'a, crate::raster::Rgb<u8>>;

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
        PayloadLayout::LumaAlpha | PayloadLayout::IndexAlpha => {
            helpers::converted(width, height, data, || image.convert())
                .map(DynamicImage::ImageLumaA8)
        }
        PayloadLayout::Rgb => helpers::native(image, data).map(DynamicImage::ImageYCbCr8),
        PayloadLayout::Color3 => helpers::native(image, data).map(DynamicImage::ImageYCbCr8),
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
        3,
        1,
        hidden_lanes,
        "putdata RGB storage mismatch",
        "putdata RGB hidden lane was missing",
        |_| [u8::MAX, 0],
        |_| [u8::MAX, 0],
    )
}
