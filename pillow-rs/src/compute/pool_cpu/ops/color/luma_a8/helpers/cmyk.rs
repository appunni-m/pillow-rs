//! Native LA to CMYK conversion from the archived four-pixel kernel.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;
use wide::u8x16;

pub(crate) fn convert(source: &GrayAlphaImage) -> Result<(Vec<u8>, u64), PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    let output_bytes = dims
        .total_pixels()
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("CMYK Convert output size overflowed".into()))?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..dims.total_pixels()).step_by(4) {
        let active = (dims.total_pixels() - start).min(4);
        let mut luma = [0u8; 16];
        for (lane, value) in luma.iter_mut().take(active).enumerate() {
            *value = source.as_raw()[(start + lane) * 2];
        }
        let black = (u8x16::splat(u8::MAX) - u8x16::new(luma)).to_array();
        for lane in black.into_iter().take(active) {
            output.extend_from_slice(&[0, 0, 0, lane]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "LA Convert CMYK output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
