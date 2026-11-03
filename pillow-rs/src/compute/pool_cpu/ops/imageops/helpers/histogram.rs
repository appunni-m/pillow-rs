use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, GrayImage, RgbImage};
use std::sync::Arc;
use wide::u8x16;

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

pub(crate) fn native_luma_lut_tables(lut: &[u8]) -> Option<[u8x16; 16]> {
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

pub(crate) fn apply_native_luma_lut_row(
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
    options: &crate::pipeline::AutocontrastOptions,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<Vec<u8>, PilError> {
    let no_ignored_bins = options.ignore.as_ref().map_or(true, Vec::is_empty);
    let zero_cutoff_luma = options.cutoff == (0.0, 0.0)
        && no_ignored_bins
        && !options.preserve_tone;
    let histogram = collect_autocontrast_histogram(
        img,
        mask,
        options.preserve_tone,
        zero_cutoff_luma,
    )?;
    let ignored = normalize_ignored_bins(&options.ignore)?;
    let lut = build_autocontrast_lut(&histogram, options.cutoff, &ignored)?;
    if options.preserve_tone && matches!(img, DynamicImage::ImageRgb8(_)) {
        // Pillow gathers statistics from `image.convert("L")`, then expands
        // the one-channel LUT for `_lut(image, lut)` without changing RGB mode.
        let mut rgb_lut = Vec::with_capacity(768);
        rgb_lut.extend_from_slice(&lut);
        rgb_lut.extend_from_slice(&lut);
        rgb_lut.extend_from_slice(&lut);
        return Ok(rgb_lut);
    }
    Ok(lut)
}

/// Build an autocontrast LUT from bins already collected by the binding.
///
/// The Python ImageOps facade computes the histogram before coercing
/// `ignore` and `cutoff`, matching Pillow's observable fault order. This
/// entrypoint keeps the bin arithmetic and LUT construction in Rust.
pub(crate) fn autocontrast_lut_from_histogram(
    histogram: &[u32],
    options: &crate::pipeline::AutocontrastOptions,
) -> Result<Vec<u8>, PilError> {
    if histogram.is_empty() || histogram.len() % 256 != 0 || histogram.len() > 4 * 256 {
        return Err(PilError::ValueError(
            "histogram must contain one to four complete 256-bin channels".into(),
        ));
    }
    let channels = histogram.len() / 256;
    let mut values = [[0usize; 256]; 4];
    for (channel, bins) in histogram.chunks_exact(256).enumerate() {
        for (value, &count) in values[channel].iter_mut().zip(bins) {
            *value = count as usize;
        }
    }
    let histogram = AutocontrastHistogram {
        channels,
        values,
        zero_cutoff_luma_bounds: None,
    };
    let ignored = normalize_ignored_bins(&options.ignore)?;
    build_autocontrast_lut(&histogram, options.cutoff, &ignored)
}

/// Build one channel's LUT from Pillow-computed integer cutoff counts.
pub(crate) fn autocontrast_lut_from_cutoff_counts(
    histogram: &[u32],
    cutoff_counts: (i128, i128),
) -> Result<Vec<u8>, PilError> {
    if histogram.len() != 256 {
        return Err(PilError::ValueError(
            "histogram must contain one complete 256-bin channel".into(),
        ));
    }
    let mut bins = [0usize; 256];
    for (count, &value) in bins.iter_mut().zip(histogram) {
        *count = value as usize;
    }
    let mut lut = vec![0u8; 256];
    build_lut_channel(
        &mut lut,
        bins,
        normalize_cutoff_count(cutoff_counts.0),
        normalize_cutoff_count(cutoff_counts.1),
        None,
    );
    Ok(lut)
}

struct AutocontrastHistogram {
    channels: usize,
    values: [[usize; 256]; 4],
    zero_cutoff_luma_bounds: Option<(u8, u8)>,
}

fn collect_autocontrast_histogram(
    img: &DynamicImage,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
    preserve_tone: bool,
    zero_cutoff_luma: bool,
) -> Result<AutocontrastHistogram, PilError> {
    let source_channels = img.mode().band_count();
    let tone_luma = preserve_tone && matches!(img, DynamicImage::ImageRgb8(_));
    let channels = if tone_luma { 1 } else { source_channels };
    let (width, height) = (img.width(), img.height());
    let image_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    let mask = mask
        .map(|mask| mask.materialize_for_ops())
        .transpose()?;
    // The ImageOps contract admits `1` and `L` masks. Both already use the
    // exact byte plane consumed by Pillow's nonzero mask predicate.
    let mask_raw = match mask.as_ref() {
        Some(DynamicImage::One(mask) | DynamicImage::ImageLuma8(mask)) => {
            Some(mask.as_raw().as_slice())
        }
        Some(_) => unreachable!("ImageOps contract admits only 1 or L masks"),
        None => None,
    };
    let raw = img.as_bytes();
    let stride = width as usize * source_channels;
    let expected_len = image_pixels.checked_mul(source_channels);
    let zero_cutoff_luma_bounds = if zero_cutoff_luma
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
                if let Some(mask) = mask_raw {
                    let mask_index = y * width as usize + x;
                    if mask[mask_index] == 0 {
                        continue;
                    }
                }
                let index = y * stride + x * source_channels;
                if tone_luma {
                    let luma = crate::color::rgb_to_luma_u8(
                        raw[index],
                        raw[index + 1],
                        raw[index + 2],
                    );
                    values[0][usize::from(luma)] += 1;
                } else {
                    for channel in 0..channels {
                        values[channel][raw[index + channel] as usize] += 1;
                    }
                }
            }
        }
    }
    Ok(AutocontrastHistogram {
        channels,
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

fn normalize_ignored_bins(ignore: &Option<Vec<i64>>) -> Result<Vec<usize>, PilError> {
    let Some(ignore) = ignore else {
        return Ok(Vec::new());
    };
    ignore
        .iter()
        .map(|&index| {
            let normalized = if index < 0 { index + 256 } else { index };
            usize::try_from(normalized)
                .ok()
                .filter(|&index| index < 256)
                .ok_or_else(|| PilError::IndexError("list assignment index out of range".into()))
        })
        .collect()
}

fn validate_cutoff(total: usize, cutoff: f64) -> Result<(), PilError> {
    let count = total as f64 * cutoff / 100.0;
    if count.is_nan() {
        return Err(PilError::ValueError("cannot convert float NaN to integer".into()));
    }
    if count.is_infinite() {
        return Err(PilError::OverflowError("cannot convert float infinity to integer".into()));
    }
    Ok(())
}

fn cutoff_count(total: usize, cutoff: f64) -> Result<(bool, usize), PilError> {
    validate_cutoff(total, cutoff)?;
    let count = (total as f64 * cutoff / 100.0).floor();
    let negative = count < 0.0;
    let magnitude = count.abs();
    let magnitude = if magnitude >= usize::MAX as f64 {
        usize::MAX
    } else {
        magnitude as usize
    };
    Ok((negative, magnitude))
}

fn normalize_cutoff_count(count: i128) -> (bool, usize) {
    let negative = count < 0;
    let magnitude = count.saturating_abs().min(usize::MAX as i128) as usize;
    (negative, magnitude)
}

fn trim_histogram_edge(histogram: &mut [usize; 256], cutoff: (bool, usize), high: bool) {
    if cutoff.0 {
        let edge = if high { &mut histogram[255] } else { &mut histogram[0] };
        *edge = edge.saturating_add(cutoff.1);
        return;
    }
    let mut remaining = cutoff.1;
    if high {
        for bin in histogram.iter_mut().rev() {
            let removed = (*bin).min(remaining);
            *bin -= removed;
            remaining -= removed;
            if remaining == 0 {
                break;
            }
        }
    } else {
        for bin in histogram.iter_mut() {
            let removed = (*bin).min(remaining);
            *bin -= removed;
            remaining -= removed;
            if remaining == 0 {
                break;
            }
        }
    }
}

fn build_autocontrast_lut(
    histogram: &AutocontrastHistogram,
    cutoff: (f64, f64),
    ignored: &[usize],
) -> Result<Vec<u8>, PilError> {
    let mut lut = vec![0u8; histogram.channels * 256];
    for channel in 0..histogram.channels {
        let mut bins = histogram.values[channel];
        for &index in ignored {
            bins[index] = 0;
        }
        let total = bins.iter().copied().sum();
        let low_cutoff = cutoff_count(total, cutoff.0)?;
        let high_cutoff = cutoff_count(total, cutoff.1)?;
        let start = channel * 256;
        build_lut_channel(
            &mut lut[start..start + 256],
            bins,
            low_cutoff,
            high_cutoff,
            histogram.zero_cutoff_luma_bounds,
        );
    }
    Ok(lut)
}

fn build_lut_channel(
    lut: &mut [u8],
    mut bins: [usize; 256],
    low_cutoff: (bool, usize),
    high_cutoff: (bool, usize),
    zero_cutoff_luma_bounds: Option<(u8, u8)>,
) {
    if let Some((low, high)) = zero_cutoff_luma_bounds {
        fill_channel_from_bounds(lut, low, high);
        return;
    }
    trim_histogram_edge(&mut bins, low_cutoff, false);
    trim_histogram_edge(&mut bins, high_cutoff, true);
    let low = bins.iter().position(|&count| count != 0);
    let high = bins.iter().rposition(|&count| count != 0);
    match (low, high) {
        (Some(low), Some(high)) if high > low => {
            fill_contrast_channel(lut, low as f64, high as f64)
        }
        _ => fill_identity_channel(lut),
    }
}

fn fill_channel_from_bounds(channel: &mut [u8], low: u8, high: u8) {
    if high <= low {
        fill_identity_channel(channel);
    } else {
        fill_contrast_channel(channel, f64::from(low), f64::from(high));
    }
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
        let lut = autocontrast_lut(
            &image,
            &crate::pipeline::AutocontrastOptions::symmetric(0.0),
            None,
        )
        .expect("cutoff-zero LUT");
        assert_eq!(lut[22], 0);
        assert_eq!(lut[23], 0);
        assert_eq!(lut[50], 39);
        assert_eq!(lut[127], 153);
        assert_eq!(lut[196], 255);
        assert_eq!(lut[197], 255);

        let constant = DynamicImage::ImageLuma8(
            GrayImage::from_raw(19, 1, vec![77; 19]).expect("valid constant L image"),
        );
        let constant_lut = autocontrast_lut(
            &constant,
            &crate::pipeline::AutocontrastOptions::symmetric(0.0),
            None,
        )
        .expect("constant LUT");
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
    equalize_lut_for_bytes(
        img.as_bytes(),
        img.width(),
        img.height(),
        channels,
        mask,
    )
}

pub(crate) fn equalize_lut_for_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    mask: Option<&DynamicImage>,
) -> Option<Vec<u8>> {
    if !matches!(channels, 1 | 3) {
        return None;
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?;
    if raw.len() != expected_len {
        return None;
    }

    let mut histograms = [[0u32; 256]; 3];
    // Masked selection retains its exact nonzero-byte predicate; unmasked
    // byte images can combine repeated samples without examining a mask.
    if let Some(mask) = mask {
        if (mask.width(), mask.height()) != (width, height)
            || !matches!(mask, DynamicImage::One(_) | DynamicImage::ImageLuma8(_))
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
pub(crate) fn lut_is_identity(lut: &[u8], channels: usize) -> bool {
    lut.len() == channels.saturating_mul(256)
        && lut.chunks_exact(256).all(|table| {
            table
                .iter()
                .enumerate()
                .all(|(index, &value)| value == index as u8)
        })
}

/// Applies native L/RGB lookup tables using the archived portable-vector path.
/// Partial L blocks and RGB pixel groups are padded without exposing padding.
pub(crate) fn apply_native_lut(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    lut: &[u8],
) -> Option<(Vec<u8>, u64, u64)> {
    if !matches!(channels, 1 | 3) || lut.len() != channels.checked_mul(256)? {
        return None;
    }
    let row_stride = width.checked_mul(channels)?;
    let expected_len = row_stride.checked_mul(height)?;
    if source.len() != expected_len {
        return None;
    }
    let tables: Vec<[u8x16; 16]> = (0..channels)
        .map(|channel| native_luma_lut_tables(&lut[channel * 256..(channel + 1) * 256]))
        .collect::<Option<_>>()?;
    let mut output = vec![0; source.len()];
    if channels == 1 {
        for (source_row, output_row) in source
            .chunks_exact(row_stride.max(1))
            .zip(output.chunks_exact_mut(row_stride.max(1)))
        {
            apply_native_luma_lut_row(source_row, output_row, &tables[0]);
        }
    } else if row_stride != 0 {
        for (source_row, output_row) in source
            .chunks_exact(row_stride)
            .zip(output.chunks_exact_mut(row_stride))
        {
            for (source_block, output_block) in
                source_row.chunks(48).zip(output_row.chunks_mut(48))
            {
                let active_pixels = source_block.len() / 3;
                let mut input = [[0u8; 16]; 3];
                for (lane, pixel) in source_block.chunks_exact(3).enumerate() {
                    input[0][lane] = pixel[0];
                    input[1][lane] = pixel[1];
                    input[2][lane] = pixel[2];
                }
                let mapped: [[u8; 16]; 3] = std::array::from_fn(|channel| {
                    native_luma_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
                });
                for lane in 0..active_pixels {
                    let start = lane * 3;
                    output_block[start] = mapped[0][lane];
                    output_block[start + 1] = mapped[1][lane];
                    output_block[start + 2] = mapped[2][lane];
                }
            }
        }
    }
    let blocks = width
        .div_ceil(16)
        .saturating_mul(channels)
        .saturating_mul(height) as u64;
    Some((output, blocks, 0))
}

pub(crate) fn apply_luma_architecture(
    input: &GrayImage,
    lut: &[u8],
    equalize: bool,
) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    if width == 0 || height == 0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(DynamicImage::ImageLuma8(input.clone()));
    }
    if equalize && lut_is_identity(lut, 1) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(DynamicImage::ImageLuma8(input.clone()));
    }
    if equalize && input.as_raw().len() <= 4096 {
        let output = input
            .as_raw()
            .iter()
            .map(|&sample| lut[usize::from(sample)])
            .collect();
        crate::compute::record_pipeline_operation_scalar_tail(input.as_raw().len() as u64);
        crate::compute::record_pipeline_operation_path("scalar-lut");
        return crate::image_utils::raw_bytes_to_image(width, height, output, 1);
    }
    let (output, blocks, tail) = apply_native_lut(
        input.as_raw(),
        width as usize,
        height as usize,
        1,
        lut,
    )
    .ok_or_else(|| PilError::InternalError("histogram L buffer shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(width, height, output, 1)
}

pub(crate) fn apply_rgb_architecture(
    input: &RgbImage,
    lut: &[u8],
    equalize: bool,
) -> Result<DynamicImage, PilError> {
    let (width, height) = input.dimensions();
    if width == 0 || height == 0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(DynamicImage::ImageRgb8(input.clone()));
    }
    if equalize && lut_is_identity(lut, 3) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(DynamicImage::ImageRgb8(input.clone()));
    }
    let (output, blocks, tail) = apply_native_lut(
        input.as_raw(),
        width as usize,
        height as usize,
        3,
        lut,
    )
    .ok_or_else(|| PilError::InternalError("histogram RGB buffer shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(width, height, output, 3)
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
