use super::*;

#[inline]
fn histogram_value_at(histogram: &[usize; 256], index: usize, fallback: u8) -> u8 {
    let mut remaining = index;
    for (value, count) in histogram.iter().enumerate() {
        if remaining < *count {
            return value as u8;
        }
        remaining -= *count;
    }
    fallback
}

/// Reduce a native L byte slice to its extrema using portable vector lanes.
/// AutoContrast at cutoff zero needs only these two bins, not a full histogram.
fn native_luma_minmax(raw: &[u8]) -> Option<(u8, u8)> {
    if raw.is_empty() {
        return None;
    }
    let mut minima = u8x16::splat(u8::MAX);
    let mut maxima = u8x16::splat(0);
    let mut chunks = raw.chunks_exact(16);
    for chunk in &mut chunks {
        let values = u8x16::new(chunk.try_into().ok()?);
        minima = minima.min(values);
        maxima = maxima.max(values);
    }
    let mut minimum = *minima.to_array().iter().min()?;
    let mut maximum = *maxima.to_array().iter().max()?;
    for &value in chunks.remainder() {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    Some((minimum, maximum))
}

pub(super) fn native_luma_lut_tables(lut: &[u8]) -> Option<[u8x16; 16]> {
    if lut.len() != 256 {
        return None;
    }
    Some(std::array::from_fn(|index| {
        let start = index * 16;
        u8x16::new(
            lut[start..start + 16]
                .try_into()
                .expect("each byte LUT table has sixteen entries"),
        )
    }))
}

#[inline]
fn native_luma_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let lookups: [u8x16; 16] = std::array::from_fn(|index| tables[index].swizzle_relaxed(low));
    let bit0 = (high & u8x16::splat(1)).simd_eq(u8x16::splat(0));
    let bit1 = (high & u8x16::splat(2)).simd_eq(u8x16::splat(0));
    let bit2 = (high & u8x16::splat(4)).simd_eq(u8x16::splat(0));
    let bit3 = (high & u8x16::splat(8)).simd_eq(u8x16::splat(0));
    let level1: [u8x16; 8] =
        std::array::from_fn(|index| bit0.select(lookups[index * 2], lookups[index * 2 + 1]));
    let level2: [u8x16; 4] =
        std::array::from_fn(|index| bit1.select(level1[index * 2], level1[index * 2 + 1]));
    let level3: [u8x16; 2] =
        std::array::from_fn(|index| bit2.select(level2[index * 2], level2[index * 2 + 1]));
    bit3.select(level3[0], level3[1])
}

pub(super) fn apply_native_luma_lut_row(
    source: &[u8],
    destination: &mut [u8],
    tables: &[u8x16; 16],
) {
    for (source, destination) in source.chunks(16).zip(destination.chunks_mut(16)) {
        let active = source.len();
        let mut padded = [0u8; 16];
        padded[..active].copy_from_slice(source);
        let mapped = native_luma_lut_chunk(u8x16::new(padded), tables).to_array();
        destination.copy_from_slice(&mapped[..active]);
    }
}

/// Build Pillow's per-channel autocontrast lookup table.
///
/// Histogram construction and percentile selection are scalar control work;
/// callers can apply the resulting byte LUT with a backend-specific data
/// kernel. Keeping this control plane shared prevents the CPU and SIMD paths
/// from drifting on cutoff rounding, masked selection, or identity channels.
pub(crate) fn autocontrast_lut(
    img: &DynamicImage,
    cutoff: f64,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<Vec<u8>, PilError> {
    let histogram = collect_autocontrast_histogram(img, cutoff, mask)?;
    if histogram.selected_pixels == 0 {
        return Ok(identity_lut(histogram.channels));
    }
    Ok(build_autocontrast_lut(&histogram, cutoff))
}

struct AutocontrastHistogram {
    channels: usize,
    selected_pixels: usize,
    values: [[usize; 256]; 4],
    zero_cutoff_luma_bounds: Option<(u8, u8)>,
}

fn collect_autocontrast_histogram(
    img: &DynamicImage,
    cutoff: f64,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<AutocontrastHistogram, PilError> {
    let channels = img.color().channel_count() as usize;
    let (width, height) = (img.width(), img.height());
    let image_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    let mask = mask
        .map(|mask| mask.materialize_for_ops())
        .transpose()?
        .map(|mask| mask.to_luma8());
    let mut selected_pixels = if mask.is_none() { image_pixels } else { 0 };
    let raw = img.as_bytes();
    let stride = width as usize * channels;
    let expected_len = image_pixels.checked_mul(channels);
    let zero_cutoff_luma_bounds = if cutoff == 0.0
        && mask.is_none()
        && matches!((img, channels), (DynamicImage::ImageLuma8(_), 1))
        && expected_len == Some(raw.len())
    {
        native_luma_minmax(raw)
    } else {
        None
    };
    let mut values = [[0usize; 256]; 4];
    let native_histogram = fill_native_autocontrast_histogram(
        img,
        mask.is_none(),
        raw,
        image_pixels,
        expected_len,
        channels,
        zero_cutoff_luma_bounds,
        &mut values,
    );
    if !native_histogram {
        for y in 0..height as usize {
            for x in 0..width as usize {
                if let Some(mask) = mask.as_ref() {
                    if mask.get_pixel(x as u32, y as u32)[0] == 0 {
                        continue;
                    }
                    selected_pixels += 1;
                }
                let index = y * stride + x * channels;
                for channel in 0..channels {
                    values[channel][raw[index + channel] as usize] += 1;
                }
            }
        }
    }
    Ok(AutocontrastHistogram {
        channels,
        selected_pixels,
        values,
        zero_cutoff_luma_bounds,
    })
}

fn fill_native_autocontrast_histogram(
    img: &DynamicImage,
    unmasked: bool,
    raw: &[u8],
    image_pixels: usize,
    expected_len: Option<usize>,
    channels: usize,
    zero_cutoff_luma_bounds: Option<(u8, u8)>,
    histograms: &mut [[usize; 256]; 4],
) -> bool {
    if zero_cutoff_luma_bounds.is_some() {
        return true;
    }
    let native_layout = matches!(
        (img, channels),
        (DynamicImage::ImageLuma8(_), 1) | (DynamicImage::ImageRgb8(_), 3)
    );
    if !unmasked || expected_len != Some(raw.len()) || !native_layout {
        return false;
    }
    // The SIMD adapter shares this control plane, so retain the banked native
    // byte histogram for its unmasked L/RGB cases.
    let counts = if image_pixels < 16_384 {
        equalize_histogram::<1>(raw, channels)
    } else {
        equalize_histogram::<4>(raw, channels)
    };
    for channel in 0..channels {
        for bin in 0..256 {
            histograms[channel][bin] = counts[channel][bin] as usize;
        }
    }
    true
}

fn identity_lut(channels: usize) -> Vec<u8> {
    let mut lut = vec![0u8; channels * 256];
    for channel in 0..channels {
        for value in 0..=u8::MAX {
            lut[channel * 256 + usize::from(value)] = value;
        }
    }
    lut
}

fn build_autocontrast_lut(histogram: &AutocontrastHistogram, cutoff: f64) -> Vec<u8> {
    let mut lut = vec![0u8; histogram.channels * 256];
    let total = histogram.selected_pixels as f64;
    for channel in 0..histogram.channels {
        let low_threshold = (total * cutoff / 100.0) as usize;
        let high_threshold = (total * (100.0 - cutoff) / 100.0) as usize;
        let (low, high) = channel_bounds(histogram, channel, low_threshold, high_threshold);
        let start = channel * 256;
        if high <= low {
            fill_identity_channel(&mut lut[start..start + 256]);
        } else {
            fill_contrast_channel(&mut lut[start..start + 256], low, high);
        }
    }
    lut
}

fn channel_bounds(
    histogram: &AutocontrastHistogram,
    channel: usize,
    low_threshold: usize,
    high_threshold: usize,
) -> (f64, f64) {
    if let Some((low, high)) = histogram.zero_cutoff_luma_bounds {
        return (f64::from(low), f64::from(high));
    }
    let high_index = high_threshold.min(histogram.selected_pixels - 1);
    (
        histogram_value_at(&histogram.values[channel], low_threshold, 0) as f64,
        histogram_value_at(&histogram.values[channel], high_index, 255) as f64,
    )
}

fn fill_identity_channel(channel: &mut [u8]) {
    for (index, value) in channel.iter_mut().enumerate() {
        *value = index as u8;
    }
}

fn fill_contrast_channel(channel: &mut [u8], low: f64, high: f64) {
    let scale = 255.0 / (high - low);
    let offset = -low * scale;
    for (value, output) in channel.iter_mut().enumerate() {
        // PIL: int(ix * scale + offset) with clamping to [0,255].
        let mapped = value as f64 * scale + offset;
        *output = if mapped < 0.0 {
            0
        } else if mapped > 255.0 {
            255
        } else {
            mapped as u8
        };
    }
}

#[cfg(test)]
mod autocontrast_luma_tests {
    use super::{apply_native_luma_lut_row, autocontrast_lut, native_luma_lut_tables};
    use crate::raster::{DynamicImage, GrayImage};

    #[test]
    fn native_luma_lut_vector_path_matches_every_scalar_table_entry_and_tail() {
        let lut: Vec<u8> = (0..=u8::MAX)
            .map(|value| value.wrapping_mul(73).wrapping_add(19))
            .collect();
        let tables = native_luma_lut_tables(&lut).expect("256-entry L LUT");
        let source: Vec<u8> = (0..=u8::MAX).chain([19, 73, 211]).collect();
        let mut destination = vec![0; source.len()];
        apply_native_luma_lut_row(&source, &mut destination, &tables);
        assert_eq!(
            destination,
            source
                .iter()
                .map(|&value| lut[usize::from(value)])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn zero_cutoff_luma_lut_uses_native_extrema_exactly() {
        let pixels = [
            23, 50, 127, 196, 80, 112, 151, 172, 24, 195, 120, 90, 67, 88, 99, 150, 77, 188, 23,
        ];
        let image = DynamicImage::ImageLuma8(
            GrayImage::from_raw(pixels.len() as u32, 1, pixels.to_vec()).expect("valid L image"),
        );
        let lut = autocontrast_lut(&image, 0.0, None).expect("cutoff-zero LUT");
        assert_eq!(lut[22], 0);
        assert_eq!(lut[23], 0);
        assert_eq!(lut[50], 39);
        assert_eq!(lut[127], 153);
        assert_eq!(lut[196], 255);
        assert_eq!(lut[197], 255);

        let constant = DynamicImage::ImageLuma8(
            GrayImage::from_raw(19, 1, vec![77; 19]).expect("valid constant L image"),
        );
        let constant_lut = autocontrast_lut(&constant, 0.0, None).expect("constant LUT");
        assert!(constant_lut.iter().copied().eq(0..=u8::MAX));
    }
}

/// Accumulate native byte bands while shortening repeated-counter dependencies.
///
/// Sixteen equal pixels need one increment per band. Other blocks alternate
/// counter banks so skewed inputs do not serialize every increment on one
/// address. Four padding words separate the banks' channel strides; the final
/// reduction reads only the 256 real bins. Small inputs use one bank to avoid
/// paying for extra zeroing and reduction.
fn equalize_histogram<const BANKS: usize>(raw: &[u8], channels: usize) -> [[u32; 256]; 3] {
    let mut banks = [[[0u32; 260]; 3]; BANKS];
    if channels == 1 {
        let mut chunks = raw.chunks_exact(16);
        for chunk in &mut chunks {
            if chunk == [chunk[0]; 16] {
                banks[0][0][usize::from(chunk[0])] += 16;
            } else {
                for (index, &value) in chunk.iter().enumerate() {
                    banks[index % BANKS][0][usize::from(value)] += 1;
                }
            }
        }
        for &value in chunks.remainder() {
            banks[0][0][usize::from(value)] += 1;
        }
    } else {
        let mut chunks = raw.chunks_exact(48);
        for chunk in &mut chunks {
            // Establish a three-byte period across the complete block. Testing
            // only its ends would miss changes inside a repeated-color region.
            let repeated = chunk[..24] == chunk[24..]
                && chunk[..12] == chunk[12..24]
                && chunk[..6] == chunk[6..12]
                && chunk[..3] == chunk[3..6];
            if repeated {
                banks[0][0][usize::from(chunk[0])] += 16;
                banks[0][1][usize::from(chunk[1])] += 16;
                banks[0][2][usize::from(chunk[2])] += 16;
            } else {
                for (index, pixel) in chunk.chunks_exact(3).enumerate() {
                    let bank = &mut banks[index % BANKS];
                    bank[0][usize::from(pixel[0])] += 1;
                    bank[1][usize::from(pixel[1])] += 1;
                    bank[2][usize::from(pixel[2])] += 1;
                }
            }
        }
        for pixel in chunks.remainder().chunks_exact(3) {
            banks[0][0][usize::from(pixel[0])] += 1;
            banks[0][1][usize::from(pixel[1])] += 1;
            banks[0][2][usize::from(pixel[2])] += 1;
        }
    }
    let mut result = [[0u32; 256]; 3];
    for bank in &banks {
        for (output, counts) in result.iter_mut().zip(bank).take(channels) {
            for (total, count) in output.iter_mut().zip(counts) {
                *total += count;
            }
        }
    }
    result
}

/// Build the per-channel LUT used by Pillow's equalize operation.
///
/// Histogram construction is scalar reduction/control work. The returned
/// native-band table is intentionally separate from applying it so SIMD can
/// keep the complete pixel pass in its vector LUT data plane.
pub(crate) fn equalize_lut(
    img: &DynamicImage,
    channels: usize,
    mask: Option<&DynamicImage>,
) -> Option<Vec<u8>> {
    if !matches!(channels, 1 | 3) {
        return None;
    }
    let expected_len = (img.width() as usize)
        .checked_mul(img.height() as usize)?
        .checked_mul(channels)?;
    let raw = img.as_bytes();
    if raw.len() != expected_len {
        return None;
    }

    let mut histograms = [[0u32; 256]; 3];
    // Masked selection retains its exact nonzero-byte predicate; unmasked
    // byte images can combine repeated samples without examining a mask.
    if let Some(mask) = mask {
        if (mask.width(), mask.height()) != (img.width(), img.height())
            || !matches!(mask, DynamicImage::ImageLuma8(_))
        {
            return None;
        }
        for (pixel, &selected) in raw.chunks_exact(channels).zip(mask.as_bytes()) {
            if selected != 0 {
                for (histogram, &value) in histograms.iter_mut().zip(pixel) {
                    histogram[usize::from(value)] += 1;
                }
            }
        }
    } else if raw.len() / channels < 16_384 {
        histograms = equalize_histogram::<1>(raw, channels);
    } else {
        histograms = equalize_histogram::<4>(raw, channels);
    }

    let mut lut = vec![0u8; channels * 256];
    for channel in 0..channels {
        let start = channel * 256;
        for value in 0..=u8::MAX {
            lut[start + usize::from(value)] = value;
        }

        // PIL equalize: step = (sum(non-zero bins) - last_bin_count) / 255
        // and lut[i] = floor((step/2 + cumulative_histogram) / step).
        let mut nonzero_bins = 0usize;
        let mut last_nonzero_count = 0u32;
        let mut total = 0u32;
        for &count in &histograms[channel] {
            total += count;
            if count > 0 {
                nonzero_bins += 1;
                last_nonzero_count = count;
            }
        }
        if nonzero_bins <= 1 {
            continue;
        }
        let step = (total - last_nonzero_count) / 255;
        if step == 0 {
            continue;
        }
        let mut n = step / 2;
        for value in 0..=u8::MAX {
            lut[start + usize::from(value)] = (n / step).min(255) as u8;
            n += histograms[channel][usize::from(value)];
        }
    }
    Some(lut)
}
#[cfg(test)]
mod equalize_histogram_tests {
    use super::equalize_histogram;

    #[test]
    fn banks_and_repeated_blocks_preserve_every_sample() {
        for channels in [1, 3] {
            for pixels in [0, 1, 15, 16, 17, 31, 32, 33, 16_383, 16_384, 16_385] {
                for pattern in 0..4 {
                    let data: Vec<u8> = (0..pixels * channels)
                        .map(|index| match pattern {
                            0 => 17,
                            1 => (index * 71 + index / 23) as u8,
                            2 => ((index / (channels * 43)) * 13) as u8,
                            // The ends match, but a pixel inside each candidate
                            // repeated block differs. Every byte must be checked.
                            _ => u8::from(index / channels % 16 == 7) * 119,
                        })
                        .collect();
                    let mut expected = [[0u32; 256]; 3];
                    for (index, &value) in data.iter().enumerate() {
                        expected[index % channels][usize::from(value)] += 1;
                    }
                    assert_eq!(equalize_histogram::<1>(&data, channels), expected);
                    assert_eq!(equalize_histogram::<4>(&data, channels), expected);
                }
            }
        }
    }
}
