//! Existing vector copy for the fixed L-mode radial-gradient raster.

use crate::compute::pool_cpu::ops::effects::helpers::generator::GeneratedImage;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};
use wide::u8x16;

pub(crate) fn execute(template: &[u8]) -> Result<GeneratedImage, PilError> {
    let mut output = Vec::with_capacity(template.len());
    let mut vector_blocks = 0u64;
    for chunk in template.chunks_exact(16) {
        let mut lanes = [0u8; 16];
        lanes.copy_from_slice(chunk);
        output.extend_from_slice(&u8x16::new(lanes).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail = template.chunks_exact(16).remainder();
    output.extend_from_slice(tail);
    let image = GrayImage::from_raw(256, 256, output)
        .ok_or_else(|| PilError::InternalError("RadialGradient L vector shape mismatch".into()))?;
    Ok(GeneratedImage {
        image: DynamicImage::ImageLuma8(image),
        vector_blocks,
        scalar_tail: tail.len() as u64,
    })
}
