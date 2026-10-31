//! Native Luma8 to YCbCr conversion from the archived eight-pixel kernel.

use crate::error::PilError;
use crate::raster::GrayImage;
use wide::i32x8;

pub(crate) fn execute(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 3)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let luma = std::array::from_fn(|lane| {
            if lane < active {
                i32::from(source.as_raw()[start + lane])
            } else {
                0
            }
        });
        let y = i32x8::new(luma).to_array().map(|value| value.clamp(0, 255) as u8);
        for value in y.into_iter().take(active) {
            output.extend_from_slice(&[value, 128, 128]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "Luma Convert YCbCr output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
