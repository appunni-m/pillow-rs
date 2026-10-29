//! Original serial single and clone-once ordered writes for `ImagePalette8`.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Luma};
use super::super::super::helpers::put_pixel::{self as helpers, BatchInput, PixelInput, PixelWrite};

pub(crate) type Input = crate::raster::GrayImage;
pub(crate) type Kernel<'a> = PixelInput<'a, Luma<u8>>;
pub(crate) type BatchKernel<'a> = BatchInput<'a, Luma<u8>>;

#[inline]
pub(crate) fn prepare(input: &Input, write: PixelWrite) -> Kernel<'_> {
    PixelInput { image: input, write }
}

#[inline]
pub(crate) fn prepare_batch<'a>(input: &'a Input, ops: &'a [PipelineOp]) -> BatchKernel<'a> {
    BatchInput { image: input, ops }
}

#[inline]
pub(crate) fn execute(input: Kernel<'_>) -> Result<DynamicImage, PilError> {
    let color = input.write.color;
    Ok(DynamicImage::ImagePalette8(helpers::write(input, Luma([color.0]))))
}

#[inline]
pub(crate) fn execute_batch(input: BatchKernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::write_batch(input, |color| Luma([color.0]))
        .map(DynamicImage::ImagePalette8)
}
