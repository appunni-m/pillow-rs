//! Byte-layout conversion through the existing native per-block kernel.

use super::super::super::NativeConvertLayout;
use super::super::native_convert_layout_block;

pub(super) fn append_layout_pixels(
    source: &[u8],
    pixel_count: usize,
    pixels_per_vector: usize,
    layout: NativeConvertLayout,
    output: &mut Vec<u8>,
) -> Option<u64> {
    let mut vector_blocks = 0u64;
    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let source_start = start_pixel * layout.source_channels;
        let input_len = active_pixels * layout.source_channels;
        let mut padded = [0u8; 16];
        padded[..input_len].copy_from_slice(&source[source_start..source_start + input_len]);
        let block = native_convert_layout_block(&padded, layout, pixels_per_vector)?;
        let output_len = active_pixels * layout.target_channels;
        output.extend_from_slice(&block[..output_len]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some(vector_blocks)
}
