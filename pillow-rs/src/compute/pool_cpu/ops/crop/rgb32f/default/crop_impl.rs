//! Serial rgb32f crop, retaining the original native sample algorithm.

use crate::compute::registry::support::InputContract;
use crate::raster::{DynamicImage, Rgb, Rgb32FImage};

use super::super::super::helpers::crop::{CropRegion, SampleCrop};

pub(crate) const INPUTS: &[InputContract] = &[];

pub(crate) type Input = Rgb32FImage;
pub(crate) type Crop<'a> = SampleCrop<'a, Rgb<f32>>;

pub(crate) fn prepare<'a>(image: &'a Input, region: CropRegion) -> Crop<'a> {
    SampleCrop::new(image, region)
}

pub(crate) fn execute(crop: Crop<'_>) -> DynamicImage {
    DynamicImage::ImageRgb32F(crop.execute())
}
