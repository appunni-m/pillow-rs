//! Original serial single and clone-once ordered writes for `ImageLumaA8`.

use super::super::super::helpers::put_pixel::{
    self as helpers, BatchInput, PixelInput, PixelWrite,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, LumaA};

pub(crate) type Input = crate::raster::GrayAlphaImage;
pub(crate) type Kernel<'a> = PixelInput<'a, LumaA<u8>>;
pub(crate) type BatchKernel<'a> = BatchInput<'a, LumaA<u8>>;

#[inline]
pub(crate) fn prepare(input: &Input, write: PixelWrite) -> Kernel<'_> {
    PixelInput {
        image: input,
        write,
    }
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
        LumaA([color.0, color.3]),
        2,
        [color.1, color.2],
        |pixel| [pixel.0[0], pixel.0[0]],
    )?;
    Ok(DynamicImage::ImageLumaA8(image))
}

#[inline]
pub(crate) fn execute_batch(input: BatchKernel<'_>) -> Result<DynamicImage, PilError> {
    helpers::write_batch_with_hidden_lanes(
        input,
        |color| LumaA([color.0, color.3]),
        |color| [color.1, color.2],
        2,
        |pixel| [pixel.0[0], pixel.0[0]],
    )
    .map(DynamicImage::ImageLumaA8)
}
