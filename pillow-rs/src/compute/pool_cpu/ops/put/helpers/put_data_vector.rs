//! Original serial native-vector prefix copies, after concrete strategy admission.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use super::put_data::DataInput;

pub(crate) fn execute<P: Pixel<Subpixel = u8>>(
    input: DataInput<'_, P>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let expected_len = input.image.width().checked_mul(input.image.height())
        .and_then(|pixels| pixels.checked_mul(u32::from(P::CHANNEL_COUNT)))
        .map(|bytes| bytes as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutData byte count overflow".into()))?;
    let copy_len = input.data.len().min(expected_len);
    let (output, vector_blocks, scalar_tail) = if copy_len == expected_len {
        (input.data[..copy_len].to_vec(), 0, 0)
    } else {
        let mut output = vec![0u8; expected_len];
        let (mut vector_blocks, mut scalar_tail) =
            copy_native_bytes(input.image.as_raw(), &mut output).ok_or_else(|| {
                PilError::InternalError("SIMD PutData buffer shape mismatch".into())
            })?;
        if copy_len != 0 {
            let (blocks, tail) = copy_native_bytes(&input.data[..copy_len], &mut output[..copy_len])
                .ok_or_else(|| PilError::InternalError("SIMD PutData prefix shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        (output, vector_blocks, scalar_tail)
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let dimensions = CheckedDims::new_allow_empty(
        input.image.width(), input.image.height(), P::CHANNEL_COUNT,
    )?;
    crate::image_utils::typed_bytes_to_buffer(dimensions, output)
}
