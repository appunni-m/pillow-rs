//! Native sample scans reused by exact-mode Default Histogram owners.
//!
//! CoreOnly 16-bit and float channels keep the public 256-bin-per-channel
//! shape. Each bin is calculated directly from the native typed sample; no
//! mode conversion, projected raster, or intermediate sample buffer is used.
//! Scans are bounded by logical image pixels; trailing carrier storage is ignored.
//! The CoreOnly contract favors a single typed scan over reusing byte-mode
//! readers. Its frozen extension cases must cover every boundary below.
// TODO(pillow-rs#3ptb): freeze CoreOnly histogram parity for u16 rounding,
// f32 clamp/NaN/rounding, RGB32F's opaque fourth band, and masked scans.

use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};

/// Counts the stored byte channels directly, without constructing another raster.
// Pillow 12.2's unpackLA handles LA, La, and PA raw inputs by repeating the
// first band into physical lanes 1 and 2. `_imaging.c::getink` does the same
// for two-component tuple inks, and Access.c reads the pair from lanes 0 and 3.
// The typed carrier stores only that pair, so ordinary inputs carry the
// omitted lanes in the raster payload; legacy carriers without them use band 0.
pub(crate) fn byte_bands<P, const MASKED_SECOND_FROM_FIRST_BAND: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32>
where
    P: Pixel<Subpixel = u8>,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    let mut histogram = vec![0u32; 256 * channels];
    let hidden_lanes = image.hidden_pixel_lanes();
    if let Some(mask) = mask_raw {
        for (index, pixel) in image.pixels().enumerate() {
            if mask[index] == 0 {
                continue;
            }
            let samples = pixel.channels();
            for (band, &stored_sample) in samples.iter().enumerate() {
                // Masked Pillow Histo.c reads raw lane 1; the unmasked two-band
                // path reads lane 3. Recover lane 1 for ordinary two-band inputs.
                let sample = if MASKED_SECOND_FROM_FIRST_BAND && band == 1 {
                    hidden_lanes
                        .filter(|lanes| lanes.lane_count() > 0)
                        .and_then(|lanes| lanes.for_pixel(index))
                        .and_then(|lanes| lanes.first())
                        .copied()
                        .unwrap_or(samples[0])
                } else {
                    stored_sample
                };
                histogram[band * 256 + usize::from(sample)] += 1;
            }
        }
    } else {
        for pixel in image.pixels() {
            for (band, &sample) in pixel.channels().iter().enumerate() {
                histogram[band * 256 + usize::from(sample)] += 1;
            }
        }
    }
    histogram
}

/// Bins native u16 channels into the established 256-bin output directly.
///
/// `NO_PILLOW_SUPPORT`: this native multiband u16 Histogram route is an
/// internal Rust mode extension; Pillow has no corresponding mode identity.
///
/// `(sample + 128) / 257` is the same rounded full-range mapping as
/// `FromPrimitive<u16> for u8`, used by the archived byte projection. The
/// source buffer remains u16; only the destination bin index is calculated.
pub(crate) fn u16_bands<P>(image: &ImageBuffer<P, Vec<u16>>, mask_raw: Option<&[u8]>) -> Vec<u32>
where
    P: Pixel<Subpixel = u16>,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    let mut histogram = vec![0u32; 256 * channels];
    if let Some(mask) = mask_raw {
        for (index, pixel) in image.pixels().enumerate() {
            if mask[index] == 0 {
                continue;
            }
            for (band, &sample) in pixel.channels().iter().enumerate() {
                let bin = (u32::from(sample) + 128) / 257;
                histogram[band * 256 + bin as usize] += 1;
            }
        }
    } else {
        for pixel in image.pixels() {
            for (band, &sample) in pixel.channels().iter().enumerate() {
                let bin = (u32::from(sample) + 128) / 257;
                histogram[band * 256 + bin as usize] += 1;
            }
        }
    }
    histogram
}

/// Bins native f32 channels into 256 normalized-value bins directly.
///
/// `NO_PILLOW_SUPPORT`: this native multiband f32 Histogram route is an
/// internal Rust mode extension; Pillow has no corresponding mode identity.
///
/// Clamping, NaN handling, fused scaling, and rounding match
/// `FromPrimitive<f32> for u8`, which the archived byte projection used. Each
/// bin is computed from the f32 sample without materializing that projected
/// byte or another sample buffer.
pub(crate) fn f32_bands<P>(image: &ImageBuffer<P, Vec<f32>>, mask_raw: Option<&[u8]>) -> Vec<u32>
where
    P: Pixel<Subpixel = f32>,
{
    f32_bands_impl::<P, false>(image, mask_raw)
}

/// `NO_PILLOW_SUPPORT`: RGB32F is an internal mode, including its synthesized
/// opaque fourth Histogram band.
///
/// Preserves RGB32F's established four-band Histogram result without building
/// the projected RGBA8 image; the final band is opaque for included pixels.
pub(crate) fn f32_rgb_bands_with_opaque_alpha(
    image: &ImageBuffer<crate::raster::Rgb<f32>, Vec<f32>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    f32_bands_impl::<crate::raster::Rgb<f32>, true>(image, mask_raw)
}

fn f32_bands_impl<P, const OPAQUE_ALPHA: bool>(
    image: &ImageBuffer<P, Vec<f32>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32>
where
    P: Pixel<Subpixel = f32>,
{
    let channels = usize::from(P::CHANNEL_COUNT);
    let output_channels = channels + if OPAQUE_ALPHA { 1 } else { 0 };
    let mut histogram = vec![0u32; 256 * output_channels];
    let mut included_pixels = 0u32;
    if let Some(mask) = mask_raw {
        for (index, pixel) in image.pixels().enumerate() {
            if mask[index] == 0 {
                continue;
            }
            if OPAQUE_ALPHA {
                included_pixels += 1;
            }
            for (band, &sample) in pixel.channels().iter().enumerate() {
                let bin = f32_byte_bin(sample);
                histogram[band * 256 + bin] += 1;
            }
        }
    } else {
        for pixel in image.pixels() {
            if OPAQUE_ALPHA {
                included_pixels += 1;
            }
            for (band, &sample) in pixel.channels().iter().enumerate() {
                let bin = f32_byte_bin(sample);
                histogram[band * 256 + bin] += 1;
            }
        }
    }
    if OPAQUE_ALPHA {
        histogram[channels * 256 + 255] = included_pixels;
    }
    histogram
}

#[inline]
fn f32_byte_bin(sample: f32) -> usize {
    let normalized = if sample < 1.0 { sample.max(0.0) } else { 1.0 };
    normalized.mul_add(255.0, 0.0).round() as usize
}

/// Preserves Pillow's I;16 byte-prefix histogram using native-endian samples.
///
/// Pillow 12.2 aliases its row pointers as `image8` for two-byte pixels, then
/// reads `image8[y][x]` in `Histo.c`. That counts the first `width` raw bytes
/// of every row, restarting the byte lane at each row; odd widths do not
/// continue the previous row's high/low-byte alternation.
// TODO(pillow-rs#3ptb): freeze exact I;16 endian, odd-width row-prefix, and
// masked byte-selection parity cases.
pub(crate) fn l16_byte_prefix_native(
    image: &ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    l16_byte_prefix::<{ cfg!(target_endian = "big") }>(image, mask_raw)
}

/// Preserves Pillow's little-endian I;16L byte-prefix histogram.
pub(crate) fn l16_byte_prefix_little_endian(
    image: &ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    l16_byte_prefix::<false>(image, mask_raw)
}

/// Preserves Pillow's big-endian I;16B byte-prefix histogram.
pub(crate) fn l16_byte_prefix_big_endian(
    image: &ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    l16_byte_prefix::<true>(image, mask_raw)
}

/// Bins the archived byte prefix while keeping endian choice compile-time.
fn l16_byte_prefix<const BIG_ENDIAN: bool>(
    image: &ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    let (width, height) = image.dimensions();
    let width = width as usize;
    let mut histogram = vec![0u32; 256];
    let raw = image.as_raw();
    if let Some(mask) = mask_raw {
        for y in 0..height as usize {
            let row_start = y * width;
            let samples = &raw[row_start..row_start + width];
            let mask_row = &mask[row_start..row_start + width];
            for (x, &included) in mask_row.iter().enumerate() {
                if included == 0 {
                    continue;
                }
                histogram[usize::from(l16_prefix_byte::<BIG_ENDIAN>(samples, x))] += 1;
            }
        }
    } else {
        for y in 0..height as usize {
            let row_start = y * width;
            let samples = &raw[row_start..row_start + width];
            for x in 0..width {
                histogram[usize::from(l16_prefix_byte::<BIG_ENDIAN>(samples, x))] += 1;
            }
        }
    }
    histogram
}

#[inline(always)]
fn l16_prefix_byte<const BIG_ENDIAN: bool>(samples: &[u16], x: usize) -> u8 {
    let lane = x & 1;
    let byte_shift = if BIG_ENDIAN {
        (lane ^ 1) << 3
    } else {
        lane << 3
    };
    (samples[x >> 1] >> byte_shift) as u8
}

/// Matches Pillow's extrema-scaled histogram for unmasked signed integer data.
pub(crate) fn i32_scalar(
    image: &crate::raster::RgbaImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    if mask_raw.is_some() {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let mut samples = image
        .pixels()
        .map(|pixel| read_i32_sample(pixel.channels()));
    let Some(first) = samples.next() else {
        return Err(PilError::ValueError("min/max not given".into()));
    };
    let (mut minimum, mut maximum) = (first, first);
    for value in samples {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    i32_scalar_with_extrema(image, None, minimum, maximum)
}

/// Scans native I samples using Pillow's float32 scale and truncating bin cast.
// NO_PILLOW_SUPPORT: Pillow 12.2 Histo.c subtracts INT32 extrema and samples
// in signed C arithmetic, whose overflow is undefined. Rust defines those
// edge cases with two's-complement wrapping before the float32 scaling.
// TODO(pillow-rs#3ptb): freeze the full-range INT32 extrema behavior against
// the pinned Pillow build; the C source does not define the overflow result.
pub(crate) fn i32_scalar_with_extrema(
    image: &crate::raster::RgbaImage,
    mask_raw: Option<&[u8]>,
    minimum: i32,
    maximum: i32,
) -> Result<Vec<u32>, PilError> {
    if mask_raw.is_some() {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let mut histogram = vec![0u32; 256];
    if image.width() == 0 || image.height() == 0 || minimum >= maximum {
        return Ok(histogram);
    }
    let scale = 255.0_f32 / maximum.wrapping_sub(minimum) as f32;
    for pixel in image.pixels() {
        let value = read_i32_sample(pixel.channels());
        let scaled = (value.wrapping_sub(minimum) as f32 * scale).trunc() as i32;
        if (0..256).contains(&scaled) {
            histogram[scaled as usize] += 1;
        }
    }
    Ok(histogram)
}

/// Matches Pillow's extrema-scaled histogram for unmasked float data.
pub(crate) fn f32_scalar(
    image: &crate::raster::RgbaImage,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    if mask_raw.is_some() {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let mut samples = image
        .pixels()
        .map(|pixel| read_f32_sample(pixel.channels()));
    let Some(first) = samples.next() else {
        return Err(PilError::ValueError("min/max not given".into()));
    };
    let (mut minimum, mut maximum) = (first, first);
    for value in samples {
        if value < minimum {
            minimum = value;
        }
        if value > maximum {
            maximum = value;
        }
    }
    f32_scalar_with_extrema(image, None, minimum, maximum)
}

/// Scans native F samples using Pillow's float32 scale, truncation, and range.
// NO_PILLOW_SUPPORT: Pillow 12.2 Histo.c casts the scaled FLOAT32 directly to
// C int. Rust omits non-finite/out-of-i32-range scaled results before casting,
// defining the NaN and infinity cases that C leaves undefined.
// TODO(pillow-rs#3ptb): freeze explicit-extrema NaN/infinity and finite range
// boundaries against the pinned Pillow build after source migration.
pub(crate) fn f32_scalar_with_extrema(
    image: &crate::raster::RgbaImage,
    mask_raw: Option<&[u8]>,
    minimum: f32,
    maximum: f32,
) -> Result<Vec<u32>, PilError> {
    if mask_raw.is_some() {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let mut histogram = vec![0u32; 256];
    if image.width() == 0 || image.height() == 0 || minimum >= maximum {
        return Ok(histogram);
    }
    let scale = 255.0_f32 / (maximum - minimum);
    for pixel in image.pixels() {
        let value = read_f32_sample(pixel.channels());
        let scaled = ((value - minimum) * scale).trunc();
        if scaled.is_nan() {
            // The pinned Pillow build's float-to-int conversion bins NaN as
            // zero for F histograms, including all-NaN degenerate images.
            histogram[0] += 1;
        } else if scaled.is_finite() && (0.0..256.0).contains(&scaled) {
            histogram[scaled as usize] += 1;
        }
    }
    Ok(histogram)
}

#[inline(always)]
fn read_i32_sample(sample: &[u8]) -> i32 {
    i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
}

#[inline(always)]
fn read_f32_sample(sample: &[u8]) -> f32 {
    f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]])
}
