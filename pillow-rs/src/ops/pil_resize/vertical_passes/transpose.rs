const RESIZE_VERTICAL_TRANSPOSE_THRESHOLD: usize = 512 * 512;

#[inline]
pub(in crate::ops::pil_resize) fn should_transpose_vertical(
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> bool {
    matches!(channels, 1..=4)
        && source_rows > 1
        && output_width > 1
        && (source_rows as usize).saturating_mul(output_width as usize)
            >= RESIZE_VERTICAL_TRANSPOSE_THRESHOLD
}

/// Reorder the horizontal-pass result from `[source_y][output_x][channel]` to
/// `[output_x][source_y][channel]` for the cache-local vertical pass.
pub(in crate::ops::pil_resize) fn transpose_resize_intermediate(
    source: &[u8],
    source_rows: u32,
    output_width: u32,
    channels: usize,
) -> Vec<u8> {
    let source_row_stride = output_width as usize * channels;
    let destination_row_stride = source_rows as usize * channels;
    let mut destination = vec![0u8; source.len()];

    // RGB's three-byte pixels make the current column-at-a-time transpose
    // touch one source cache line per row and channel. Tile both dimensions so
    // each source tile is read in row-major order while the corresponding
    // transposed destination rows stay resident during their short write span.
    // Parallel CPU keeps its disjoint-column scheduling below.
    #[cfg(not(feature = "parallel"))]
    if channels == 3 {
        const TILE_WIDTH: usize = 32;
        const TILE_HEIGHT: usize = 32;
        let source_rows = source_rows as usize;
        let output_width = output_width as usize;
        for tile_y in (0..source_rows).step_by(TILE_HEIGHT) {
            let end_y = (tile_y + TILE_HEIGHT).min(source_rows);
            for tile_x in (0..output_width).step_by(TILE_WIDTH) {
                let end_x = (tile_x + TILE_WIDTH).min(output_width);
                for y in tile_y..end_y {
                    let source_row_start = y * source_row_stride;
                    for x in tile_x..end_x {
                        let source_start = source_row_start + x * 3;
                        let destination_start = x * destination_row_stride + y * 3;
                        destination[destination_start..destination_start + 3]
                            .copy_from_slice(&source[source_start..source_start + 3]);
                    }
                }
            }
        }
        return destination;
    }

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        &mut destination,
        destination_row_stride,
        output_width as usize,
        |_row_start, _row_end, x, row| {
            let x = x as usize;
            for y in 0..source_rows as usize {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );

    #[cfg(not(feature = "parallel"))]
    for x in 0..output_width as usize {
        let row_start = x * destination_row_stride;
        let row = &mut destination[row_start..row_start + destination_row_stride];
        for y in 0..source_rows as usize {
            let source_start = y * source_row_stride + x * channels;
            let destination_start = y * channels;
            row[destination_start..destination_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }

    destination
}
