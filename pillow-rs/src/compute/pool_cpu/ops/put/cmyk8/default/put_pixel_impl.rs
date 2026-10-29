//! Original serial single and clone-once ordered writes for `ImageCmyk8`.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Rgba};
use super::super::super::helpers::put_pixel::{self as helpers, BatchInput, PixelInput, PixelWrite};

pub(crate) type Input = crate::raster::RgbaImage;
pub(crate) type Kernel<'a> = PixelInput<'a, Rgba<u8>>;
pub(crate) type BatchKernel<'a> = BatchInput<'a, Rgba<u8>>;

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
    Ok(DynamicImage::ImageCmyk8(helpers::write(input, Rgba([color.0, color.1, color.2, color.3]))))
}

#[inline]
pub(crate) fn execute_batch(input: BatchKernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::write_batch(input, |color| Rgba([color.0, color.1, color.2, color.3]))
        .map(DynamicImage::ImageCmyk8)
}
