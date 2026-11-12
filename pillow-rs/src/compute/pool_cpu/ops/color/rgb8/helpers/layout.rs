//! Native RGB8 layout identity kernel from the archived five-pixel path.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::RgbImage;
use wide::u8x16;

const IDENTITY_LANES: u8x16 = u8x16::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

pub(crate) fn execute(source: &RgbImage) -> Result<(Vec<u8>, u64), PilError> {
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGB Convert source storage did not match its dimensions".into(),
        ));
    }

    let pixels = source_dims.total_pixels();
    let mut output = Vec::with_capacity(source_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(5) {
        let active = (pixels - start).min(5);
        let input_len = active * 3;
        let source_start = start * 3;
        let mut padded = [0u8; 16];
        padded[..input_len]
            .copy_from_slice(&source.as_raw()[source_start..source_start + input_len]);
        let block = u8x16::new(padded).shuffle(IDENTITY_LANES).to_array();
        output.extend_from_slice(&block[..input_len]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGB Convert identity output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
