//! Exact scalar box-window candidate for normalized native-byte 5x5 filters.
//!
//! The caller supplies an output buffer already copied from `raw`; this
//! helper changes only interior pixels. Unsupported layouts and parameters,
//! invalid lengths, and scratch reservation failure decline without changing
//! `out`.

const CHANNELS_MIN: usize = 2;
const CHANNELS_MAX: usize = 4;
const UNIFORM_COEFFICIENT: f32 = 1.0f32 / 25.0f32;

/// Apply the exact normalized all-ones kernel for native interleaved LA, RGB,
/// or RGBA bytes. Intended as the single low-level admission call from the
/// generic byte-row fallback; serial-only placement is controlled by its
/// caller.
#[inline(never)]
pub(super) fn apply_if_uniform(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> bool {
    if !(CHANNELS_MIN..=CHANNELS_MAX).contains(&channels) || width < 5 || height < 5 {
        return false;
    }
    let Some(_pixel_count) = width.checked_mul(height) else {
        return false;
    };
    let Some(row_samples) = width.checked_mul(channels) else {
        return false;
    };
    let Some(expected_bytes) = row_samples.checked_mul(height) else {
        return false;
    };
    if raw.len() != expected_bytes || out.len() != expected_bytes {
        return false;
    }
    if rounding_bias != 0.5
        || kernel
            .iter()
            .any(|&coefficient| coefficient != UNIFORM_COEFFICIENT)
    {
        return false;
    }

    match channels {
        2 => apply_uniform::<2>(raw, out, width, height, row_samples),
        3 => apply_uniform::<3>(raw, out, width, height, row_samples),
        4 => apply_uniform::<4>(raw, out, width, height, row_samples),
        _ => false,
    }
}

fn apply_uniform<const CHANNELS: usize>(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    row_samples: usize,
) -> bool {
    // Five bytes sum to at most 1,275; five such columns sum to at most
    // 6,375. Both sliding updates subtract a term already in the sum before
    // adding its replacement, so every intermediate fits u16.
    // For Pillow's normalized f32 coefficient 1/25, the ordered floating
    // evaluation differs from the exact mean by less than 0.001. Since 25
    // is odd, mean + 0.5 stays at least 1/50 from an integer: its clipped
    // byte is exactly (sum + 12) / 25, as for the existing uniform L kernel.
    // Reserve before writing so allocation failure leaves the caller's copied
    // output intact and available to the generic fallback.
    let mut columns = Vec::<u16>::new();
    if columns.try_reserve_exact(row_samples).is_err() {
        return false;
    }
    columns.resize(row_samples, 0);
    crate::compute::record_pipeline_allocation(
        columns
            .capacity()
            .saturating_mul(std::mem::size_of::<u16>()),
    );

    let row_stride = row_samples;
    for source_y in 0..5 {
        let source_row = source_y * row_stride;
        for sample in 0..row_stride {
            columns[sample] += u16::from(raw[source_row + sample]);
        }
    }

    for output_y in 2..height - 2 {
        let mut horizontal = [0u16; CHANNELS];
        for column in 0..5 {
            let column_start = column * CHANNELS;
            for channel in 0..CHANNELS {
                horizontal[channel] += columns[column_start + channel];
            }
        }

        let output_row = output_y * row_stride;
        for x in 2..width - 2 {
            let output_pixel = output_row + x * CHANNELS;
            for channel in 0..CHANNELS {
                out[output_pixel + channel] = ((horizontal[channel] + 12) / 25) as u8;
            }

            if x + 1 < width - 2 {
                let leaving_column = (x - 2) * CHANNELS;
                let entering_column = (x + 3) * CHANNELS;
                for channel in 0..CHANNELS {
                    horizontal[channel] -= columns[leaving_column + channel];
                    horizontal[channel] += columns[entering_column + channel];
                }
            }
        }

        if output_y + 1 < height - 2 {
            let leaving_row = (output_y - 2) * row_stride;
            let entering_row = (output_y + 3) * row_stride;
            for sample in 0..row_stride {
                columns[sample] -= u16::from(raw[leaving_row + sample]);
                columns[sample] += u16::from(raw[entering_row + sample]);
            }
        }
    }

    true
}
