//! Shared 128-bit native-byte copy kernel for typed I;16 crop strategies.

use crate::compute::pool_cpu::ops::crop::helpers::crop::CropRegion;
use crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes;
use crate::error::PilError;
use crate::raster::{GenericImageView, ImageBuffer, Luma};

pub(crate) fn crop_luma16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    region: CropRegion,
) -> Result<ImageBuffer<Luma<u16>, Vec<u16>>, PilError> {
    let source_stride = (image.width() as usize)
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("SIMD crop source stride overflow".into()))?;
    let output_stride = (region.width as usize)
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("SIMD crop output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(region.height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD crop output length overflow".into()))?;
    let mut samples = vec![0u16; output_len / 2];
    let source: &[u8] = bytemuck::cast_slice(image.as_raw());
    let output: &mut [u8] = bytemuck::cast_slice_mut(samples.as_mut_slice());
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..region.height as usize {
        let source_y = (region.top as usize)
            .checked_add(y)
            .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
        let source_start = source_y
            .checked_mul(source_stride)
            .and_then(|offset| {
                (region.left as usize)
                    .checked_mul(2)
                    .and_then(|left| offset.checked_add(left))
            })
            .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
        let source_end = source_start
            .checked_add(output_stride)
            .ok_or_else(|| PilError::ValueError("SIMD crop source range overflow".into()))?;
        let output_start = y
            .checked_mul(output_stride)
            .ok_or_else(|| PilError::ValueError("SIMD crop output range overflow".into()))?;
        let output_end = output_start
            .checked_add(output_stride)
            .ok_or_else(|| PilError::ValueError("SIMD crop output range overflow".into()))?;
        let source_row = source
            .get(source_start..source_end)
            .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        let output_row = output
            .get_mut(output_start..output_end)
            .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        let (blocks, tail) = copy_native_bytes(source_row, output_row)
            .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }

    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    ImageBuffer::from_raw(region.width, region.height, samples)
        .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))
}
