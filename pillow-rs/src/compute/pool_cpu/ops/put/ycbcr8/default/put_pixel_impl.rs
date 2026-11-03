//! Original serial single and clone-once ordered writes for `ImageYCbCr8`.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Rgb};
use super::super::super::helpers::put_pixel::{self as helpers, BatchInput, PixelInput, PixelWrite};

pub(crate) type Input = crate::raster::RgbImage;
pub(crate) type Kernel<'a> = PixelInput<'a, Rgb<u8>>;
pub(crate) type BatchKernel<'a> = BatchInput<'a, Rgb<u8>>;

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
    let image = helpers::write_with_hidden_lanes(
        input,
        Rgb([color.0, color.1, color.2]),
        1,
        [color.3, 0],
        |_| [u8::MAX, 0],
    )?;
    Ok(DynamicImage::ImageYCbCr8(image))
}

#[inline]
pub(crate) fn execute_batch(input: BatchKernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::write_batch_with_hidden_lanes(
        input,
        |color| Rgb([color.0, color.1, color.2]),
        |color| [color.3, 0],
        1,
        |_| [u8::MAX, 0],
    )
    .map(DynamicImage::ImageYCbCr8)
}
