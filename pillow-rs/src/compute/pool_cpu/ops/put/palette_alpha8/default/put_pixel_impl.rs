//! Original serial single and clone-once ordered writes for `ImagePaletteAlpha8`.

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, LumaA};
use super::super::super::helpers::put_pixel::{self as helpers, BatchInput, PixelInput, PixelWrite};

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = PixelInput<'a, LumaA<u8>>;
pub(crate) type BatchKernel<'a> = BatchInput<'a, LumaA<u8>>;

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
    Ok(DynamicImage::ImagePaletteAlpha8(helpers::write(input, LumaA([color.0, color.3]))))
}

#[inline]
pub(crate) fn execute_batch(input: BatchKernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::write_batch(input, |color| LumaA([color.0, color.3]))
        .map(DynamicImage::ImagePaletteAlpha8)
}
