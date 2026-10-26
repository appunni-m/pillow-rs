//! Scalar-domain and grayscale fast paths for color counting.

use super::super::FormattedPixelValue;
use super::pillow_table::{PillowColorCounts, pillow_color_components, pillow_color_pixel};

pub(in crate::image) enum RgbGrayscaleColorCounts {
    NotGrayscale,
    TooManyColors,
    Colors(Vec<(u32, FormattedPixelValue)>),
}

pub(in crate::image) enum ScalarColorDomainCounts {
    NotApplicable,
    TooManyColors,
    Colors(Vec<(u32, FormattedPixelValue)>),
}

fn scalar_byte_domain_index(mode: &str, pixel: u32) -> Option<usize> {
    match mode {
        "I" => {
            let value = i32::from_ne_bytes(pixel.to_ne_bytes());
            if (0..=255).contains(&value) {
                usize::try_from(value).ok()
            } else {
                None
            }
        }
        "F" => {
            if pixel == 0 {
                return Some(0);
            }
            if pixel >> 31 != 0 {
                return None;
            }
            let exponent = (pixel >> 23) & 0xff;
            if !(127..=134).contains(&exponent) {
                return None;
            }
            let shift = 23 - (exponent - 127);
            let significand = (1 << 23) | (pixel & 0x7f_ffff);
            let fractional_mask = (1 << shift) - 1;
            if significand & fractional_mask != 0 {
                return None;
            }
            let value = significand >> shift;
            if value <= 255 {
                usize::try_from(value).ok()
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(in crate::image) fn scalar_byte_domain_is_plausible(mode: &str, bytes: &[u8]) -> bool {
    let sample_count = bytes.len() / 4;
    if sample_count == 0 {
        return true;
    }
    let last_index = sample_count - 1;
    let base_step = last_index / 15;
    let remainder = last_index % 15;
    for ordinal in 0..=15usize {
        let Some(sample_index) = base_step
            .checked_mul(ordinal)
            .and_then(|base| base.checked_add((remainder * ordinal) / 15))
        else {
            return false;
        };
        let Some(start) = sample_index.checked_mul(4) else {
            return false;
        };
        let Some(end) = start.checked_add(4) else {
            return false;
        };
        let Some(sample) = bytes.get(start..end) else {
            return false;
        };
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        if scalar_byte_domain_index(mode, pixel).is_none() {
            return false;
        }
    }
    true
}

pub(in crate::image) fn getcolors_scalar_byte_domain(
    mode: &str,
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> ScalarColorDomainCounts {
    let mut histogram = [0u32; 256];
    let mut first_seen = Vec::<u8>::with_capacity(256);
    for sample in bytes.chunks_exact(4) {
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let Some(index) = scalar_byte_domain_index(mode, pixel) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        let count = histogram[index];
        let Some(next_count) = count.checked_add(1) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        if count == 0 {
            if first_seen.len() == maxcolors {
                return ScalarColorDomainCounts::TooManyColors;
            }
            let Ok(index) = u8::try_from(index) else {
                return ScalarColorDomainCounts::NotApplicable;
            };
            first_seen.push(index);
        }
        histogram[index] = next_count;
    }

    scalar_byte_domain_counts(
        mode,
        &histogram,
        first_seen,
        maxcolors,
        code_mask,
        code_polynomial,
    )
}

#[inline]
fn scalar_byte_domain_counts(
    mode: &str,
    histogram: &[u32; 256],
    first_seen: Vec<u8>,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> ScalarColorDomainCounts {
    // Preserve Pillow's first-insertion probe sequence without allocating its
    // maxcolors-sized direct table for this bounded scalar domain.
    let mut counts =
        PillowColorCounts::Sparse(std::collections::HashMap::with_capacity(first_seen.len()));
    let mut distinct_colors = 0usize;
    for value in first_seen {
        let index = usize::from(value);
        let pixel = if mode == "I" {
            u32::from_ne_bytes(i32::from(value).to_ne_bytes())
        } else {
            f32::from(value).to_bits()
        };
        let Ok(repetitions) = usize::try_from(histogram[index]) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        let Some(counted) = counts.count_repeated(
            &mut distinct_colors,
            pixel,
            repetitions,
            maxcolors,
            code_mask,
            code_polynomial,
        ) else {
            return ScalarColorDomainCounts::NotApplicable;
        };
        if !counted {
            return ScalarColorDomainCounts::TooManyColors;
        }
    }

    ScalarColorDomainCounts::Colors(
        counts
            .into_slot_order(distinct_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                let value = if mode == "I" {
                    FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes()))
                } else {
                    FormattedPixelValue::Float(f32::from_bits(pixel) as f64)
                };
                (count, value)
            })
            .collect(),
    )
}

pub(in crate::image) const SCALAR_I_SPARSE_RUN_COLOR_LIMIT: usize = 4096;
pub(in crate::image) const SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES: usize = 256;
pub(in crate::image) const SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS: usize = 65_536;

pub(in crate::image) fn getcolors_scalar_i_repeated_runs(
    bytes: &[u8],
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> ScalarColorDomainCounts {
    let exact_len = bytes.len() / 4 * 4;
    let mut counts = PillowColorCounts::Sparse(std::collections::HashMap::with_capacity(256));
    let mut distinct_colors = 0usize;
    let mut offset = 0usize;

    while offset < exact_len {
        let sample = &bytes[offset..offset + 4];
        let pixel = u32::from_ne_bytes([sample[0], sample[1], sample[2], sample[3]]);

        let pattern = [
            sample[0], sample[1], sample[2], sample[3], sample[0], sample[1], sample[2], sample[3],
            sample[0], sample[1], sample[2], sample[3], sample[0], sample[1], sample[2], sample[3],
        ];
        let mut run_end = offset;
        while let Some(end) = run_end.checked_add(pattern.len()) {
            if end > exact_len || bytes[run_end..end] != pattern[..] {
                break;
            }
            run_end = end;
        }
        while let Some(end) = run_end.checked_add(4) {
            if end > exact_len || bytes[run_end..end] != sample[..] {
                break;
            }
            run_end = end;
        }

        let repetitions = (run_end - offset) / 4;
        match counts.count_repeated(
            &mut distinct_colors,
            pixel,
            repetitions,
            maxcolors,
            code_mask,
            code_polynomial,
        ) {
            Some(true) => {}
            Some(false) => return ScalarColorDomainCounts::TooManyColors,
            None => {
                for _ in 0..repetitions {
                    if !counts.count(
                        &mut distinct_colors,
                        pixel,
                        maxcolors,
                        code_mask,
                        code_polynomial,
                    ) {
                        return ScalarColorDomainCounts::TooManyColors;
                    }
                }
            }
        }
        if distinct_colors > SCALAR_I_SPARSE_RUN_COLOR_LIMIT {
            return ScalarColorDomainCounts::NotApplicable;
        }
        offset = run_end;
    }

    ScalarColorDomainCounts::Colors(
        counts
            .into_slot_order(distinct_colors)
            .into_iter()
            .map(|(_, (pixel, count))| {
                (
                    count,
                    FormattedPixelValue::Integer(i32::from_ne_bytes(pixel.to_ne_bytes())),
                )
            })
            .collect(),
    )
}

pub(in crate::image) fn scalar_prefix_has_repeated_samples(bytes: &[u8]) -> bool {
    let mut samples = bytes.chunks_exact(4).take(64);
    let Some(mut previous) = samples.next() else {
        return false;
    };
    let mut repeated_pairs = 0usize;
    for sample in samples {
        if sample == previous {
            repeated_pairs += 1;
            if repeated_pairs >= 4 {
                return true;
            }
        }
        previous = sample;
    }
    false
}

pub(in crate::image) fn scalar_i_prefix_has_long_run(bytes: &[u8]) -> bool {
    let Some(first) = bytes.get(..4) else {
        return false;
    };
    bytes
        .chunks_exact(4)
        .take(SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES)
        .all(|sample| sample == first)
}

pub(in crate::image) fn getcolors_rgb8_grayscale(
    image: &crate::raster::RgbImage,
    maxcolors: usize,
    code_mask: u32,
    code_polynomial: u32,
) -> RgbGrayscaleColorCounts {
    let mut histogram = [0u32; 256];
    let mut distinct_colors = 0usize;
    for pixel in image.as_raw().chunks_exact(3) {
        let [red, green, blue] = [pixel[0], pixel[1], pixel[2]];
        if red != green || green != blue {
            return RgbGrayscaleColorCounts::NotGrayscale;
        }
        let count = &mut histogram[usize::from(red)];
        if *count == 0 {
            distinct_colors += 1;
            if distinct_colors > maxcolors {
                return RgbGrayscaleColorCounts::TooManyColors;
            }
        }
        *count += 1;
    }

    // At most 256 grayscale keys are occupied. Preserve Pillow's slot order
    // with its same probe recurrence, without allocating or zeroing its large
    // `maxcolors`-sized table for every scanned pixel.
    let mut occupied = Vec::<(u32, u32, u32)>::with_capacity(distinct_colors);
    for (gray, count) in histogram.into_iter().enumerate() {
        if count == 0 {
            continue;
        }
        let Ok(gray) = u8::try_from(gray) else {
            continue;
        };
        let pixel = pillow_color_pixel(&[gray, gray, gray]);
        let mut slot = (!pixel) & code_mask;
        let mut increment = 0u32;
        let mut first_conflict = true;

        loop {
            if !occupied
                .iter()
                .any(|(occupied_slot, _, _)| *occupied_slot == slot)
            {
                occupied.push((slot, pixel, count));
                break;
            }

            if first_conflict {
                increment = (pixel ^ (pixel >> 3)) & code_mask;
                if increment == 0 {
                    increment = code_mask;
                }
                first_conflict = false;
            } else {
                increment <<= 1;
                if increment > code_mask {
                    increment ^= code_polynomial;
                }
            }
            slot = (slot + increment) & code_mask;
        }
    }

    occupied.sort_unstable_by_key(|(slot, _, _)| *slot);
    RgbGrayscaleColorCounts::Colors(
        occupied
            .into_iter()
            .map(|(_, pixel, count)| (count, pillow_color_components(pixel, 3)))
            .collect(),
    )
}
