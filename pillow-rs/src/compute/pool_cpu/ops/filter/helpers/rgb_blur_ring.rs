//! Native RGB kernel for Pillow's exact 5x5 BLUR ring.
//!
//! The 16 edge coefficients are one and the inner 3x3 coefficients are zero.
//! Per channel, the ring sum is therefore a 5x5 sum minus the centered 3x3
//! sum. Both totals use byte samples and remain exact in `u16`.

const CHANNELS: usize = 3;
const BLUR_RING_KERNEL: [f32; 25] = [
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
    1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
];

/// Admit only Pillow's native BLUR kernel and its original divisor/offset.
#[inline]
pub(in crate::compute::pool_cpu::ops::filter) fn parameters_match(
    kernel: &[f32; 25],
    scale: f32,
    offset: f32,
) -> bool {
    scale == 16.0 && offset == 0.0 && kernel == &BLUR_RING_KERNEL
}

/// Apply the ring convolution to native RGB bytes, preserving the copied
/// two-pixel border. `None` declines before changing output when dimensions,
/// lengths, or scratch allocation cannot be represented.
#[inline(never)]
pub(in crate::compute::pool_cpu::ops::filter) fn apply(
    raw: &[u8],
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let row_stride = width.checked_mul(CHANNELS)?;
    let byte_count = row_stride.checked_mul(height)?;
    if raw.len() != byte_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some(raw.to_vec());
    }

    let mut wide_columns = Vec::<u16>::new();
    wide_columns.try_reserve_exact(row_stride).ok()?;
    wide_columns.resize(row_stride, 0);
    let mut inner_columns = Vec::<u16>::new();
    inner_columns.try_reserve_exact(row_stride).ok()?;
    inner_columns.resize(row_stride, 0);

    // These live scratch buffers are part of the public-call cost and the
    // allocation receipt. Each byte contributes to at most five rows.
    crate::compute::record_pipeline_allocation(
        wide_columns
            .capacity()
            .saturating_add(inner_columns.capacity())
            .saturating_mul(std::mem::size_of::<u16>()),
    );

    for source_y in 0..5 {
        let source_row = source_y * row_stride;
        for sample in 0..row_stride {
            wide_columns[sample] += u16::from(raw[source_row + sample]);
            if (1..4).contains(&source_y) {
                inner_columns[sample] += u16::from(raw[source_row + sample]);
            }
        }
    }

    let mut output = raw.to_vec();
    for y in 2..height - 2 {
        let mut wide = [0u16; CHANNELS];
        let mut inner = [0u16; CHANNELS];
        for column in 0..5 {
            let column_start = column * CHANNELS;
            for channel in 0..CHANNELS {
                wide[channel] += wide_columns[column_start + channel];
            }
        }
        for column in 1..4 {
            let column_start = column * CHANNELS;
            for channel in 0..CHANNELS {
                inner[channel] += inner_columns[column_start + channel];
            }
        }

        let output_row = y * row_stride;
        for x in 2..width - 2 {
            let output_pixel = output_row + x * CHANNELS;
            for channel in 0..CHANNELS {
                let ring_sum = wide[channel] - inner[channel];
                output[output_pixel + channel] = ((ring_sum + 8) >> 4) as u8;
            }

            if x + 1 < width - 2 {
                let wide_leaving = (x - 2) * CHANNELS;
                let wide_entering = (x + 3) * CHANNELS;
                let inner_leaving = (x - 1) * CHANNELS;
                let inner_entering = (x + 2) * CHANNELS;
                for channel in 0..CHANNELS {
                    wide[channel] = wide[channel] - wide_columns[wide_leaving + channel]
                        + wide_columns[wide_entering + channel];
                    inner[channel] = inner[channel] - inner_columns[inner_leaving + channel]
                        + inner_columns[inner_entering + channel];
                }
            }
        }

        if y + 1 < height - 2 {
            let wide_leaving_row = (y - 2) * row_stride;
            let wide_entering_row = (y + 3) * row_stride;
            let inner_leaving_row = (y - 1) * row_stride;
            let inner_entering_row = (y + 2) * row_stride;
            for sample in 0..row_stride {
                wide_columns[sample] = wide_columns[sample]
                    - u16::from(raw[wide_leaving_row + sample])
                    + u16::from(raw[wide_entering_row + sample]);
                inner_columns[sample] = inner_columns[sample]
                    - u16::from(raw[inner_leaving_row + sample])
                    + u16::from(raw[inner_entering_row + sample]);
            }
        }
    }

    Some(output)
}
