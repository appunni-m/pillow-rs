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
pub(super) fn parameters_match(kernel: &[f32; 25], scale: f32, offset: f32) -> bool {
    scale == 16.0 && offset == 0.0 && kernel == &BLUR_RING_KERNEL
}

/// Apply the ring convolution to native RGB bytes, preserving the copied
/// two-pixel border. `None` declines before changing output when dimensions,
/// lengths, or scratch allocation cannot be represented.
#[inline(never)]
pub(super) fn apply(raw: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::{BLUR_RING_KERNEL, apply, parameters_match};
    use crate::compute::pool_cpu::ops::filter::filter_5x5_byte_row;

    const NORMALIZED_BLUR_RING: [f32; 25] = [
        0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0, 0.0, 0.0, 0.0625, 0.0625, 0.0, 0.0,
        0.0, 0.0625, 0.0625, 0.0, 0.0, 0.0, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625,
    ];

    fn reference(raw: &[u8], width: usize, height: usize) -> Vec<u8> {
        let row_stride = width * 3;
        let mut expected = raw.to_vec();
        for y in 0..height {
            let row_start = y * row_stride;
            filter_5x5_byte_row(
                raw,
                &mut expected[row_start..row_start + row_stride],
                y as i32,
                width as i32,
                height as i32,
                3,
                &NORMALIZED_BLUR_RING,
                0.5,
            );
        }
        expected
    }

    #[test]
    fn rgb_ring_matches_ordered_filter_for_small_odd_even_and_tailed_sizes() {
        for width in [4usize, 5, 6, 7, 16, 17, 31] {
            for height in [4usize, 5, 6, 9, 17] {
                let raw = (0..width * height * 3)
                    .map(|index| ((index * 73 + index / 7 * 29 + 17) % 256) as u8)
                    .collect::<Vec<_>>();
                assert_eq!(
                    apply(&raw, width, height),
                    Some(reference(&raw, width, height))
                );
            }
        }
    }

    #[test]
    fn rgb_ring_matches_extreme_samples_and_preserves_borders() {
        for value in [0u8, 255] {
            let raw = vec![value; 17 * 9 * 3];
            assert_eq!(apply(&raw, 17, 9), Some(reference(&raw, 17, 9)));
        }

        let raw = (0..17 * 9 * 3)
            .map(|index| ((index * 47 + 3) % 256) as u8)
            .collect::<Vec<_>>();
        let actual = apply(&raw, 17, 9).expect("valid RGB raster");
        for y in 0..9 {
            for x in 0..17 {
                if x < 2 || x >= 15 || y < 2 || y >= 7 {
                    let start = (y * 17 + x) * 3;
                    assert_eq!(&actual[start..start + 3], &raw[start..start + 3]);
                }
            }
        }
    }

    #[test]
    fn dispatch_admits_only_pillow_blur_parameters() {
        assert!(parameters_match(&BLUR_RING_KERNEL, 16.0, 0.0));
        assert!(!parameters_match(&BLUR_RING_KERNEL, 15.0, 0.0));
        assert!(!parameters_match(&BLUR_RING_KERNEL, 16.0, 1.0));

        let mut changed_kernel = BLUR_RING_KERNEL;
        changed_kernel[12] = 1.0;
        assert!(!parameters_match(&changed_kernel, 16.0, 0.0));
    }
}
