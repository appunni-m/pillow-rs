//! AVX-512F 2x2 reduction for exact signed ImageI32 samples.

#![allow(unsafe_code)] // Prepared dispatch validates AVX-512F before entering the leaf.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, RgbaImage};

const LANES: usize = super::super::super::helpers::i32_avx512f::LANES;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    x_factor: u32,
    y_factor: u32,
) -> Result<DynamicImage, PilError> {
    let output_width = input.width().div_ceil(x_factor.max(1));
    let output_height = input.height().div_ceil(y_factor.max(1));
    if x_factor != 2
        || y_factor != 2
        || !input.width().is_multiple_of(2)
        || !input.height().is_multiple_of(2)
        || !(output_width as usize).is_multiple_of(LANES)
    {
        return super::super::super::helpers::scalar32::reduce_i32(input, x_factor, y_factor)
            .map(DynamicImage::ImageI32);
    }

    let source_width = usize::try_from(input.width())
        .map_err(|_| PilError::InternalError("ImageI32 AVX-512F width overflow".into()))?;
    let mut output = CheckedDims::new(output_width, output_height, 4)?.alloc_buffer();
    let source = input.as_raw();
    for output_y in 0..output_height as usize {
        let top = output_y * 2 * source_width * 4;
        let bottom = top + source_width * 4;
        for output_x in (0..output_width as usize).step_by(LANES) {
            let source_x = output_x * 8;
            // SAFETY: the prepared guard requires even source dimensions and complete 16-output
            // blocks. Each row pointer therefore spans 32 valid i32 samples; the runtime context
            // is checked for AVX-512F before this function is selected.
            let values = unsafe {
                super::super::super::helpers::i32_avx512f::reduce_quartet_block(
                    source.as_ptr().add(top + source_x).cast(),
                    source.as_ptr().add(bottom + source_x).cast(),
                )
            };
            let destination = (output_y * output_width as usize + output_x) * 4;
            for (lane, value) in values.into_iter().enumerate() {
                let offset = destination + lane * 4;
                output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
    }
    RgbaImage::from_raw(output_width, output_height, output)
        .map(DynamicImage::ImageI32)
        .ok_or_else(|| PilError::InternalError("failed to construct I Reduce output".into()))
}
