//! Native LA to YCbCr conversion from the archived eight-pixel kernel.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;
use wide::i32x8;

pub(crate) fn convert(source: &GrayAlphaImage) -> Result<(Vec<u8>, u64), PilError> {
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    let output_bytes = dims
        .total_pixels()
        .checked_mul(3)
        .ok_or_else(|| PilError::DimensionError("YCbCr Convert output size overflowed".into()))?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..dims.total_pixels()).step_by(8) {
        let active = (dims.total_pixels() - start).min(8);
        let luma = std::array::from_fn(|lane| {
            if lane < active {
                i32::from(source.as_raw()[(start + lane) * 2])
            } else {
                0
            }
        });
        let values = i32x8::new(luma).to_array();
        for value in values.into_iter().take(active) {
            output.extend_from_slice(&[value.clamp(0, 255) as u8, 128, 128]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "LA Convert YCbCr output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
