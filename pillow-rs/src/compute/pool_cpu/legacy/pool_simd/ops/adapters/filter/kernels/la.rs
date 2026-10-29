//! SIMD kernels and parity tests for native-LA Filter5x5.

use crate::compute::pool_simd::ops::native_filter::pack_u16x16;
use wide::{u8x16, u16x16};

#[inline(always)]
fn fill_horizontal_row(
    raw: &[u8],
    row_samples: usize,
    width: usize,
    vector_end: usize,
    source_y: usize,
    destination: &mut [u16],
) -> Option<()> {
    let row_start = source_y.checked_mul(row_samples)?;
    let row_end = row_start.checked_add(row_samples)?;
    let row = raw.get(row_start..row_end)?;
    let mut x = 2usize;
    while x < vector_end {
        let load = |tap: usize| -> Option<u16x16> {
            let source_x = x.checked_add(tap)?.checked_sub(2)?;
            let start = row_start.checked_add(source_x.checked_mul(2)?)?;
            let end = start.checked_add(16)?;
            let bytes: [u8; 16] = raw.get(start..end)?.try_into().ok()?;
            Some(u16x16::from(u8x16::new(bytes)))
        };
        let sum = load(0)? + load(1)? + load(2)? + load(3)? + load(4)?;
        let start = x.checked_mul(2)?;
        destination
            .get_mut(start..start.checked_add(16)?)?
            .copy_from_slice(&sum.to_array());
        x += 8;
    }
    while x < width - 2 {
        for channel in 0..2 {
            let mut sum = 0u16;
            for tap in 0..5 {
                sum += u16::from(row[(x + tap - 2) * 2 + channel]);
            }
            destination[x * 2 + channel] = sum;
        }
        x += 1;
    }
    Some(())
}

/// Filter native interleaved LA pixels for the one-extra-tap 5x5 kernel.
///
/// The exact byte result is `(sum + 12) / 25`, clipped to 255: the ordered
/// floating-point reference stays at least 0.02 from a half-byte boundary,
/// while its coefficient and FMA error is below 0.001. Each `u16x16` vector
/// covers eight LA pixels. Five rolling horizontal-sum rows feed the vertical
/// pass, and the two-pixel border remains copied from the source.
pub(crate) fn native_filter_5x5_single_outlier_la_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let row_samples = width.checked_mul(2)?;
    let sample_count = row_samples.checked_mul(height)?;
    if raw.len() != sample_count || out.len() != sample_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }

    let horizontal_blocks = (width - 4) / 8;
    let vector_end = 2 + horizontal_blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_samples]);
    for source_y in 0..5 {
        fill_horizontal_row(
            raw,
            row_samples,
            width,
            vector_end,
            source_y,
            &mut horizontal_rows[source_y],
        )?;
    }
    for y in 2..height - 2 {
        let row_ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let output_row = y.checked_mul(row_samples)?;
        let extra_row = (y - 2).checked_mul(row_samples)?;
        let mut x = 2usize;
        while x < vector_end {
            let start = x.checked_mul(2)?;
            let load = |row_id: usize| -> Option<u16x16> {
                Some(u16x16::new(
                    horizontal_rows[row_id]
                        .get(start..start.checked_add(16)?)?
                        .try_into()
                        .ok()?,
                ))
            };
            let extra_start = extra_row.checked_add((x + 2).checked_mul(2)?)?;
            let extra_end = extra_start.checked_add(16)?;
            let extra = u16x16::from(u8x16::new(
                raw.get(extra_start..extra_end)?.try_into().ok()?,
            ));
            let sum = load(row_ids[0])?
                + load(row_ids[1])?
                + load(row_ids[2])?
                + load(row_ids[3])?
                + load(row_ids[4])?
                + extra;
            let rounded = sum + u16x16::splat(12);
            let quotient = rounded.mul_keep_high(u16x16::splat(5_243)) >> 1u32;
            let packed = pack_u16x16(quotient.min(u16x16::splat(255))).to_array();
            let output_start = output_row.checked_add(start)?;
            out.get_mut(output_start..output_start.checked_add(16)?)?
                .copy_from_slice(&packed);
            x += 8;
        }
        while x < width - 2 {
            let output_start = output_row.checked_add(x.checked_mul(2)?)?;
            let extra_start = extra_row.checked_add((x + 2).checked_mul(2)?)?;
            for channel in 0..2 {
                let sample = x * 2 + channel;
                let sum = u32::from(horizontal_rows[row_ids[0]][sample])
                    + u32::from(horizontal_rows[row_ids[1]][sample])
                    + u32::from(horizontal_rows[row_ids[2]][sample])
                    + u32::from(horizontal_rows[row_ids[3]][sample])
                    + u32::from(horizontal_rows[row_ids[4]][sample])
                    + u32::from(raw[extra_start + channel]);
                out[output_start + channel] = ((sum + 12) / 25).min(255) as u8;
            }
            x += 1;
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(
                raw,
                row_samples,
                width,
                vector_end,
                next_source_y,
                &mut horizontal_rows[next_source_y % 5],
            )?;
        }
    }

    let interior_height = height - 4;
    let vector_blocks = horizontal_blocks.checked_mul(interior_height)?;
    let scalar_tail = ((width - 4) % 8).checked_mul(interior_height)?;
    Some((
        u64::try_from(vector_blocks).ok()?,
        u64::try_from(scalar_tail).ok()?,
    ))
}

#[cfg(test)]
mod la_single_outlier_tests {
    use super::native_filter_5x5_single_outlier_la_rows;
    use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
    use crate::raster::{DynamicImage, GrayAlphaImage};

    #[test]
    fn single_outlier_la_matches_reference_across_edges_tails_and_clipping() {
        let mut kernel = [1.0f32; 25];
        kernel[24] = 2.0;
        for (width, height) in [
            (1usize, 1usize),
            (4, 7),
            (7, 4),
            (5, 5),
            (7, 5),
            (13, 7),
            (19, 11),
            (71, 67),
        ] {
            for pattern in 0..4 {
                let mut state = 0x9e37_79b9u32;
                let raw: Vec<u8> = (0..width * height * 2)
                    .map(|index| match pattern {
                        0 => 0,
                        1 => 255,
                        2 => ((index * 73 + index / (width * 2) * 29 + 17) % 256) as u8,
                        _ => {
                            state ^= state << 13;
                            state ^= state >> 17;
                            state ^= state << 5;
                            state as u8
                        }
                    })
                    .collect();
                let image = DynamicImage::ImageLumaA8(
                    GrayAlphaImage::from_raw(width as u32, height as u32, raw.clone())
                        .expect("valid LA image"),
                );
                let expected = execute_filter5x5(&image, &kernel, 25.0, 0.0, Some("LA"))
                    .expect("CPU filter reference");
                let mut actual = raw.clone();
                let (vector_blocks, scalar_tail) =
                    native_filter_5x5_single_outlier_la_rows(&raw, &mut actual, width, height)
                        .expect("valid native LA dimensions");

                let interior_height = height.saturating_sub(4);
                let interior_width = width.saturating_sub(4);
                assert_eq!(
                    vector_blocks,
                    ((interior_width / 8) * interior_height) as u64,
                    "vector receipt {width}x{height}, pattern {pattern}"
                );
                assert_eq!(
                    scalar_tail,
                    ((interior_width % 8) * interior_height) as u64,
                    "tail receipt {width}x{height}, pattern {pattern}"
                );
                assert_eq!(
                    actual,
                    expected.as_bytes(),
                    "native LA {width}x{height}, pattern {pattern}"
                );
            }
        }
    }
}
