//! Native Luma8 to CMYK conversion from the archived four-pixel kernel.

use crate::error::PilError;
use crate::raster::GrayImage;
use wide::u8x16;

pub(crate) fn execute(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 4)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(4) {
        let active = (pixels - start).min(4);
        let mut luma = [0u8; 16];
        luma[..active].copy_from_slice(&source.as_raw()[start..start + active]);
        let black = (u8x16::splat(u8::MAX) - u8x16::new(luma)).to_array();
        for value in black.into_iter().take(active) {
            output.extend_from_slice(&[0, 0, 0, value]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "Luma Convert CMYK output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
