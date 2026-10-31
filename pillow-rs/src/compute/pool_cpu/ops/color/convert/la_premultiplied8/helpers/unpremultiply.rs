//! Native interleaved La-to-LA conversion.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;
use wide::u8x16;

pub(crate) fn execute(source: &GrayAlphaImage) -> Result<(Vec<u8>, u64), PilError> {
    let dimensions = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "La Convert source storage did not match its dimensions".into(),
        ));
    }

    let mut output = Vec::with_capacity(dimensions.total_bytes());
    let mut vector_blocks = 0u64;
    for block in source.as_raw().chunks(16) {
        let active_bytes = block.len();
        let mut padded = [0u8; 16];
        padded[..active_bytes].copy_from_slice(block);
        let converted =
            crate::compute::pool_cpu::ops::color::convert::helpers::alpha::unpremultiply_block::<2>(
                u8x16::new(padded),
            )
            .to_array();
        output.extend_from_slice(&converted[..active_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "La Convert output storage did not match its dimensions".into(),
        ));
    }
    Ok((output, vector_blocks))
}
