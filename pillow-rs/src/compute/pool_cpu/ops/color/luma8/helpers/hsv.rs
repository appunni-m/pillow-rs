//! Native Luma8 to HSV conversion from the archived gray-lane path.

use crate::error::PilError;
use crate::raster::GrayImage;

pub(crate) fn execute(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 3)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let luma: [u8; 8] = std::array::from_fn(|lane| {
            if lane < active {
                source.as_raw()[start + lane]
            } else {
                0
            }
        });
        for value in luma.into_iter().take(active) {
            output.extend_from_slice(&[0, 0, value]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "Luma Convert HSV output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
