//! Shared native SIMD neighborhood kernels used by backend dispatchers.
//!
//! Kernels live outside the general adapter registry so CPU and SIMD entry
//! points can reuse one implementation without growing `adapters.rs`.

use wide::{f32x8, i16x8, u8x16, u16x16};

#[inline]
fn pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [wide::u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}

/// Filter native interleaved LA pixels with the exact binomial kernel. One
/// `u16x16` vector spans eight complete pixels, so the same arithmetic lanes
/// independently accumulate luminance and alpha without a mode conversion.
pub(crate) fn native_filter_5x5_binomial_la_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    let sample_count = pixel_count.checked_mul(2)?;
    if raw.len() != sample_count || out.len() != sample_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }

    const WEIGHTS: [u16; 5] = [1, 4, 6, 4, 1];
    let row_samples = width.checked_mul(2)?;
    let horizontal_blocks = (width - 4) / 8;
    let vector_end = 2 + horizontal_blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_samples]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| -> Option<()> {
        let row_start = source_y.checked_mul(row_samples)?;
        let mut x = 2usize;
        while x < vector_end {
            let load = |tap: usize| -> Option<u16x16> {
                let source_x = x.checked_add(tap)?.checked_sub(2)?;
                let start = row_start.checked_add(source_x.checked_mul(2)?)?;
                let end = start.checked_add(16)?;
                let bytes: [u8; 16] = raw.get(start..end)?.try_into().ok()?;
                Some(u16x16::from(u8x16::new(bytes)))
            };
            let outer = load(0)? + load(4)?;
            let inner = load(1)? + load(3)?;
            let weighted =
                outer + inner * u16x16::splat(WEIGHTS[1]) + load(2)? * u16x16::splat(WEIGHTS[2]);
            let start = x.checked_mul(2)?;
            let samples = weighted.to_array();
            destination
                .get_mut(start..start.checked_add(16)?)?
                .copy_from_slice(&samples);
            x += 8;
        }
        while x < width - 2 {
            let base = row_start + x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                let outer = u16::from(raw[sample - 4]) + u16::from(raw[sample + 4]);
                let inner = u16::from(raw[sample - 2]) + u16::from(raw[sample + 2]);
                destination[x * 2 + channel] =
                    outer + inner * WEIGHTS[1] + u16::from(raw[sample]) * WEIGHTS[2];
            }
            x += 1;
        }
        Some(())
    };

    for source_y in 0..5 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y])?;
    }
    for y in 2..height - 2 {
        let row_ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let output_row = y.checked_mul(row_samples)?;
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
            let outer = load(row_ids[0])? + load(row_ids[4])?;
            let inner = load(row_ids[1])? + load(row_ids[3])?;
            let weighted = outer
                + inner * u16x16::splat(WEIGHTS[1])
                + load(row_ids[2])? * u16x16::splat(WEIGHTS[2]);
            let packed = pack_u16x16((weighted + u16x16::splat(128)) >> 8u32).to_array();
            out.get_mut(output_row + start..output_row + start + 16)?
                .copy_from_slice(&packed);
            x += 8;
        }
        while x < width - 2 {
            let base = x * 2;
            for channel in 0..2 {
                let sample = base + channel;
                let outer = u32::from(horizontal_rows[row_ids[0]][sample])
                    + u32::from(horizontal_rows[row_ids[4]][sample]);
                let inner = u32::from(horizontal_rows[row_ids[1]][sample])
                    + u32::from(horizontal_rows[row_ids[3]][sample]);
                let center = u32::from(horizontal_rows[row_ids[2]][sample]);
                let sum = outer + inner * u32::from(WEIGHTS[1]) + center * u32::from(WEIGHTS[2]);
                out[output_row + sample] = ((sum + 128) >> 8) as u8;
            }
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

/// Evaluate eight I-mode samples of a 5x5 kernel in parallel, preserving the
/// same reversed-row and middle-first FMA order as Pillow's CPU path.
#[inline]
fn native_filter_5x5_i32_vector(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [i32; 8] {
    if x_start + 8 <= width - 2 {
        let row = |dy: isize, kernel_start: usize| -> f32x8 {
            let source_y = (y as isize + dy) as usize;
            let first_pixel = source_y * width + x_start - 2;
            let first_start = first_pixel * 4;
            let second_start = (first_pixel + 4) * 4;
            let first_pixels: [i32; 8] =
                bytemuck::pod_read_unaligned(&raw[first_start..first_start + 8 * 4]);
            let second_pixels: [i32; 8] =
                bytemuck::pod_read_unaligned(&raw[second_start..second_start + 8 * 4]);
            let first = first_pixels.map(|pixel| i32::from_le(pixel) as f32);
            let second = second_pixels.map(|pixel| i32::from_le(pixel) as f32);
            let taps: [f32x8; 5] = std::array::from_fn(|tap| {
                f32x8::from(std::array::from_fn(|lane| {
                    let sample_index = tap + lane;
                    if sample_index < 8 {
                        first[sample_index]
                    } else {
                        second[sample_index - 4]
                    }
                }))
            });
            let sum = taps[1] * f32x8::splat(kernel[kernel_start + 1]);
            let sum = taps[0].mul_add(f32x8::splat(kernel[kernel_start]), sum);
            let sum = taps[2].mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
            let sum = taps[3].mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
            taps[4].mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
        };

        let mut total = f32x8::splat(rounding_bias);
        total += row(2, 0);
        total += row(1, 5);
        total += row(0, 10);
        total += row(-1, 15);
        total += row(-2, 20);
        let values = total.to_array();
        return std::array::from_fn(|lane| {
            if values[lane] >= 0.0 {
                values[lane] as i32
            } else {
                0
            }
        });
    }

    // Keep the short final block scalar-gathered: contiguous eight-lane loads
    // would cross the row edge even though only a few output lanes are live.
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [[0.0f32; 8]; 5];
        let source_y = (y as isize + dy) as usize;
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 3);
            for tap in 0..5 {
                let source_x = x + tap - 2;
                let base = (source_y * width + source_x) * 4;
                samples[tap][lane] =
                    i32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]])
                        as f32;
            }
        }
        let sum = f32x8::from(samples[1]) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(samples[0]).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = f32x8::from(samples[2]).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = f32x8::from(samples[3]).mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        f32x8::from(samples[4]).mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(2, 0);
    total += row(1, 5);
    total += row(0, 10);
    total += row(-1, 15);
    total += row(-2, 20);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        if values[lane] >= 0.0 {
            values[lane] as i32
        } else {
            0
        }
    })
}

pub(crate) fn native_filter_5x5_i32_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    let byte_count = pixel_count.checked_mul(4)?;
    if raw.len() != byte_count || out.len() != byte_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }
    let interior_width = width - 4;
    let interior_height = height - 4;
    let vector_blocks = interior_width.div_ceil(8).saturating_mul(interior_height);
    let row_stride = width * 4;
    let apply_row = |y: usize, destination: &mut [u8]| {
        let mut x = 2usize;
        while x < width - 2 {
            let active = (width - 2 - x).min(8);
            let values = native_filter_5x5_i32_vector(raw, width, y, x, kernel, rounding_bias);
            for (lane, value) in values.into_iter().enumerate().take(active) {
                let base = (x + lane) * 4;
                destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += active;
        }
    };

    // SIMD work remains row-serial even in a build that also enables the
    // separate Parallel CPU profile.
    for y in 2..height - 2 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
    Some((vector_blocks as u64, 0))
}
