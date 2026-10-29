//! AVX2 lane arithmetic for exact native byte Reduce strategies.

#![allow(unsafe_code)] // Mode strategies select this only after AVX2 capability checks.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use super::super::general::ByteReduceContext;
#[cfg(target_arch = "x86")]
use std::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

const LANES: usize = 8;
const LANES_U32: u32 = 8;
const RGB_BYTES: usize = 3;

#[inline]
fn clamp_u8(value: u32) -> u8 {
    u8::try_from(value.min(u32::from(u8::MAX))).unwrap_or(u8::MAX)
}

#[target_feature(enable = "avx2")]
unsafe fn average_u32x8(
    sums: [u32; LANES],
    multipliers: [u32; LANES],
    amends: [u32; LANES],
) -> [u32; LANES] {
    let mut output = [0u32; LANES];
    // SAFETY: all three inputs and the destination contain eight u32 lanes;
    // unaligned AVX2 loads/stores read and write exactly those 32-byte arrays.
    unsafe {
        let sum = arch::_mm256_loadu_si256(sums.as_ptr().cast());
        let multiplier = arch::_mm256_loadu_si256(multipliers.as_ptr().cast());
        let amend = arch::_mm256_loadu_si256(amends.as_ptr().cast());
        let rounded = arch::_mm256_add_epi32(sum, amend);
        let scaled = arch::_mm256_mullo_epi32(rounded, multiplier);
        let average = arch::_mm256_srli_epi32::<24>(scaled);
        arch::_mm256_storeu_si256(output.as_mut_ptr().cast(), average);
    }
    output
}

#[target_feature(enable = "avx2")]
unsafe fn average_2x2_u32x8(sums: [u32; LANES]) -> [u32; LANES] {
    let mut output = [0u32; LANES];
    // SAFETY: both arrays contain eight u32 lanes; unaligned AVX2 loads/stores
    // access exactly those arrays, and this function requires AVX2.
    unsafe {
        let sum = arch::_mm256_loadu_si256(sums.as_ptr().cast());
        let rounded = arch::_mm256_add_epi32(sum, arch::_mm256_set1_epi32(2));
        let average = arch::_mm256_srli_epi32::<2>(rounded);
        arch::_mm256_storeu_si256(output.as_mut_ptr().cast(), average);
    }
    output
}

fn rgb_3x5_pixel_sums(
    source: &[u8],
    width: usize,
    source_x: usize,
    source_y: usize,
) -> [u32; RGB_BYTES] {
    let mut sums = [0u32; RGB_BYTES];
    let mut source_row = source_y * width * RGB_BYTES + source_x * RGB_BYTES;
    for _ in 0..5 {
        let mut source_index = source_row;
        for _ in 0..3 {
            sums[0] += u32::from(source[source_index]);
            sums[1] += u32::from(source[source_index + 1]);
            sums[2] += u32::from(source[source_index + 2]);
            source_index += RGB_BYTES;
        }
        source_row += width * RGB_BYTES;
    }
    sums
}

#[target_feature(enable = "avx2")]
unsafe fn reduce_rgb_2x2_block(
    source: &[u8],
    width: usize,
    output_x: usize,
    output_y: usize,
    output: &mut [u8],
) {
    let source_x = output_x * 6;
    let source_top = output_y * 2 * width * RGB_BYTES + source_x;
    let source_bottom = source_top + width * RGB_BYTES;
    let channel_sums = |channel| {
        std::array::from_fn(|lane| {
            let relative_index = lane * 6 + channel;
            let source_index = source_top + relative_index;
            u32::from(source[source_index])
                + u32::from(source[source_index + 3])
                + u32::from(source[source_bottom + relative_index])
                + u32::from(source[source_bottom + relative_index + 3])
        })
    };
    let averages: [[u8; LANES]; RGB_BYTES] = std::array::from_fn(|channel| {
        // SAFETY: this target-feature function supplies exactly eight sums.
        unsafe { average_2x2_u32x8(channel_sums(channel)) }.map(clamp_u8)
    });
    for lane in 0..LANES {
        let destination = lane * RGB_BYTES;
        output[destination] = averages[0][lane];
        output[destination + 1] = averages[1][lane];
        output[destination + 2] = averages[2][lane];
    }
}

#[target_feature(enable = "avx2")]
unsafe fn reduce_rgb_3x5_block(
    source: &[u8],
    width: usize,
    output_x: usize,
    output_y: usize,
    output: &mut [u8],
) {
    const MULTIPLIER: u32 = ((1u128 << 32) / (15 * 256)) as u32;
    const AMEND: u32 = 7;
    let source_x = output_x * 3;
    let source_y = output_y * 5;
    let mut channel_sums = [[0u32; LANES]; RGB_BYTES];
    for lane in 0..LANES {
        let sums = rgb_3x5_pixel_sums(source, width, source_x + lane * 3, source_y);
        for channel in 0..RGB_BYTES {
            channel_sums[channel][lane] = sums[channel];
        }
    }
    let averages: [[u8; LANES]; RGB_BYTES] = std::array::from_fn(|channel| {
        let multipliers = [MULTIPLIER; LANES];
        let amends = [AMEND; LANES];
        // SAFETY: this target-feature function supplies eight bounded sums.
        unsafe { average_u32x8(channel_sums[channel], multipliers, amends) }.map(clamp_u8)
    });
    for lane in 0..LANES {
        let destination = lane * RGB_BYTES;
        output[destination] = averages[0][lane];
        output[destination + 1] = averages[1][lane];
        output[destination + 2] = averages[2][lane];
    }
}

#[target_feature(enable = "avx2")]
unsafe fn write_special_rgb_vector<const CHANNELS: usize, const PREMULTIPLIED: bool, const RGB_3X5: bool>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    factors: (u32, u32),
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> bool {
    let (main_width, main_height) = context.main_dimensions();
    let full_rgb_vector = CHANNELS == 3
        && !PREMULTIPLIED
        && valid_pixels == LANES
        && main_width >= LANES_U32
        && output_x <= main_width - LANES_U32
        && output_y < main_height;
    if full_rgb_vector && factors == (2, 2) {
        // SAFETY: dispatch selected AVX2; eight complete RGB 2x2 blocks fit
        // in the source image and the destination has exactly 24 output bytes.
        unsafe {
            reduce_rgb_2x2_block(
                context.source(),
                context.source_width() as usize,
                output_x as usize,
                output_y as usize,
                output,
            );
        }
        return true;
    }
    if full_rgb_vector && RGB_3X5 && factors == (3, 5) {
        // SAFETY: the full-vector condition proves all eight RGB 3x5 blocks fit.
        unsafe {
            reduce_rgb_3x5_block(
                context.source(),
                context.source_width() as usize,
                output_x as usize,
                output_y as usize,
                output,
            );
        }
        return true;
    }
    false
}

#[target_feature(enable = "avx2")]
unsafe fn write_generic_vector<const CHANNELS: usize, const PREMULTIPLIED: bool>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    let (main_width, _) = context.main_dimensions();
    let full_width_batch = valid_pixels != 0
        && output_x < main_width
        && valid_pixels as u32 <= main_width - output_x;
    let mut sums = [[0u32; LANES]; 4];
    let mut multipliers = [0u32; LANES];
    let mut amends = [0u32; LANES];
    if full_width_batch {
        let (multiplier, amend) = context.full_width_rounding(output_y).ok_or_else(|| {
            PilError::InternalError("Reduce AVX2 full-row rounding facts were unavailable".into())
        })?;
        for lane in 0..valid_pixels {
            let pixel_sums = context
                .full_width_sums(output_x + lane as u32, output_y)
                .ok_or_else(|| {
                    PilError::InternalError("Reduce AVX2 full-row bounds were invalid".into())
                })?;
            for channel in 0..CHANNELS {
                sums[channel][lane] = pixel_sums[channel];
            }
            multipliers[lane] = multiplier;
            amends[lane] = amend;
        }
    } else {
        for lane in 0..valid_pixels {
            let (pixel_sums, multiplier, amend) = context
                .pixel_sums(output_x + lane as u32, output_y)
                .ok_or_else(|| {
                    PilError::InternalError("Reduce AVX2 edge block facts were unavailable".into())
                })?;
            for channel in 0..CHANNELS {
                sums[channel][lane] = pixel_sums[channel];
            }
            multipliers[lane] = multiplier;
            amends[lane] = amend;
        }
    }
    let mut averages = [[0u8; LANES]; 4];
    for channel in 0..CHANNELS {
        // SAFETY: AVX2 is established by the prepared strategy; the lane sums
        // and fixed-point factors obey the shared Reduce guard's bounds.
        let values = unsafe { average_u32x8(sums[channel], multipliers, amends) };
        averages[channel] = values.map(clamp_u8);
    }
    for lane in 0..valid_pixels {
        let alpha = if PREMULTIPLIED {
            averages[CHANNELS - 1][lane]
        } else {
            u8::MAX
        };
        for channel in 0..CHANNELS {
            let value = averages[channel][lane];
            let unpremultiplied = if PREMULTIPLIED && channel + 1 < CHANNELS && alpha != 0 {
                let value = u16::from(value) * 255 / u16::from(alpha);
                u8::try_from(value).unwrap_or(u8::MAX)
            } else {
                value
            };
            output[lane * CHANNELS + channel] = unpremultiplied;
        }
    }
    Ok(())
}

#[target_feature(enable = "avx2")]
unsafe fn reduce_vector_block<
    const CHANNELS: usize,
    const PREMULTIPLIED: bool,
    const RGB_3X5: bool,
>(
    context: &ByteReduceContext<'_, CHANNELS, PREMULTIPLIED>,
    factors: (u32, u32),
    output_x: u32,
    output_y: u32,
    valid_pixels: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    // SAFETY: this function is entered only from the selected AVX2 Reduce plan.
    if unsafe {
        write_special_rgb_vector::<CHANNELS, PREMULTIPLIED, RGB_3X5>(
            context,
            factors,
            output_x,
            output_y,
            valid_pixels,
            output,
        )
    } {
        return Ok(());
    }
    // SAFETY: the same prepared AVX2 plan and exact row slice remain in scope.
    unsafe { write_generic_vector(context, output_x, output_y, valid_pixels, output) }
}

/// Runs the archived native-byte AVX2 reducer for one exact mode strategy.
pub(crate) fn reduce<const CHANNELS: usize, const PREMULTIPLIED: bool, const RGB_3X5: bool>(
    source: &[u8],
    width: u32,
    height: u32,
    x_factor: u32,
    y_factor: u32,
) -> Result<Vec<u8>, PilError> {
    let x_factor = x_factor.max(1);
    let y_factor = y_factor.max(1);
    let area = x_factor.checked_mul(y_factor).ok_or_else(|| {
        PilError::InternalError("Reduce AVX2 factor area overflowed after selection".into())
    })?;
    if area > u32::MAX / 256 {
        return Err(PilError::InternalError(
            "Reduce AVX2 factor area exceeded the lane-sum bound".into(),
        ));
    }
    let context = ByteReduceContext::<CHANNELS, PREMULTIPLIED>::new(
        source, width, height, x_factor, y_factor,
    );
    let (output_width, output_height) = context.output_dimensions();
    let mut output = CheckedDims::new(output_width, output_height, CHANNELS as u8)?.alloc_buffer();
    if output_width == 0 || output_height == 0 {
        return Ok(output);
    }
    let output_width_usize = output_width as usize;
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "CheckedDims validated the complete tightly packed output buffer"
    )]
    let row_stride = output_width_usize * CHANNELS;
    let vector_limit = if output_width < LANES_U32 {
        output_width
    } else {
        output_width / LANES_U32 * LANES_U32
    };
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "CheckedDims validated each row offset and the final row end"
    )]
    for y in 0..output_height {
        let row_start = y as usize * row_stride;
        let row = &mut output[row_start..row_start + row_stride];
        for local in (0..vector_limit).step_by(LANES) {
            let valid_pixels = (output_width - local).min(LANES_U32) as usize;
            let start = local as usize * CHANNELS;
            let end = start + valid_pixels * CHANNELS;
            // SAFETY: this entry point is called only through a prepared AVX2
            // strategy whose context and request guard establish host capability.
            unsafe {
                reduce_vector_block::<CHANNELS, PREMULTIPLIED, RGB_3X5>(
                    &context,
                    (x_factor, y_factor),
                    local,
                    y,
                    valid_pixels,
                    &mut row[start..end],
                )?;
            }
        }
        context.write_row_range(y, row, vector_limit, output_width);
    }
    Ok(output)
}
