//! Packed-word x86_64 kernel for large native RGBA Rotate270 operations.

const PACKED_RGBA_TILE_SIZE: usize = 32;

#[inline(always)]
fn load_rgba_words(row: &[u8]) -> [u32; 4] {
    [
        u32::from_ne_bytes(row[0..4].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[4..8].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[8..12].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[12..16].try_into().expect("RGBA word")),
    ]
}

#[inline(always)]
fn store_rgba_words(row: &mut [u8], words: [u32; 4]) {
    row[0..4].copy_from_slice(&words[0].to_ne_bytes());
    row[4..8].copy_from_slice(&words[1].to_ne_bytes());
    row[8..12].copy_from_slice(&words[2].to_ne_bytes());
    row[12..16].copy_from_slice(&words[3].to_ne_bytes());
}

/// Transpose complete 4×4 RGBA blocks within dedicated 32×32 outer tiles.
/// Each source row contributes four contiguous native pixels; the packed words
/// are then written as four contiguous destination rows.
pub(super) fn rotate270_tiled(source: &[u8], output: &mut [u8], width: u32, height: u32) {
    let width = width as usize;
    let height = height as usize;
    let source_stride = width * 4;
    let output_stride = height * 4;
    let tile_size = PACKED_RGBA_TILE_SIZE;

    debug_assert_eq!(output.len(), source_stride * height);
    debug_assert_eq!(tile_size % 4, 0);
    debug_assert_eq!(width % 4, 0);
    debug_assert_eq!(height % 4, 0);

    for output_y_tile in (0..width).step_by(tile_size) {
        let output_y_end = (output_y_tile + tile_size).min(width);
        for output_x_tile in (0..height).step_by(tile_size) {
            let output_x_end = (output_x_tile + tile_size).min(height);
            for output_y in (output_y_tile..output_y_end).step_by(4) {
                let source_x_offset = output_y * 4;
                for output_x in (output_x_tile..output_x_end).step_by(4) {
                    let source_y0 = height - 1 - output_x;
                    let source_y1 = source_y0 - 1;
                    let source_y2 = source_y1 - 1;
                    let source_y3 = source_y2 - 1;
                    let source_end = source_x_offset + 16;
                    let source_row0 = load_rgba_words(
                        &source[source_y0 * source_stride + source_x_offset
                            ..source_y0 * source_stride + source_end],
                    );
                    let source_row1 = load_rgba_words(
                        &source[source_y1 * source_stride + source_x_offset
                            ..source_y1 * source_stride + source_end],
                    );
                    let source_row2 = load_rgba_words(
                        &source[source_y2 * source_stride + source_x_offset
                            ..source_y2 * source_stride + source_end],
                    );
                    let source_row3 = load_rgba_words(
                        &source[source_y3 * source_stride + source_x_offset
                            ..source_y3 * source_stride + source_end],
                    );

                    let destination_start = output_y * output_stride + output_x * 4;
                    store_rgba_words(
                        &mut output[destination_start..destination_start + 16],
                        [
                            source_row0[0],
                            source_row1[0],
                            source_row2[0],
                            source_row3[0],
                        ],
                    );
                    store_rgba_words(
                        &mut output[destination_start + output_stride
                            ..destination_start + output_stride + 16],
                        [
                            source_row0[1],
                            source_row1[1],
                            source_row2[1],
                            source_row3[1],
                        ],
                    );
                    store_rgba_words(
                        &mut output[destination_start + 2 * output_stride
                            ..destination_start + 2 * output_stride + 16],
                        [
                            source_row0[2],
                            source_row1[2],
                            source_row2[2],
                            source_row3[2],
                        ],
                    );
                    store_rgba_words(
                        &mut output[destination_start + 3 * output_stride
                            ..destination_start + 3 * output_stride + 16],
                        [
                            source_row0[3],
                            source_row1[3],
                            source_row2[3],
                            source_row3[3],
                        ],
                    );
                }
            }
        }
    }
}
