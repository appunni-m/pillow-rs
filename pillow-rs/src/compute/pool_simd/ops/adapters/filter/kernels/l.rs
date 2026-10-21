//! SIMD kernels and parity tests for native-L Filter5x5.

use wide::{u8x16, u16x8};

/// Native-L uniform 5x5 filter with exact integer row sums. The CPU helper
/// documents why the ordered f32 reference rounds to `(sum + 12) / 25`.
/// The bounded numerator is at most 6,387; high-half multiplication by 5,243
/// followed by a one-bit shift divides it by 25 exactly in every SIMD lane.
pub(crate) fn native_filter_5x5_uniform_luma_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    if raw.len() != pixel_count || out.len() != pixel_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }
    let blocks = (width - 4) / 8;
    let vector_end = 2 + blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    let fill_row = |source_y: usize, destination: &mut [u16]| {
        let row = &raw[source_y * width..(source_y + 1) * width];
        let mut x = 2;
        while x < vector_end {
            let mut padded = [0u8; 16];
            padded[..12].copy_from_slice(&row[x - 2..x + 10]);
            let window = u8x16::new(padded);
            let tap = |offset: u8| {
                let indices = u8x16::new(std::array::from_fn(|lane| {
                    if lane < 8 { lane as u8 + offset } else { 0 }
                }));
                u16x8::from_u8x16_low(window.swizzle_relaxed(indices))
            };
            let sum = tap(0) + tap(1) + tap(2) + tap(3) + tap(4);
            destination[x..x + 8].copy_from_slice(&sum.to_array());
            x += 8;
        }
        while x < width - 2 {
            destination[x] = row[x - 2..x + 3]
                .iter()
                .map(|&sample| u16::from(sample))
                .sum();
            x += 1;
        }
    };
    for source_y in 0..5 {
        fill_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 2..height - 2 {
        let ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let load = |row_id: usize, x: usize| {
            u16x8::new(horizontal_rows[row_id][x..x + 8].try_into().unwrap())
        };
        let mut x = 2;
        while x < vector_end {
            let sum = load(ids[0], x)
                + load(ids[1], x)
                + load(ids[2], x)
                + load(ids[3], x)
                + load(ids[4], x);
            let quotient = (sum + u16x8::splat(12)).mul_keep_high(u16x8::splat(5243)) >> 1u32;
            let bytes = quotient.to_array().map(|value| value as u8);
            out[y * width + x..y * width + x + 8].copy_from_slice(&bytes);
            x += 8;
        }
        while x < width - 2 {
            let sum = horizontal_rows[ids[0]][x]
                + horizontal_rows[ids[1]][x]
                + horizontal_rows[ids[2]][x]
                + horizontal_rows[ids[3]][x]
                + horizontal_rows[ids[4]][x];
            out[y * width + x] = ((sum + 12) / 25) as u8;
            x += 1;
        }
        let next_y = y + 3;
        if next_y < height {
            fill_row(next_y, &mut horizontal_rows[next_y % 5]);
        }
    }
    Some((
        (blocks * (height - 4)) as u64,
        (((width - 4) % 8) * (height - 4)) as u64,
    ))
}

/// Filter a native L image with the exact 5x5 binomial kernel. Horizontal
/// weighted rows are kept in a five-row ring so each source row is loaded and
/// filtered once. A 16-byte vector load plus five fixed shuffles supplies the
/// eight adjacent output lanes without per-lane scalar gathers. Horizontal
/// sums fit in 12 bits and the final weighted sum is at most 65,280.
pub(crate) fn native_filter_5x5_binomial_luma_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    if raw.len() != pixel_count || out.len() != pixel_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }

    let horizontal_blocks = (width - 4) / 8;
    let vector_end = 2 + horizontal_blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| -> Option<()> {
        let row_start = source_y.checked_mul(width)?;
        let mut x = 2usize;
        while x < vector_end {
            let base = row_start.checked_add(x.checked_sub(2)?)?;
            let window_bytes = raw.get(base..base.checked_add(12)?)?;
            let mut padded = [0u8; 16];
            padded[..12].copy_from_slice(window_bytes);
            let window = u8x16::new(padded);
            let tap = |offset: u8| {
                let indices = u8x16::new(std::array::from_fn(|lane| {
                    if lane < 8 { lane as u8 + offset } else { 0 }
                }));
                u16x8::from_u8x16_low(window.swizzle_relaxed(indices))
            };
            let weighted = tap(0)
                + tap(1) * u16x8::splat(4)
                + tap(2) * u16x8::splat(6)
                + tap(3) * u16x8::splat(4)
                + tap(4);
            destination
                .get_mut(x..x + 8)?
                .copy_from_slice(&weighted.to_array());
            x += 8;
        }
        while x < width - 2 {
            destination[x] = u16::from(raw[row_start + x - 2])
                + u16::from(raw[row_start + x - 1]) * 4
                + u16::from(raw[row_start + x]) * 6
                + u16::from(raw[row_start + x + 1]) * 4
                + u16::from(raw[row_start + x + 2]);
            x += 1;
        }
        Some(())
    };

    for source_y in 0..5 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y])?;
    }
    for y in 2..height - 2 {
        let row_ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let output_row = y.checked_mul(width)?;
        let mut x = 2usize;
        while x < vector_end {
            let load = |row_id: usize| -> Option<u16x8> {
                Some(u16x8::new(
                    horizontal_rows[row_id].get(x..x + 8)?.try_into().ok()?,
                ))
            };
            let weighted = load(row_ids[0])?
                + load(row_ids[1])? * u16x8::splat(4)
                + load(row_ids[2])? * u16x8::splat(6)
                + load(row_ids[3])? * u16x8::splat(4)
                + load(row_ids[4])?;
            let rounded = (weighted + u16x8::splat(128)).to_array();
            let bytes = rounded.map(|value| (value >> 8) as u8);
            out.get_mut(output_row + x..output_row + x + 8)?
                .copy_from_slice(&bytes);
            x += 8;
        }
        while x < width - 2 {
            let sum = u32::from(horizontal_rows[row_ids[0]][x])
                + u32::from(horizontal_rows[row_ids[1]][x]) * 4
                + u32::from(horizontal_rows[row_ids[2]][x]) * 6
                + u32::from(horizontal_rows[row_ids[3]][x]) * 4
                + u32::from(horizontal_rows[row_ids[4]][x]);
            out[output_row + x] = ((sum + 128) >> 8) as u8;
            x += 1;
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 5])?;
        }
    }

    let interior_height = height - 4;
    Some((
        (horizontal_blocks * interior_height) as u64,
        ((width - 4) % 8 * interior_height) as u64,
    ))
}

#[cfg(test)]
mod tests {
    use crate::compute::pool_simd::ops::adapters::simd_filter_5x5;

    #[test]
    fn simd_binomial_5x5_luma_matches_cpu_across_vector_tails_and_borders() {
        use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
        let kernel = std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
        let operation = PipelineOp::Filter5x5 {
            kernel,
            scale: 256.0,
            offset: 0.0,
        };
        for (width, height) in [(5u32, 5u32), (11, 7), (12, 8), (19, 11), (20, 13)] {
            let raw: Vec<u8> = (0..width as usize * height as usize)
                .map(|index| ((index * 73 + index / width as usize * 29 + 17) % 256) as u8)
                .collect();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, raw).expect("valid L image dimensions"),
            );
            let expected = execute_filter5x5(&image, &kernel, 256.0, 0.0, Some("L"))
                .expect("CPU 5x5 binomial filter");
            let actual =
                simd_filter_5x5(&image, &operation, Some("L")).expect("SIMD 5x5 binomial filter");
            assert!(matches!(actual, DynamicImage::ImageLuma8(_)));
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
        }
    }

    #[test]
    fn simd_uniform_5x5_magic_divide_matches_every_bounded_numerator() {
        use wide::u16x8;

        for first in (12usize..=6387).step_by(8) {
            let numerators =
                u16x8::new(std::array::from_fn(|lane| (first + lane).min(6387) as u16));
            let quotients = (numerators.mul_keep_high(u16x8::splat(5243)) >> 1u32).to_array();
            for (numerator, quotient) in numerators.to_array().into_iter().zip(quotients) {
                assert_eq!(
                    quotient as usize,
                    numerator as usize / 25,
                    "rounded numerator {numerator}"
                );
            }
        }
    }

    #[test]
    fn simd_uniform_5x5_luma_matches_cpu_across_vector_tails_and_borders() {
        use crate::compute::pool_cpu::ops::filter::execute_filter5x5;
        use crate::pipeline::PipelineOp;
        use crate::raster::{DynamicImage, GrayImage};

        let kernel = [1.0; 25];
        let operation = PipelineOp::Filter5x5 {
            kernel,
            scale: 25.0,
            offset: 0.0,
        };
        for (width, height) in [
            (5u32, 5u32),
            (13, 7),
            (12, 8),
            (19, 11),
            (20, 13),
            (4, 7),
            (7, 4),
            (31, 17),
        ] {
            let raw = (0..width as usize * height as usize)
                .map(|index| {
                    ((index.wrapping_mul(73) + index / width as usize * 29 + 17) % 256) as u8
                })
                .collect::<Vec<_>>();
            let image = DynamicImage::ImageLuma8(
                GrayImage::from_raw(width, height, raw).expect("valid native L image"),
            );
            let expected = execute_filter5x5(&image, &kernel, 25.0, 0.0, Some("L"))
                .expect("CPU uniform 5x5 filter");
            let actual =
                simd_filter_5x5(&image, &operation, Some("L")).expect("SIMD uniform 5x5 filter");

            assert!(matches!(actual, DynamicImage::ImageLuma8(_)));
            assert_eq!(actual.as_bytes(), expected.as_bytes(), "{width}x{height}");
        }
    }
}
