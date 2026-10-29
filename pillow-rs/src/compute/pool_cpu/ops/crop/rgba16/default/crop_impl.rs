//! Serial rgba16 crop, retaining the original native sample algorithm.

use crate::compute::registry::support::InputContract;
use crate::raster::{DynamicImage, ImageBuffer, Rgba};

use super::super::super::helpers::crop::{CropRegion, SampleCrop};

pub(crate) const INPUTS: &[InputContract] = &[];

pub(crate) type Input = ImageBuffer<Rgba<u16>, Vec<u16>>;
pub(crate) type Crop<'a> = SampleCrop<'a, Rgba<u16>>;

pub(crate) fn prepare<'a>(image: &'a Input, region: CropRegion) -> Crop<'a> {
    SampleCrop::new(image, region)
}

pub(crate) fn execute(crop: Crop<'_>) -> DynamicImage {
    DynamicImage::ImageRgba16(crop.execute())
}
