//! Native Luma8 to LA conversion from the archived byte-layout kernel.

use crate::error::PilError;
use crate::raster::GrayImage;
use wide::u8x16;

const LUMA_TO_LA: u8x16 = u8x16::new([
    0, 15, 1, 15, 2, 15, 3, 15, 4, 15, 5, 15, 6, 15, 7, 15,
]);

pub(crate) fn execute(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 2)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let mut luma = [0u8; 16];
        luma[..active].copy_from_slice(&source.as_raw()[start..start + active]);
        luma[15] = u8::MAX;
        let interleaved = u8x16::new(luma).swizzle_relaxed(LUMA_TO_LA).to_array();
        output.extend_from_slice(&interleaved[..active * 2]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "Luma Convert LA output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
