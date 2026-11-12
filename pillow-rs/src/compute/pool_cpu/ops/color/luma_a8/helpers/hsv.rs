//! Native LA to HSV conversion from the archived gray-lane path.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;

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
        .ok_or_else(|| PilError::DimensionError("HSV Convert output size overflowed".into()))?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..dims.total_pixels()).step_by(8) {
        let active = (dims.total_pixels() - start).min(8);
        for lane in 0..active {
            output.extend_from_slice(&[0, 0, source.as_raw()[(start + lane) * 2]]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "LA Convert HSV output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
