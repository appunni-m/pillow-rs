//! Serial one crop, retaining the original native sample algorithm.

use crate::compute::registry::support::InputContract;
use crate::error::PilError;
use crate::raster::Pixel;
use crate::raster::{DynamicImage, GrayImage, Luma};

use super::super::super::helpers::crop::{CropRegion, ByteCrop};

pub(crate) const INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    allow_empty: false, bytes_per_pixel: <Luma<u8> as Pixel>::CHANNEL_COUNT,
}];

pub(crate) type Input = GrayImage;
pub(crate) type Crop<'a> = ByteCrop<'a, Luma<u8>>;

pub(crate) fn prepare<'a>(image: &'a Input, region: CropRegion) -> Result<Crop<'a>, PilError> {
    ByteCrop::new(
        image,
        region,
        "raw_bytes_to_image: L buffer shape mismatch",
    )
}

pub(crate) fn execute(crop: Crop<'_>) -> Result<DynamicImage, PilError> {
    crop.execute().map(DynamicImage::One)
}
