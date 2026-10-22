//! Native byte-layout 3x3 filter kernels.

use super::super::super::native_filter_load_byte_block;
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
use super::super::super::simd_pack_u16x16;
use wide::{f32x8, i32x8, u8x16, u16x8, u16x16};

/// Evaluate eight output samples of Pillow's 3x3 byte convolution in parallel.
///
/// The scalar CPU implementation starts each three-tap row with the middle
/// product and then uses fused multiply-adds for the left and right products.
/// Keep that order here so vector hardware does not change the observable
/// truncation boundary.  The final lane is clamped to the last interior pixel
/// when a row does not divide evenly by eight; callers only store valid lanes.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_filter_3x3_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut left = [0.0f32; 8];
        let mut middle = [0.0f32; 8];
        let mut right = [0.0f32; 8];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 2);
            let row = (y as isize + dy) as usize * width;
            let left_index = (row + x - 1) * channels;
            let middle_index = (row + x) * channels;
            let right_index = (row + x + 1) * channels;
            left[lane] = raw[left_index + channel] as f32;
            middle[lane] = raw[middle_index + channel] as f32;
            right[lane] = raw[right_index + channel] as f32;
        }
        let sum = f32x8::from(middle) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(left).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        f32x8::from(right).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Evaluate the rounded 3x3 byte smoothing kernel in eight integer lanes.
///
/// The weighted sum is in `0..=3315`, so `u16x8` cannot overflow. Since the
/// divisor is 13, `(sum + 6) / 13` is exactly nearest-integer rounding; this
/// preserves Pillow's intermediate byte while avoiding per-channel float
/// gathers for packed four-byte pixels.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_filter_3x3_pixel_block_integer(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_bytes: usize,
) -> [u8; 8] {
    debug_assert!((1..=4).contains(&channels));
    debug_assert!(active_bytes <= 8);
    debug_assert!(x_start >= 1 && x_start + active_bytes.div_ceil(channels) < width);

    let load = |start: usize| {
        u16x8::new(std::array::from_fn(|lane| {
            if lane < active_bytes {
                u16::from(raw[start + lane])
            } else {
                0
            }
        }))
    };
    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * channels;
        let left = source_row + (x_start - 1) * channels;
        let middle = left + channels;
        let right = middle + channels;
        (load(left), load(middle), load(right))
    };

    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    rounded.map(|value| (value / 13) as u8)
}

/// Evaluate one native channel across eight adjacent pixels using the exact
/// integer SMOOTH kernel. This avoids nine floating-point vector gathers for
/// L, LA, RGBA, and CMYK while preserving their distinct active-channel rules.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_filter_3x3_channel_block_integer(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
) -> f32x8 {
    debug_assert!((1..=4).contains(&channels));
    debug_assert!(channel < channels);
    debug_assert!((1..=8).contains(&active_pixels));
    debug_assert!(x_start >= 1 && x_start + active_pixels <= width - 1);

    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * channels;
        let load = |offset: isize| {
            u16x8::new(std::array::from_fn(|lane| {
                if lane < active_pixels {
                    let x = (x_start + lane) as isize + offset;
                    u16::from(raw[source_row + x as usize * channels + channel])
                } else {
                    0
                }
            }))
        };
        (load(-1), load(0), load(1))
    };

    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    f32x8::from(std::array::from_fn(|lane| (rounded[lane] / 13) as f32))
}

/// Load a contiguous sixteen-byte window for eight adjacent L samples, then
/// shuffle the left, center, and right taps into native vectors. This replaces
/// 24 scalar byte gathers per row with one contiguous load and three shuffles.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_filter_3x3_luma_block_integer(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
) -> Option<f32x8> {
    const LEFT: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 0, 0, 0, 0, 0, 0, 0];
    const CENTER: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0];
    const RIGHT: [u8; 16] = [2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 0, 0, 0, 0, 0];

    if width < 3 || x_start < 1 || x_start + 15 > width {
        return None;
    }
    let load_row = |row: usize| -> Option<(u16x8, u16x8, u16x8)> {
        let start = row.checked_mul(width)?.checked_add(x_start - 1)?;
        let window: [u8; 16] = raw.get(start..start.checked_add(16)?)?.try_into().ok()?;
        let window = u8x16::new(window);
        Some((
            u16x8::from_u8x16_low(window.swizzle_relaxed(u8x16::new(LEFT))),
            u16x8::from_u8x16_low(window.swizzle_relaxed(u8x16::new(CENTER))),
            u16x8::from_u8x16_low(window.swizzle_relaxed(u8x16::new(RIGHT))),
        ))
    };

    let (below_left, below_middle, below_right) = load_row(y + 1)?;
    let (center_left, center_middle, center_right) = load_row(y)?;
    let (above_left, above_middle, above_right) = load_row(y - 1)?;
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    Some(f32x8::from(std::array::from_fn(|lane| {
        (rounded[lane] / 13) as f32
    })))
}

/// Evaluate contiguous interleaved RGB channel bytes with the exact rounded
/// 3x3 smoothing kernel. Neighbor pixels are three bytes apart, so each lane
/// follows one channel through the left, center, and right taps without
/// scalar per-channel gathers.
#[inline]
pub(in crate::compute::pool_simd::ops::adapters) fn native_filter_3x3_rgb_byte_block_integer(
    raw: &[u8],
    width: usize,
    y: usize,
    start_byte: usize,
    active_bytes: usize,
) -> f32x8 {
    debug_assert!(width >= 3);
    debug_assert!((1..=8).contains(&active_bytes));
    debug_assert!(start_byte >= 3 && start_byte + active_bytes <= (width - 1) * 3);

    let load = |start: usize| {
        u16x8::new(std::array::from_fn(|lane| {
            if lane < active_bytes {
                u16::from(raw[start + lane])
            } else {
                0
            }
        }))
    };
    let row = |dy: isize| {
        let source_row = (y as isize + dy) as usize * width * 3;
        let middle = source_row + start_byte;
        (load(middle - 3), load(middle), load(middle + 3))
    };

    let (below_left, below_middle, below_right) = row(1);
    let (center_left, center_middle, center_right) = row(0);
    let (above_left, above_middle, above_right) = row(-1);
    let weighted = below_left
        + below_middle
        + below_right
        + center_left
        + center_middle * u16x8::splat(5)
        + center_right
        + above_left
        + above_middle
        + above_right;
    let rounded = (weighted + u16x8::splat(6)).to_array();
    f32x8::from(std::array::from_fn(|lane| (rounded[lane] / 13) as f32))
}

/// Evaluate a block of complete interleaved pixels in parallel.
///
/// The ordinary byte layouts are interleaved, so a vector over x positions
/// has to gather every sample one at a time.  For the two- and four-channel
/// layouts, grouping `8 / channels` adjacent pixels instead makes each tap a
/// contiguous byte span.  The active lanes retain the scalar kernel's
/// middle-first FMA order; inactive lanes are never stored.
#[inline]
pub(super) fn native_filter_3x3_pixel_block_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    y: usize,
    x_start: usize,
    active_pixels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    debug_assert!((1..=4).contains(&channels));
    debug_assert!(active_pixels <= 8 / channels.max(1));
    debug_assert!(x_start >= 1 && x_start + active_pixels <= width - 1);

    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let source_row = (y as isize + dy) as usize * width;
        let active_bytes = active_pixels * channels;
        let source_start = (source_row + x_start - 1) * channels;
        let left = native_filter_load_byte_block(raw, source_start, active_bytes);
        let middle = native_filter_load_byte_block(raw, source_start + channels, active_bytes);
        let right = native_filter_load_byte_block(raw, source_start + 2 * channels, active_bytes);
        let sum = middle * f32x8::splat(kernel[kernel_start + 1]);
        let sum = left.mul_add(f32x8::splat(kernel[kernel_start]), sum);
        right.mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Evaluate the separable binomial 3x3 convolution directly on packed LA
/// samples. The row ring keeps the horizontally weighted values as integers;
/// the 16-wide lanes cover eight adjacent LA pixels without widening the image
/// or gathering each channel independently.
#[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
pub(crate) fn native_filter_3x3_la_binomial_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let row_samples = width.checked_mul(2)?;
    let expected_len = row_samples.checked_mul(height)?;
    if raw.len() != expected_len || out.len() != expected_len {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }

    let mut horizontal_rows: [Vec<u16>; 3] = std::array::from_fn(|_| vec![0; row_samples]);
    let fill_horizontal_row = |source_y: usize, destination: &mut [u16]| {
        let row_start = source_y * row_samples;
        let mut x = 1usize;
        while x < width - 1 {
            let active_pixels = (width - 1 - x).min(8);
            let active_bytes = active_pixels * 2;
            let load = |pixel_x: usize| {
                let start = row_start + pixel_x * 2;
                let mut bytes = [0u8; 16];
                bytes[..active_bytes].copy_from_slice(&raw[start..start + active_bytes]);
                u16x16::from(u8x16::new(bytes))
            };
            let horizontal = load(x - 1) + load(x) * u16x16::splat(2) + load(x + 1);
            let values = horizontal.to_array();
            let output_start = x * 2;
            destination[output_start..output_start + active_bytes]
                .copy_from_slice(&values[..active_bytes]);
            x += active_pixels;
        }
    };

    for source_y in 0..3 {
        fill_horizontal_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 1..height - 1 {
        let above = &horizontal_rows[(y - 1) % 3];
        let center = &horizontal_rows[y % 3];
        let below = &horizontal_rows[(y + 1) % 3];
        let output_row = y * row_samples;
        let mut x = 1usize;
        while x < width - 1 {
            let active_pixels = (width - 1 - x).min(8);
            let active_bytes = active_pixels * 2;
            let sample_start = x * 2;
            let load = |row: &[u16]| {
                u16x16::new(std::array::from_fn(|lane| {
                    if lane < active_bytes {
                        row[sample_start + lane]
                    } else {
                        0
                    }
                }))
            };
            let weighted = load(above) + load(center) * u16x16::splat(2) + load(below);
            let rounded = (weighted + u16x16::splat(8)) >> 4u32;
            let bytes = simd_pack_u16x16(rounded).to_array();
            out[output_row + sample_start..output_row + sample_start + active_bytes]
                .copy_from_slice(&bytes[..active_bytes]);
            x += active_pixels;
        }

        let next_source_y = y + 2;
        if next_source_y < height {
            fill_horizontal_row(next_source_y, &mut horizontal_rows[next_source_y % 3]);
        }
    }
    Some((((width - 2).div_ceil(8) * (height - 2)) as u64, 0))
}

/// Apply the packed LA binomial kernel with one interleaved NEON load per
/// source row and eight output pixels. Lane shifts form horizontal neighbors;
/// integer widening preserves exact rounding and keeps both stored bands.
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[allow(unsafe_code)]
pub(crate) fn native_filter_3x3_la_binomial_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    use core::arch::aarch64 as neon;

    let row_samples = width.checked_mul(2)?;
    let expected_len = row_samples.checked_mul(height)?;
    if raw.len() != expected_len || out.len() != expected_len {
        return None;
    }
    if width < 3 || height < 3 {
        return Some((0, 0));
    }

    let horizontal = |source_y: usize, x: usize| {
        // SAFETY: `raw.len() == row_samples * height` was checked above,
        // `source_y` is one of the valid filter rows, and the vector-loop
        // condition `x + 14 < width` keeps this 16-pixel LA load, starting at
        // `x - 1`, within that row and therefore within `raw`.
        let samples =
            unsafe { neon::vld2q_u8(raw.as_ptr().add(source_y * row_samples + (x - 1) * 2)) };
        let filter_channel = |samples: neon::uint8x16_t| unsafe {
            let low = neon::vget_low_u8(samples);
            let high = neon::vget_high_u8(samples);
            let left = low;
            let center = neon::vext_u8(low, high, 1);
            let right = neon::vext_u8(low, high, 2);
            neon::vaddq_u16(neon::vaddl_u8(left, right), neon::vaddl_u8(center, center))
        };
        (filter_channel(samples.0), filter_channel(samples.1))
    };

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 1..height - 1 {
        let output_row = y * row_samples;
        let mut x = 1usize;
        while x + 14 < width {
            let above = horizontal(y - 1, x);
            let center = horizontal(y, x);
            let below = horizontal(y + 1, x);
            let filter_vertical = |above: neon::uint16x8_t,
                                   center: neon::uint16x8_t,
                                   below: neon::uint16x8_t| unsafe {
                let sum = neon::vaddq_u16(
                    neon::vaddq_u16(above, below),
                    neon::vaddq_u16(center, center),
                );
                let rounded = neon::vshrq_n_u16(neon::vaddq_u16(sum, neon::vdupq_n_u16(8)), 4);
                neon::vqmovn_u16(rounded)
            };
            let output = neon::uint8x8x2_t(
                filter_vertical(above.0, center.0, below.0),
                filter_vertical(above.1, center.1, below.1),
            );
            // SAFETY: `x + 14 < width` guarantees all eight output pixels
            // starting at x are interior pixels within this row. The checked
            // output length covers every row, and `out` is disjoint from `raw`.
            unsafe {
                neon::vst2_u8(out.as_mut_ptr().add(output_row + x * 2), output);
            }
            x += 8;
            vector_blocks += 1;
        }

        for pixel_x in x..width - 1 {
            for channel in 0..2 {
                let horizontal_sum = |source_y: usize| {
                    let center = source_y * row_samples + pixel_x * 2 + channel;
                    u16::from(raw[center - 2])
                        + u16::from(raw[center]) * 2
                        + u16::from(raw[center + 2])
                };
                let sum = u32::from(horizontal_sum(y - 1))
                    + u32::from(horizontal_sum(y)) * 2
                    + u32::from(horizontal_sum(y + 1));
                out[output_row + pixel_x * 2 + channel] = ((sum + 8) >> 4) as u8;
            }
            scalar_tail += 2;
        }
    }
    Some((vector_blocks, scalar_tail))
}

/// Apply the exact native-byte 3x3 convolution with eight-wide vector lanes.
/// Borders retain the source bytes, matching the CPU implementation.
pub(in crate::compute::pool_simd::ops::adapters::filter) fn native_filter_3x3_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    native_filter_3x3_rows_active(
        raw,
        out,
        width,
        height,
        channels,
        channels,
        kernel,
        rounding_bias,
    );
}

/// Load a bounded RGB byte-stream block through SIMD widening. The final
/// partial block is zero-padded; only its active lanes are stored.
#[inline]
fn native_filter_load_rgb_byte_stream_block(
    raw: &[u8],
    start: usize,
    active_bytes: usize,
) -> f32x8 {
    debug_assert!(active_bytes <= 8);
    let mut padded = [0u8; 16];
    padded[..active_bytes].copy_from_slice(&raw[start..start + active_bytes]);
    let widened = u16x16::from(u8x16::new(padded));
    let [low, _high]: [u16x8; 2] = bytemuck::cast(widened);
    f32x8::from_i32x8(i32x8::from_u16x8(low))
}

/// Evaluate eight consecutive RGB channel bytes with the ordinary 3x3 math.
///
/// RGB uses one kernel for all three channels, so an eight-byte vector may
/// start or end within a pixel. A horizontal neighbor is exactly three bytes
/// away, which keeps each lane on the same channel without deinterleaving.
#[inline]
fn native_filter_3x3_rgb_byte_stream_vector(
    raw: &[u8],
    row_stride: usize,
    y: usize,
    output_byte_start: usize,
    active_bytes: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    debug_assert!(active_bytes <= 8);
    debug_assert!(output_byte_start >= 3);
    debug_assert!(output_byte_start + active_bytes <= row_stride - 3);

    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let source_start = (y as isize + dy) as usize * row_stride + output_byte_start;
        let left = native_filter_load_rgb_byte_stream_block(raw, source_start - 3, active_bytes);
        let middle = native_filter_load_rgb_byte_stream_block(raw, source_start, active_bytes);
        let right = native_filter_load_rgb_byte_stream_block(raw, source_start + 3, active_bytes);
        let sum = middle * f32x8::splat(kernel[kernel_start + 1]);
        let sum = left.mul_add(f32x8::splat(kernel[kernel_start]), sum);
        right.mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Apply a native RGB 3x3 kernel as one contiguous stream of channel bytes.
///
/// The stream covers only the interior pixels: three border bytes remain at
/// each end of every row. Every lane reads its horizontal neighbors at -3/+3
/// bytes, preserving channel identity while removing per-channel gathers and
/// strided output scatters from the generic row loop.
pub(in crate::compute::pool_simd::ops::adapters::filter) fn native_filter_3x3_rgb_byte_stream(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> Option<u64> {
    let row_stride = width.checked_mul(3)?;
    let expected_len = row_stride.checked_mul(height)?;
    if width < 3 || height < 3 || raw.len() != expected_len || out.len() != expected_len {
        return None;
    }
    let interior_bytes = row_stride - 6;
    let blocks_per_row = interior_bytes.div_ceil(8);
    let interior_height = height - 2;
    let apply_row = |y: usize, output_row: &mut [u8]| {
        let mut output_byte = 3usize;
        let interior_end = row_stride - 3;
        while output_byte < interior_end {
            let active_bytes = (interior_end - output_byte).min(8);
            let values = native_filter_3x3_rgb_byte_stream_vector(
                raw,
                row_stride,
                y,
                output_byte,
                active_bytes,
                kernel,
                rounding_bias,
            );
            output_row[output_byte..output_byte + active_bytes]
                .copy_from_slice(&values[..active_bytes]);
            output_byte += active_bytes;
        }
    };

    #[cfg(feature = "parallel")]
    super::super::super::simd_rows_mut_serial!(
        out,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            if (1..height - 1).contains(&y) {
                apply_row(y, row);
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }

    Some((blocks_per_row * interior_height) as u64)
}

/// Apply a native-byte 3x3 convolution to only the active channels.
///
/// LA/RGBA Sharpness uses this form because Pillow smooths the RGB samples
/// while copying alpha unchanged. CMYK passes all four stored samples as
/// active channels. The vector arithmetic and scalar interior tail are shared
/// with the ordinary Filter3x3 path.
fn native_filter_3x3_rows_active(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> (u64, u64) {
    if !(1..=4).contains(&channels)
        || active_channels == 0
        || active_channels > channels
        || width < 3
        || height < 3
    {
        return (0, 0);
    }
    let interior_width = width - 2;
    let interior_height = height - 2;
    let grouped_pixels = active_channels == channels && matches!(channels, 2 | 4);
    let pixel_block = (8 / channels).max(1);
    let vector_blocks = if grouped_pixels {
        interior_width
            .div_ceil(pixel_block)
            .saturating_mul(interior_height)
    } else {
        interior_width
            .div_ceil(8)
            .saturating_mul(active_channels)
            .saturating_mul(interior_height)
    };
    let scalar_tail = 0;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        if grouped_pixels {
            let mut x = 1usize;
            while x < width - 1 {
                let active_pixels = (width - 1 - x).min(pixel_block);
                let values = native_filter_3x3_pixel_block_vector(
                    raw,
                    width,
                    channels,
                    y,
                    x,
                    active_pixels,
                    kernel,
                    rounding_bias,
                );
                for pixel in 0..active_pixels {
                    for channel in 0..channels {
                        row[(x + pixel) * channels + channel] = values[pixel * channels + channel];
                    }
                }
                x += active_pixels;
            }
        } else {
            for channel in 0..active_channels {
                let mut x = 1usize;
                while x < width - 1 {
                    let active = (width - 1 - x).min(8);
                    let values = native_filter_3x3_vector(
                        raw,
                        width,
                        channels,
                        channel,
                        y,
                        x,
                        kernel,
                        rounding_bias,
                    );
                    for (lane, value) in values.into_iter().enumerate().take(active) {
                        row[(x + lane) * channels + channel] = value;
                    }
                    x += active;
                }
            }
        }
    };

    #[cfg(feature = "parallel")]
    super::super::super::simd_rows_mut_serial!(
        out,
        row_stride,
        height,
        |_row_start, _row_end, y, row| {
            let y = y as usize;
            if (1..height - 1).contains(&y) {
                apply_row(y, row);
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
    (vector_blocks as u64, scalar_tail as u64)
}
