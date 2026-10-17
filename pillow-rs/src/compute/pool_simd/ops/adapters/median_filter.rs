//! Focused fixed-size SIMD order-statistic kernels.

use super::u8x16;

#[inline(always)]
fn compare_exchange(values: &mut [u8x16], left: usize, right: usize) {
    let a = values[left];
    let b = values[right];
    values[left] = a.min(b);
    values[right] = a.max(b);
}

/// Select the median of nine vector values with the CPU path's 19-pair network.
///
/// The fixed network preserves duplicates and avoids fully sorting all nine
/// samples for the native L and LA 3×3 median paths.
#[inline]
pub(super) fn median_of_nine_vectors(values: &mut [u8x16]) -> u8x16 {
    debug_assert_eq!(values.len(), 9);
    compare_exchange(values, 1, 2);
    compare_exchange(values, 4, 5);
    compare_exchange(values, 7, 8);
    compare_exchange(values, 0, 1);
    compare_exchange(values, 3, 4);
    compare_exchange(values, 6, 7);
    compare_exchange(values, 0, 3);
    compare_exchange(values, 1, 2);
    compare_exchange(values, 4, 5);
    compare_exchange(values, 7, 8);
    compare_exchange(values, 3, 6);
    compare_exchange(values, 4, 7);
    compare_exchange(values, 5, 8);
    compare_exchange(values, 1, 4);
    compare_exchange(values, 2, 5);
    compare_exchange(values, 4, 7);
    compare_exchange(values, 4, 2);
    compare_exchange(values, 6, 4);
    compare_exchange(values, 4, 2);
    values[4]
}

#[cfg(test)]
mod tests {
    use super::super::u8x16;
    use super::median_of_nine_vectors;

    #[test]
    fn median_network_matches_full_sort_with_ties_and_replicated_edges() {
        let mut state = 0x6d2b_79f5u32;
        for case in 0..4096u32 {
            let mut samples = [u8x16::splat(0); 9];
            let mut expected = [[0u8; 16]; 9];
            for (index, sample) in samples.iter_mut().enumerate() {
                let mut lanes = [0u8; 16];
                for (lane, value) in lanes.iter_mut().enumerate() {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    // A small domain deliberately exercises repeated samples,
                    // as replicated image borders do in real windows.
                    *value = ((state >> 16) % 7) as u8;
                    expected[index][lane] = *value;
                }
                *sample = u8x16::new(lanes);
            }

            // Include all-equal, ascending, and descending lane vectors in
            // addition to the deterministic repeated-value sequence.
            if case == 0 {
                samples.fill(u8x16::splat(91));
                expected.fill([91; 16]);
            } else if case == 1 || case == 2 {
                for index in 0..9 {
                    let value = if case == 1 {
                        index as u8
                    } else {
                        (8 - index) as u8
                    };
                    samples[index] = u8x16::splat(value);
                    expected[index] = [value; 16];
                }
            }

            let actual = median_of_nine_vectors(&mut samples);
            let actual = actual.to_array();
            for lane in 0..16 {
                let mut sorted = expected.map(|sample| sample[lane]);
                sorted.sort_unstable();
                assert_eq!(actual[lane], sorted[4], "case={case} lane={lane}");
            }
        }
    }
}
