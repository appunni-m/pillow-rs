//! AVX2 tiles for transposing opaque four-byte pixels.

#![allow(unsafe_code)]

use core::arch::x86_64::{
    _mm256_loadu_si256, _mm256_permute2x128_si256, _mm256_permutevar8x32_epi32, _mm256_setr_epi32,
    _mm256_setzero_si256, _mm256_storeu_si256, _mm256_unpackhi_epi32, _mm256_unpackhi_epi64,
    _mm256_unpacklo_epi32, _mm256_unpacklo_epi64,
};

const TILE: usize = 8;
const CHANNELS: usize = 4;

pub(super) fn transpose_rgba(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    reverse_x: bool,
    reverse_y: bool,
) -> Option<(u64, u64)> {
    let expected_bytes = width.checked_mul(height)?.checked_mul(CHANNELS)?;
    if width < TILE
        || height < TILE
        || source.len() != expected_bytes
        || output.len() != expected_bytes
        || !std::is_x86_feature_detected!("avx2")
    {
        return None;
    }

    let full_rows = width / TILE * TILE;
    let full_columns = height / TILE * TILE;

    // SAFETY: AVX2 is detected above. The full-tile bounds ensure every
    // unaligned load and store stays within the validated source and output
    // slices; each tile owns disjoint output rows and byte spans.
    unsafe {
        transpose_full_tiles(
            source,
            output,
            width,
            height,
            full_rows,
            full_columns,
            reverse_x,
            reverse_y,
        );
    }

    // Copy the incomplete bottom and right edges as complete opaque pixels.
    // This also handles either reversal without reading beyond the image.
    for output_y in full_rows..width {
        let source_x = if reverse_x {
            width - 1 - output_y
        } else {
            output_y
        };
        for output_x in 0..height {
            let source_y = if reverse_y {
                height - 1 - output_x
            } else {
                output_x
            };
            let source_start = (source_y * width + source_x) * CHANNELS;
            let output_start = (output_y * height + output_x) * CHANNELS;
            output[output_start..output_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }
    for output_y in 0..full_rows {
        let source_x = if reverse_x {
            width - 1 - output_y
        } else {
            output_y
        };
        for output_x in full_columns..height {
            let source_y = if reverse_y {
                height - 1 - output_x
            } else {
                output_x
            };
            let source_start = (source_y * width + source_x) * CHANNELS;
            let output_start = (output_y * height + output_x) * CHANNELS;
            output[output_start..output_start + CHANNELS]
                .copy_from_slice(&source[source_start..source_start + CHANNELS]);
        }
    }

    let tiles = (full_rows / TILE) * (full_columns / TILE);
    let scalar_pixels = width * height - full_rows * full_columns;
    Some(((tiles * TILE) as u64, (scalar_pixels * CHANNELS) as u64))
}

#[target_feature(enable = "avx2")]
unsafe fn transpose_full_tiles(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    full_rows: usize,
    full_columns: usize,
    reverse_x: bool,
    reverse_y: bool,
) {
    let source_stride = width * CHANNELS;
    let output_stride = height * CHANNELS;

    // Keep the existing cache-friendly order: an output-column band is held
    // while neighboring source-x tiles and their destination row spans run.
    // SAFETY: caller validates the exact byte lengths and full-tile bounds.
    unsafe {
        for output_x_start in (0..full_columns).step_by(TILE) {
            let source_y = if reverse_y {
                height - output_x_start - TILE
            } else {
                output_x_start
            };
            for output_y_start in (0..full_rows).step_by(TILE) {
                let source_x = if reverse_x {
                    width - output_y_start - TILE
                } else {
                    output_y_start
                };

                let mut rows = [_mm256_setzero_si256(); TILE];
                for row in 0..TILE {
                    let start = (source_y + row) * source_stride + source_x * CHANNELS;
                    rows[row] = _mm256_loadu_si256(source.as_ptr().add(start).cast());
                }

                let r01_lo = _mm256_unpacklo_epi32(rows[0], rows[1]);
                let r01_hi = _mm256_unpackhi_epi32(rows[0], rows[1]);
                let r23_lo = _mm256_unpacklo_epi32(rows[2], rows[3]);
                let r23_hi = _mm256_unpackhi_epi32(rows[2], rows[3]);
                let r45_lo = _mm256_unpacklo_epi32(rows[4], rows[5]);
                let r45_hi = _mm256_unpackhi_epi32(rows[4], rows[5]);
                let r67_lo = _mm256_unpacklo_epi32(rows[6], rows[7]);
                let r67_hi = _mm256_unpackhi_epi32(rows[6], rows[7]);

                let a0 = _mm256_unpacklo_epi64(r01_lo, r23_lo);
                let a1 = _mm256_unpackhi_epi64(r01_lo, r23_lo);
                let a2 = _mm256_unpacklo_epi64(r01_hi, r23_hi);
                let a3 = _mm256_unpackhi_epi64(r01_hi, r23_hi);
                let b0 = _mm256_unpacklo_epi64(r45_lo, r67_lo);
                let b1 = _mm256_unpackhi_epi64(r45_lo, r67_lo);
                let b2 = _mm256_unpacklo_epi64(r45_hi, r67_hi);
                let b3 = _mm256_unpackhi_epi64(r45_hi, r67_hi);

                let columns = [
                    _mm256_permute2x128_si256::<0x20>(a0, b0),
                    _mm256_permute2x128_si256::<0x20>(a1, b1),
                    _mm256_permute2x128_si256::<0x20>(a2, b2),
                    _mm256_permute2x128_si256::<0x20>(a3, b3),
                    _mm256_permute2x128_si256::<0x31>(a0, b0),
                    _mm256_permute2x128_si256::<0x31>(a1, b1),
                    _mm256_permute2x128_si256::<0x31>(a2, b2),
                    _mm256_permute2x128_si256::<0x31>(a3, b3),
                ];
                let reverse_lanes = _mm256_setr_epi32(7, 6, 5, 4, 3, 2, 1, 0);
                for (column_index, mut column) in columns.into_iter().enumerate() {
                    if reverse_y {
                        column = _mm256_permutevar8x32_epi32(column, reverse_lanes);
                    }
                    let row = if reverse_x {
                        TILE - 1 - column_index
                    } else {
                        column_index
                    };
                    let start = (output_y_start + row) * output_stride + output_x_start * CHANNELS;
                    _mm256_storeu_si256(output.as_mut_ptr().add(start).cast(), column);
                }
            }
        }
    }
}
