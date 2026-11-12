//! Native LA premultiplication from the archived portable-vector alpha kernel.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::color::helpers::convert::wide::{div255_u16x16, pack_u16x16};
use crate::error::PilError;
use crate::raster::GrayAlphaImage;
use wide::{u8x16, u16x16};

pub(crate) fn convert(source: &GrayAlphaImage) -> Result<(GrayAlphaImage, u64), PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    let mut output = Vec::with_capacity(dims.total_bytes());
    let mut vector_blocks = 0u64;
    let alpha_indices = u8x16::new(std::array::from_fn(|lane| ((lane / 2) * 2 + 1) as u8));
    let alpha_mask = u8x16::new(std::array::from_fn(
        |lane| {
            if lane % 2 == 1 { u8::MAX } else { 0 }
        },
    ));
    for start in (0..dims.total_bytes()).step_by(16) {
        let active_bytes = (dims.total_bytes() - start).min(16);
        let mut padded = [0u8; 16];
        padded[..active_bytes].copy_from_slice(&source.as_raw()[start..start + active_bytes]);
        let input = u8x16::new(padded);
        let alpha = input.shuffle(alpha_indices);
        let scaled = div255_u16x16(u16x16::from(input) * u16x16::from(alpha) + u16x16::splat(127));
        let converted = pack_u16x16(scaled);
        let pixels = alpha_mask
            .simd_eq(u8x16::ZERO)
            .select(converted, input)
            .to_array();
        output.extend_from_slice(&pixels[..active_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let image = GrayAlphaImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("LA Convert La output shape mismatch".into()))?;
    Ok((image, vector_blocks))
}
