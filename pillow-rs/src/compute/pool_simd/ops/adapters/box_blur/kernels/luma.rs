//! Focused kernels for native-L SIMD BoxBlur.

use super::{
    SIMD_BOX_BLUR_BIAS, SIMD_BOX_BLUR_SCALE, simd_blur_pack_u32x16_to_bytes,
    simd_luma_horizontal_radius_one_rows, simd_luma_load_radius_one_block, u16x16, u32x16,
};

/// Apply Pillow's horizontal and vertical radius-one BoxBlur passes to L rows.
///
/// The horizontal pass is rounded before the vertical pass, matching Pillow's
/// two-pass fixed-point behavior. Three horizontally filtered rows stay in a
/// ring, avoiding a full-frame input clone and horizontal intermediate.
pub(crate) fn simd_luma_box_blur_radius_one_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
) {
    debug_assert_eq!(source.len(), width * height);
    debug_assert_eq!(destination.len(), source.len());
    if width == 0 || height == 0 {
        return;
    }

    let mut rows = [vec![0u8; width], vec![0u8; width], vec![0u8; width]];
    let mut next_horizontal_row = 0usize;
    let last_row = height - 1;
    let whole_weight = u32x16::splat(SIMD_BOX_BLUR_SCALE / 3);
    let rounding_bias = u32x16::splat(SIMD_BOX_BLUR_BIAS);

    for y in 0..height {
        let bottom_row = y.saturating_add(1).min(last_row);
        while next_horizontal_row <= bottom_row {
            let source_start = next_horizontal_row * width;
            let source_row = &source[source_start..source_start + width];
            let slot = next_horizontal_row % 3;
            simd_luma_horizontal_radius_one_rows(source_row, &mut rows[slot], width, 1);
            next_horizontal_row += 1;
        }

        let top_row = y.saturating_sub(1);
        let top = &rows[top_row % 3];
        let center = &rows[y % 3];
        let bottom = &rows[bottom_row % 3];
        let output_start = y * width;
        let output_row = &mut destination[output_start..output_start + width];

        for x in (0..width).step_by(16) {
            let top = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                top,
                x,
                top[width - 1],
            )));
            let center = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                center,
                x,
                center[width - 1],
            )));
            let bottom = u32x16::from(u16x16::from(simd_luma_load_radius_one_block(
                bottom,
                x,
                bottom[width - 1],
            )));
            let weighted = (top + center + bottom) * whole_weight + rounding_bias;
            let packed = simd_blur_pack_u32x16_to_bytes(weighted >> 24u32);
            let count = (width - x).min(16);
            output_row[x..x + count].copy_from_slice(&packed[..count]);
        }
    }

    let vector_blocks = width.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
}
