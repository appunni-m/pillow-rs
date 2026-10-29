//! Serial la_premultiplied8 crop, retaining the original native sample algorithm.

use crate::compute::registry::support::InputContract;
use crate::error::PilError;
use crate::raster::Pixel;
use crate::raster::{DynamicImage, GrayAlphaImage, LumaA};

use super::super::super::helpers::crop::{CropRegion, ByteCrop};

pub(crate) const INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    allow_empty: false, bytes_per_pixel: <LumaA<u8> as Pixel>::CHANNEL_COUNT,
}];

pub(crate) type Input = GrayAlphaImage;
pub(crate) type Crop<'a> = ByteCrop<'a, LumaA<u8>>;

pub(crate) fn prepare<'a>(image: &'a Input, region: CropRegion) -> Result<Crop<'a>, PilError> {
    ByteCrop::new(
        image,
        region,
        "raw_bytes_to_image: LA buffer shape mismatch",
    )
}

pub(crate) fn execute(crop: Crop<'_>) -> Result<DynamicImage, PilError> {
    crop.execute().map(DynamicImage::ImageLaPremultiplied8)
}
