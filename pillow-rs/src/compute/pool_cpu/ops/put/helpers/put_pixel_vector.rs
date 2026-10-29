//! Original single-pixel and ordered RGBA vector writes, after strategy admission.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ImageBuffer, Pixel};
use super::masked_store::store;
use super::put_pixel::{BatchInput, PixelInput};

fn finish<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    width: u32, height: u32, output: Vec<u8>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let dimensions = CheckedDims::new(width, height, CHANNELS as u8)?;
    crate::image_utils::typed_bytes_to_buffer(dimensions, output)
}

pub(crate) fn single<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: PixelInput<'_, P>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let row_bytes = (input.image.width() as usize).checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel row stride overflow".into()))?;
    let target_start = (input.write.y as usize).checked_mul(row_bytes)
        .and_then(|row| row.checked_add((input.write.x as usize).checked_mul(CHANNELS)?))
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel offset overflow".into()))?;
    let target_end = target_start.checked_add(CHANNELS)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel end overflow".into()))?;
    let mut output = input.image.as_raw().to_vec();
    if target_end > output.len() {
        return Err(PilError::InternalError("prepared PutPixel source shape changed".into()));
    }
    if output.len() < 16 {
        let mut block = [0u8; 16];
        block[..output.len()].copy_from_slice(&output);
        store::<CHANNELS>(&mut block, 0, target_start, target_end, input.write.color);
        let output_len = output.len();
        output.copy_from_slice(&block[..output_len]);
    } else {
        let block_start = target_start.min(output.len() - 16);
        store::<CHANNELS>(&mut output, block_start, target_start, target_end, input.write.color);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(1);
    finish::<P, CHANNELS>(input.image.width(), input.image.height(), output)
}

fn apply_rgba_batch(output: &mut [u8], row_bytes: usize, ops: &[PipelineOp]) -> Result<(), PilError> {
    for op in ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Err(PilError::InternalError("prepared PutPixel run changed".into()));
        };
        let target_start = (*y as usize).checked_mul(row_bytes)
            .and_then(|row| row.checked_add((*x as usize).checked_mul(4)?))
            .ok_or_else(|| PilError::InternalError("SIMD PutPixel offset overflow".into()))?;
        let target_end = target_start.checked_add(4)
            .ok_or_else(|| PilError::InternalError("SIMD PutPixel end overflow".into()))?;
        if target_end > output.len() {
            return Err(PilError::InternalError("prepared PutPixel run source shape changed".into()));
        }
        let block_start = target_start.min(output.len() - 16);
        store::<4>(output, block_start, target_start, target_end, *color);
    }
    Ok(())
}

pub(crate) fn batch(
    input: BatchInput<'_, crate::raster::Rgba<u8>>,
) -> Result<crate::raster::RgbaImage, PilError> {
    let row_bytes = (input.image.width() as usize).checked_mul(4)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel row stride overflow".into()))?;
    let mut output = input.image.as_raw().to_vec();
    apply_rgba_batch(&mut output, row_bytes, input.ops)?;
    let result = finish::<crate::raster::Rgba<u8>, 4>(
        input.image.width(), input.image.height(), output,
    )?;
    for _ in input.ops {
        crate::compute::begin_pipeline_operation_telemetry("PutPixel");
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(1);
        crate::compute::finish_pipeline_operation_telemetry();
    }
    Ok(result)
}
