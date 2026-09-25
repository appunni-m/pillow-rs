//! ImageOps CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible image operations: autocontrast, equalize,
//! invert, flip, mirror, posterize, solarize, grayscale, colorize,
//! contain, cover, fit, pad, scale, expand, and crop border.

use crate::raster::DynamicImage;

use crate::color::pil_grayscale;

/// Python 3's round() (banker's rounding): rounds half to even.
/// This matches PIL's behavior: round(12.5) -> 12, round(13.5) -> 14.
fn bankers_round(x: f64) -> f64 {
    let floor = x.floor();
    let frac = x - floor;
    if frac == 0.5 {
        if floor % 2.0 == 0.0 {
            floor
        } else {
            floor + 1.0
        }
    } else {
        (x + 0.5).floor()
    }
}

use super::geometry::execute_resize;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::pil_resize::pil_resize;
use crate::ops::pil_resize::pil_resize_boxed;
use crate::pipeline::ResampleFilter;

#[cfg(feature = "parallel")]
const POINT_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

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
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let image_pixels = CheckedDims::new(w, h, 1)?.total_pixels();
    let mask = mask
        .map(|mask| mask.materialize_for_ops())
        .transpose()?
        .map(|mask| mask.to_luma8());
    let mut selected_pixels = if mask.is_none() { image_pixels } else { 0 };
    let raw = img.as_bytes();
    let stride = w as usize * channels;
    let mut histograms = [[0usize; 256]; 4];

    // The SIMD adapter shares this scalar control plane, so keep its hot
    // unmasked L/RGB case on the existing banked native-byte histogram. The
    // independent counters reduce the dependency chain for repeated samples;
    // the mask and other native layouts retain the general semantic loop.
    let expected_len = image_pixels.checked_mul(channels);
    let native_histogram = if mask.is_none()
        && expected_len == Some(raw.len())
        && matches!(
            (img, channels),
            (DynamicImage::ImageLuma8(_), 1) | (DynamicImage::ImageRgb8(_), 3)
        ) {
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
    } else {
        false
    };

    if !native_histogram {
        for y in 0..h as usize {
            for x in 0..w as usize {
                if let Some(mask) = mask.as_ref() {
                    if mask.get_pixel(x as u32, y as u32)[0] == 0 {
                        continue;
                    }
                    selected_pixels += 1;
                }
                let index = y * stride + x * channels;
                for c in 0..channels {
                    histograms[c][raw[index + c] as usize] += 1;
                }
            }
        }
    }

    let mut lut = vec![0u8; channels * 256];
    // Pillow's all-zero mask produces an identity LUT rather than dividing by
    // a zero-sized histogram. Filling identity entries also lets a vector
    // backend keep its native data path for this valid no-op result.
    if selected_pixels == 0 {
        for channel in 0..channels {
            for value in 0..=u8::MAX {
                lut[channel * 256 + usize::from(value)] = value;
            }
        }
        return Ok(lut);
    }

    let total = selected_pixels as f64;
    for channel in 0..channels {
        let low_thresh = (total * cutoff / 100.0) as usize;
        let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;
        let lo = histogram_value_at(&histograms[channel], low_thresh, 0) as f64;
        let hi = histogram_value_at(
            &histograms[channel],
            high_thresh.min(selected_pixels - 1),
            255,
        ) as f64;
        let start = channel * 256;
        if hi <= lo {
            for value in 0..=u8::MAX {
                lut[start + usize::from(value)] = value;
            }
            continue;
        }

        let scale = 255.0 / (hi - lo);
        let offset = -lo * scale;
        for value in 0..=u8::MAX {
            // PIL: int(ix * scale + offset) with clamping to [0,255].
            let mapped = f64::from(value) * scale + offset;
            lut[start + usize::from(value)] = if mapped < 0.0 {
                0
            } else if mapped > 255.0 {
                255
            } else {
                mapped as u8
            };
        }
    }
    Ok(lut)
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

#[inline]
fn apply_autocontrast_row(
    raw: &[u8],
    raw_start: usize,
    row: &mut [u8],
    channels: usize,
    lut: &[u8],
) {
    for (index, output) in row.iter_mut().enumerate() {
        let channel = index % channels;
        *output = lut[channel * 256 + usize::from(raw[raw_start + index])];
    }
}

fn apply_point_rows<F>(bytes: &mut [u8], width: usize, height: usize, transform: F)
where
    F: Fn(&mut [u8]) + Send + Sync,
{
    #[cfg(feature = "parallel")]
    let stride = width.saturating_mul(3);
    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(bytes, stride, height, |_row_start, _row_end, _y, row| {
            transform(row);
        });
    } else {
        transform(bytes);
    }
    #[cfg(not(feature = "parallel"))]
    let _ = (width, height);
    #[cfg(not(feature = "parallel"))]
    transform(bytes);
}

#[inline]
fn expand_rgba_row(
    row: &mut [u8],
    y: usize,
    source: &[u8],
    source_stride: usize,
    offset_x: usize,
    offset_y: usize,
    copy_width: usize,
    copy_height: usize,
    fill: [u8; 4],
) {
    for pixel in row.chunks_exact_mut(4) {
        pixel.copy_from_slice(&fill);
    }

    if y < offset_y || y >= offset_y.saturating_add(copy_height) {
        return;
    }
    let source_start = (y - offset_y).saturating_mul(source_stride);
    let copy_bytes = copy_width.saturating_mul(4);
    if copy_bytes == 0 || offset_x >= row.len() || source_start >= source.len() {
        return;
    }
    let output_start = offset_x.saturating_mul(4);
    if output_start >= row.len() {
        return;
    }
    let output_end = output_start.saturating_add(copy_bytes).min(row.len());
    let available = output_end - output_start;
    let source_end = source_start.saturating_add(available).min(source.len());
    let available = source_end - source_start;
    row[output_start..output_start + available].copy_from_slice(&source[source_start..source_end]);
}

/// Autocontrast: stretch image contrast based on histogram cutoff.
/// PIL: per-channel histogram, find lo/hi at cutoff percentiles for each channel,
/// then linearly map [lo, hi] to [0, 255] using truncation (int() cast).
pub fn op_autocontrast(
    img: &DynamicImage,
    cutoff: f64,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    // Pillow's ImageOps.autocontrast preserves an empty image instead of
    // sending its zero-pixel buffer through the ordinary allocation guard.
    // Keep this public behavior here so the CPU lane matches the SIMD scalar
    // implementation and retains the source mode for a 0×0 result.
    if w == 0 || h == 0 {
        return Ok(img.clone());
    }
    #[cfg(feature = "parallel")]
    let image_pixels = CheckedDims::new(w, h, 1)?.total_pixels();
    let lut = autocontrast_lut(img, cutoff, mask)?;
    let raw = img.as_bytes();
    let mut out = raw.to_vec();
    let stride = w as usize * channels;

    #[cfg(feature = "parallel")]
    if image_pixels >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut out,
            stride,
            h as usize,
            |row_start, _row_end, _y, row| {
                apply_autocontrast_row(raw, row_start, row, channels, &lut);
            }
        );
    } else {
        for y in 0..h as usize {
            let row_start = y * stride;
            apply_autocontrast_row(
                raw,
                row_start,
                &mut out[row_start..row_start + stride],
                channels,
                &lut,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for y in 0..h as usize {
        let row_start = y * stride;
        apply_autocontrast_row(
            raw,
            row_start,
            &mut out[row_start..row_start + stride],
            channels,
            &lut,
        );
    }
    let result = crate::image_utils::raw_bytes_to_image(w, h, out, channels)?;
    Ok(preserve_mode(img, result))
}

/// Equalize: histogram equalization matching PIL's algorithm.
/// Build LUT from non-zero histogram bins, using PIL's step formula.
pub fn op_equalize(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    op_equalize_with_mask(img, None)
}

/// Equalize with a validated histogram mask, retaining native L/RGB storage.
pub(crate) fn op_equalize_with_mask(
    img: &DynamicImage,
    mask: Option<&std::sync::Arc<crate::image::Image>>,
) -> Result<DynamicImage, PilError> {
    let mask = mask.map(|mask| mask.materialized_shared()).transpose()?;
    let converted;
    let source = if matches!(
        img,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
    ) {
        img
    } else {
        converted = DynamicImage::ImageRgb8(img.to_rgb8());
        &converted
    };
    let channels = usize::from(source.color().channel_count());
    let lut = equalize_lut(source, channels, mask.as_deref())
        .ok_or_else(|| PilError::ValueError("images do not match".into()))?;
    let result = if super::super::point_lut_is_identity(&lut, channels) {
        source.clone()
    } else {
        super::effects::op_eval(source, &lut)?
    };
    Ok(preserve_mode(img, result))
}

/// Invert: subtract each pixel value from 255 (all channels, matching PIL's point()).
pub fn op_invert(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let raw = img.as_bytes();
    let out = raw.iter().map(|value| u8::MAX - *value).collect();
    let result = match channels {
        1 => crate::raster::GrayImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLuma8)
            .ok_or_else(|| PilError::InternalError("invert L buffer shape mismatch".to_string()))?,
        2 => crate::raster::GrayAlphaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("invert LA buffer shape mismatch".to_string())
            })?,
        3 => crate::raster::RgbImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGB buffer shape mismatch".to_string())
            })?,
        _ => crate::raster::RgbaImage::from_raw(w, h, out)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| {
                PilError::InternalError("invert RGBA buffer shape mismatch".to_string())
            })?,
    };
    Ok(result)
}

/// Flip vertically.
pub fn op_flip(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    Ok(img.flipv())
}

/// Mirror horizontally.
pub fn op_mirror(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    Ok(img.fliph())
}

/// Posterize: reduce the number of bits per channel.
pub fn op_posterize(img: &DynamicImage, bits: u8) -> Result<DynamicImage, PilError> {
    let mask = !((1u8 << (8 - bits)) - 1);
    let mut rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    apply_point_rows(rgb.as_mut(), width as usize, height as usize, |row| {
        for pixel in row.chunks_exact_mut(3) {
            for channel in pixel {
                *channel &= mask;
            }
        }
    });
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Solarize: invert pixels where value >= threshold.
/// PIL uses >=, not >.
pub fn op_solarize(img: &DynamicImage, threshold: u8) -> Result<DynamicImage, PilError> {
    if matches!(
        img,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
    ) {
        // Every stored byte is a color sample in the public L/RGB contract.
        // Write the result once instead of expanding L to RGB and converting back.
        let bytes = img
            .as_bytes()
            .iter()
            .map(|&value| {
                if value >= threshold {
                    255 - value
                } else {
                    value
                }
            })
            .collect();
        let channels = if matches!(img, DynamicImage::ImageLuma8(_)) {
            1
        } else {
            3
        };
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            bytes,
            channels,
        );
    }
    let t = threshold;
    let mut rgb = img.to_rgb8();
    let (width, height) = rgb.dimensions();
    apply_point_rows(rgb.as_mut(), width as usize, height as usize, |row| {
        for pixel in row.chunks_exact_mut(3) {
            for channel in pixel {
                if *channel >= t {
                    // PIL uses >=, not >
                    *channel = u8::MAX - *channel;
                }
            }
        }
    });
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

/// Grayscale: convert to L-mode using Pillow's source-mode conversion path.
///
/// `ImageOps.grayscale` delegates to `Image.convert("L")`, whose C dispatch
/// does not reinterpret every source buffer as RGB. In particular, `I` and
/// `F` contain one scalar sample in a four-byte transport, `1` treats every
/// non-zero sample as white, and YCbCr copies its Y band directly. Keeping
/// those source semantics here is required before a later operation receives
/// the new L-mode segment.
pub fn op_grayscale(img: &DynamicImage, mode: Option<&str>) -> Result<DynamicImage, PilError> {
    let gray = match mode {
        Some("La") => {
            return Err(PilError::ValueError(
                "conversion from La to L not supported".into(),
            ));
        }
        // RGBa's fallback converter first restores RGB with integer division;
        // alpha zero preserves the stored color and over-alpha samples clip.
        Some("RGBa") => pil_grayscale(&crate::ops::pil_resize::unpremultiply_alpha(img))?,
        Some("I") => match crate::color::i_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("i_to_l always returns L mode"),
        },
        Some("F") => match crate::color::f_to_l(img) {
            DynamicImage::ImageLuma8(gray) => gray,
            _ => unreachable!("f_to_l always returns L mode"),
        },
        Some("1") => {
            let mut gray = img.to_luma8();
            for pixel in gray.pixels_mut() {
                pixel[0] = if pixel[0] == 0 { 0 } else { u8::MAX };
            }
            gray
        }
        Some("CMYK") => crate::color::cmyk_to_grayscale(img)?,
        Some("HSV") => crate::color::pil_grayscale(&crate::color::hsv_to_rgb(img))?,
        Some("YCbCr") => {
            // Pillow's Convert.c maps YCbCr→L through the Y band directly,
            // not through an RGB round trip and a second luma calculation.
            let source = img.to_rgb8();
            crate::raster::GrayImage::from_fn(source.width(), source.height(), |x, y| {
                crate::raster::Luma([source.get_pixel(x, y)[0]])
            })
        }
        _ => pil_grayscale(img)?,
    };
    Ok(DynamicImage::ImageLuma8(gray))
}

/// Colorize: map grayscale values to a two-color gradient.
/// Always outputs RGB (PIL behavior).
///
/// PIL builds a 256-entry LUT per channel in ``ImageOps.colorize`` using
/// floor integer division (``//``), then applies it via ``ImageOps._lut``.
/// The mapping supports optional three-color ``mid`` plus blackpoint /
/// midpoint / whitepoint positions; this is replicated exactly so negative
/// color deltas round the same way.
pub fn colorize_lut(
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> [[u8; 256]; 3] {
    let mut lut = [[0u8; 256]; 3];
    for channel in 0..3 {
        let black_c = [black.0, black.1, black.2][channel] as i32;
        let white_c = [white.0, white.1, white.2][channel] as i32;
        let mid_c = mid.map(|m| [m.0, m.1, m.2][channel] as i32);
        let bp = blackpoint as i32;
        let mp = midpoint as i32;
        let wp = whitepoint as i32;
        for (index, slot) in lut[channel].iter_mut().enumerate() {
            let index = index as i32;
            let value = if index < bp {
                black_c
            } else if let Some(mid_c) = mid_c {
                if index < mp {
                    let span = mp - bp;
                    let step = if span == 0 {
                        0
                    } else {
                        ((index - bp) * (mid_c - black_c)).div_euclid(span)
                    };
                    black_c + step
                } else if index < wp {
                    let span = wp - mp;
                    let step = if span == 0 {
                        0
                    } else {
                        ((index - mp) * (white_c - mid_c)).div_euclid(span)
                    };
                    mid_c + step
                } else {
                    white_c
                }
            } else if index < wp {
                let span = wp - bp;
                let step = if span == 0 {
                    0
                } else {
                    ((index - bp) * (white_c - black_c)).div_euclid(span)
                };
                black_c + step
            } else {
                white_c
            };
            *slot = value.clamp(0, 255) as u8;
        }
    }
    lut
}

pub fn op_colorize(
    img: &DynamicImage,
    black: &(u8, u8, u8),
    white: &(u8, u8, u8),
    mid: Option<(u8, u8, u8)>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> Result<DynamicImage, PilError> {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    let lut = colorize_lut(black, white, mid, blackpoint, midpoint, whitepoint);
    for y in 0..h {
        for x in 0..w {
            let g = gray.get_pixel(x, y)[0] as usize;
            out.put_pixel(x, y, crate::raster::Rgb([lut[0][g], lut[1][g], lut[2][g]]));
        }
    }
    // Colorize always outputs RGB (PIL behavior)
    Ok(DynamicImage::ImageRgb8(out))
}

/// Contain: resize to fit within (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does not truncate.
pub fn op_contain(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // Pillow's ImageOps.contain evaluates source and destination aspect
    // ratios before calling Image.resize, so either zero height raises
    // ZeroDivisionError instead of flowing into a deferred empty image.
    if ih == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio > dest_ratio {
        // Image is wider: adjust height
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is taller: adjust width
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    // Image.resize rejects rounded-zero dimensions.  The one valid exception
    // is an empty-width source whose contain height is unchanged: that
    // request equals the source and Pillow returns an empty copy.
    let empty_width_copy = iw == 0 && nw == 0 && nh == ih;
    if (nw == 0 || nh == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Pillow preserves a zero dimension when the source aspect-ratio math
    // rounds one axis to zero. `pil_resize` has an explicit empty-image path;
    // clamping here would turn a valid 0×N result into 1×N.
    let result = pil_resize(img, nw, nh, filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Cover: resize to cover (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does NOT crop.
pub fn op_cover(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // Pillow's ImageOps.cover evaluates source and destination aspect ratios
    // before calling Image.resize. Preserve its division errors instead of
    // allowing a deferred empty image or a zero target height to flow into
    // Rust's non-panicking floating-point arithmetic.
    if ih == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio < dest_ratio {
        // Image is taller: adjust height to cover
        if iw == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is wider: adjust width to cover
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    // Image.resize rejects rounded-zero dimensions. The one valid exception
    // is an empty-width source whose cover height is unchanged, for which the
    // resize request equals the source and Pillow returns an empty copy.
    let empty_width_copy = iw == 0 && nw == 0 && nh == ih;
    if (nw == 0 || nh == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    let result = pil_resize(img, nw, nh, filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Fit: resize to fit within (w, h) with bleed and centering, then crop.
/// PIL: applies bleed to source, computes crop box, resize with box parameter.
/// Uses PIL's exact box-based resize to match pixel-perfect output.
pub fn op_fit(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // ImageOps.fit performs these divisions before Image.resize sees the
    // boxed target. Keep the eager public checks mirrored here for callers
    // that execute a deferred pipeline through an explicit CPU backend.
    if ih == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    if h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    if iw == 0 {
        // A zero-width source can be copied only when the fit request keeps
        // the empty dimensions and does not alter the boxed source height.
        if w == 0 && h == ih && bleed == 0.0 {
            return Ok(img.clone());
        }
        if w == 0 || h < ih {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if w == 0 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Bleed pixels (PIL: bleed * image.size)
    let bleed_w = bleed * iw as f64;
    let bleed_h = bleed * ih as f64;
    // Live size
    // ImageOps.fit keeps the live dimensions as the exact positive result of
    // subtracting the bleed. Pillow does not clamp either axis to one pixel;
    // doing so changes the crop box for small images with non-zero bleed.
    let live_w = iw as f64 - 2.0 * bleed_w;
    let live_h = ih as f64 - 2.0 * bleed_h;
    let live_ratio = live_w / live_h;
    let output_ratio = w as f64 / h as f64;
    // Compute crop dimensions (PIL: floats, no rounding)
    let (crop_w, crop_h) = if (live_ratio - output_ratio).abs() < 1e-10 {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        // Live is wider: crop sides
        (output_ratio * live_h, live_h)
    } else {
        // Live is taller: crop top/bottom
        (live_w, live_w / output_ratio)
    };
    // Compute crop position with centering (PIL: floats, no rounding)
    let crop_left = bleed_w + (live_w - crop_w) * centering.0;
    let crop_top = bleed_h + (live_h - crop_h) * centering.1;
    // Use PIL's box-based resize (maps source box to target size)
    // Pillow's P resize path forces nearest-neighbour sampling even when
    // ImageOps.fit received another method. PA is different: its two raw
    // bands are passed through the requested filter and stay indexed.
    let resize_filter = if explicit_mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    };
    let result = pil_resize_boxed(
        img,
        w,
        h,
        crop_left,
        crop_top,
        crop_left + crop_w,
        crop_top + crop_h,
        resize_filter,
        explicit_mode,
    );
    Ok(preserve_mode(img, result))
}

/// Pad: resize to fit within (w, h), then pad with fill color.
/// PIL: contain then paste with centering, using round() for paste offset.
pub fn op_pad(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL: Image.new(image.mode, size, color) defaults to mode-appropriate fill
    // RGBA/LA modes: transparent fill (alpha=0). L/RGB: opaque black.
    let has_alpha = matches!(
        img.color(),
        crate::raster::ColorType::Rgba8 | crate::raster::ColorType::La8
    );
    let default_fill = if has_alpha {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, 255)
    };
    let fill = color.unwrap_or(default_fill);
    let (iw, ih) = (img.width(), img.height());
    // Step 1: contain (resize to fit within target)
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio > dest_ratio {
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    if iw != 0 && ih != 0 && (nw == 0 || nh == 0) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if iw == 0 && ih != 0 && nh != ih {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Pillow's ImagingCore resize keeps P/PA as indexed samples and ignores
    // the requested resampling kernel; using nearest here avoids interpolated
    // palette indices while preserving the raw mode through the pad path.
    let resize_filter = if explicit_mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    };
    // F-mode samples are four-byte IEEE words, not four independent byte
    // channels.  Image.resize already owns the exact f64 coefficient/f32
    // store path for this representation; reuse it for Pad's contain step
    // instead of routing through pil_resize's byte-oriented generic loop.
    let resized = if nw == 0 || nh == 0 {
        // Pillow's empty-width source can resize only when the contain pass
        // keeps its source height. Preserve the zero-width result instead of
        // routing it through the generic F/byte kernels or clamping to one.
        preserve_mode(img, pil_resize(img, nw, nh, resize_filter, explicit_mode))
    } else if explicit_mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        execute_resize(img, nw.max(1), nh.max(1), &resize_filter, explicit_mode)?
    } else {
        pil_resize(img, nw.max(1), nh.max(1), resize_filter, explicit_mode)
    };
    if nw == w && nh == h {
        return Ok(preserve_mode(img, resized));
    }

    if explicit_mode == Some("P") {
        let source = resized.to_luma8();
        let fill_index = color.map_or(0, |value| value.0);
        let mut padded =
            crate::raster::GrayImage::from_pixel(w, h, crate::raster::Luma([fill_index]));
        let (offset_x, offset_y) = if nw != w {
            (
                bankers_round((w as f64 - nw as f64) * centering.0.clamp(0.0, 1.0)) as u32,
                0,
            )
        } else {
            (
                0,
                bankers_round((h as f64 - nh as f64) * centering.1.clamp(0.0, 1.0)) as u32,
            )
        };
        for py in 0..nh.min(h) {
            for px in 0..nw.min(w) {
                let dx = offset_x + px;
                let dy = offset_y + py;
                if dx < w && dy < h {
                    padded.put_pixel(dx, dy, *source.get_pixel(px, py));
                }
            }
        }
        return Ok(DynamicImage::ImageLuma8(padded));
    }

    if explicit_mode == Some("PA") {
        let source = resized.to_luma_alpha8();
        let (fill_index, fill_alpha) = color.map_or((0, 0), |value| (value.0, value.3));
        let mut padded = crate::raster::GrayAlphaImage::from_pixel(
            w,
            h,
            crate::raster::LumaA([fill_index, fill_alpha]),
        );
        let (offset_x, offset_y) = if nw != w {
            (
                bankers_round((w as f64 - nw as f64) * centering.0.clamp(0.0, 1.0)) as u32,
                0,
            )
        } else {
            (
                0,
                bankers_round((h as f64 - nh as f64) * centering.1.clamp(0.0, 1.0)) as u32,
            )
        };
        for py in 0..nh.min(h) {
            for px in 0..nw.min(w) {
                let dx = offset_x + px;
                let dy = offset_y + py;
                if dx < w && dy < h {
                    padded.put_pixel(dx, dy, *source.get_pixel(px, py));
                }
            }
        }
        return Ok(DynamicImage::ImageLumaA8(padded));
    }

    // Step 2: pad to target size. Build the native RGBA output once and
    // operate on disjoint rows; repeated `put_pixel` calls otherwise add a
    // bounds-check and pixel-wrapper construction for every destination.
    // PIL: x = round((size[0] - resized.width) * max(0, min(centering[0], 1)))
    let cx = centering.0.clamp(0.0, 1.0);
    let cy = centering.1.clamp(0.0, 1.0);
    let src_rgba = resized.to_rgba8();
    // Pillow's ImageOps.pad uses Image.paste without a mask, so the resized
    // source replaces the destination pixels even when the source has alpha.
    let (offset_x, offset_y) = if nw != w {
        (bankers_round((w as f64 - nw as f64) * cx) as usize, 0usize)
    } else {
        (0usize, bankers_round((h as f64 - nh as f64) * cy) as usize)
    };
    let width = w as usize;
    #[cfg(feature = "parallel")]
    let height = h as usize;
    let copy_width = nw.min(w) as usize;
    let copy_height = nh.min(h) as usize;
    let output_stride = width.saturating_mul(4);
    let source_stride = nw as usize * 4;
    let fill_bytes = [fill.0, fill.1, fill.2, fill.3];
    let mut output = CheckedDims::new(w, h, 4)?.alloc_buffer();

    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut output,
            output_stride,
            height,
            |_row_start, _row_end, _y, row| {
                for pixel in row.chunks_exact_mut(4) {
                    pixel.copy_from_slice(&fill_bytes);
                }
            }
        );
    } else {
        for row in output.chunks_exact_mut(output_stride) {
            for pixel in row.chunks_exact_mut(4) {
                pixel.copy_from_slice(&fill_bytes);
            }
        }
    }
    #[cfg(not(feature = "parallel"))]
    for row in output.chunks_exact_mut(output_stride) {
        for pixel in row.chunks_exact_mut(4) {
            pixel.copy_from_slice(&fill_bytes);
        }
    }

    let source = src_rgba.as_raw();
    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut output,
            output_stride,
            height,
            |_row_start, _row_end, y, row| {
                let output_y = y as usize;
                if output_y < offset_y || output_y >= offset_y.saturating_add(copy_height) {
                    return;
                }
                let source_y = output_y - offset_y;
                let source_start = source_y * source_stride;
                let output_start = offset_x * 4;
                let byte_count = copy_width * 4;
                row[output_start..output_start + byte_count]
                    .copy_from_slice(&source[source_start..source_start + byte_count]);
            }
        );
    } else {
        for output_y in offset_y..offset_y.saturating_add(copy_height) {
            let source_y = output_y - offset_y;
            let source_start = source_y * source_stride;
            let output_start = output_y * output_stride + offset_x * 4;
            let byte_count = copy_width * 4;
            output[output_start..output_start + byte_count]
                .copy_from_slice(&source[source_start..source_start + byte_count]);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for output_y in offset_y..offset_y.saturating_add(copy_height) {
        let source_y = output_y - offset_y;
        let source_start = source_y * source_stride;
        let output_start = output_y * output_stride + offset_x * 4;
        let byte_count = copy_width * 4;
        output[output_start..output_start + byte_count]
            .copy_from_slice(&source[source_start..source_start + byte_count]);
    }

    let padded = crate::image_utils::raw_bytes_to_image(w, h, output, 4)?;
    Ok(preserve_mode(img, padded))
}

/// CropBorder: remove `border` pixels from all four sides.
pub fn op_crop_border(img: &DynamicImage, border: u32) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width(), img.height());
    // Pillow permits a border exactly half the image size and returns a
    // zero-sized image; only a strictly oversized border is invalid.
    if border > w / 2 {
        // Pillow delegates this invalid box to Image.crop(), whose public
        // contract reports the right edge being left of the left edge when
        // the width is the first invalid dimension.
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if border > h / 2 {
        // Keep the height-specific crop error observable for rectangular
        // inputs instead of collapsing it into the width diagnostic above.
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
    }

    // DynamicImage::crop_imm dispatches through generic pixel access, which
    // makes this byte-preserving operation several times slower than Pillow
    // for large rasters. Copy complete native rows instead. Typed and
    // floating-point images retain the image crate's existing behavior.
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return Ok(img.crop_imm(border, border, w - 2 * border, h - 2 * border)),
    };
    let output_width = w - 2 * border;
    let output_height = h - 2 * border;
    let output_dims = CheckedDims::new_allow_empty(output_width, output_height, channels as u8)?;
    let source_stride = (w as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("CropBorder source stride overflow".into()))?;
    let output_stride = (output_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("CropBorder output stride overflow".into()))?;
    // The crop overwrites every output byte; reserve without zero-filling the
    // destination so each pixel is written only once.
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let source = img.as_bytes();
    let x_offset = border as usize * channels;
    for y in 0..output_height as usize {
        let source_start = (border as usize + y)
            .checked_mul(source_stride)
            .and_then(|offset| offset.checked_add(x_offset))
            .ok_or_else(|| PilError::ValueError("CropBorder source offset overflow".into()))?;
        let source_end = source_start
            .checked_add(output_stride)
            .ok_or_else(|| PilError::ValueError("CropBorder source range overflow".into()))?;
        let source_row = source.get(source_start..source_end).ok_or_else(|| {
            PilError::InternalError("CropBorder source buffer shape mismatch".into())
        })?;
        output.extend_from_slice(source_row);
    }
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        output_width,
        output_height,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Scale: resize by a floating-point factor.
pub fn op_scale(
    img: &DynamicImage,
    factor: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // ImageOps.scale uses Python's round(image_dimension * factor), whose
    // ties-to-even behavior is observable at .5 dimensions (13 * 1.5 -> 20
    // while 11 * 1.5 -> 16).
    let new_w = bankers_round(img.width() as f64 * factor) as u32;
    let new_h = bankers_round(img.height() as f64 * factor) as u32;
    let result = pil_resize(img, new_w.max(1), new_h.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Expand: add a border of `border` pixels with `fill` color around the image.
/// The fill is a 4-tuple (r,g,b,a). Indexed `P`/`PA` inputs retain their raw
/// sample layout; the tuple's first byte is the `P` index and the first and
/// fourth bytes are the `PA` index/alpha pair.
pub fn op_expand(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width(), img.height());
    let new_w = w + 2 * border;
    let new_h = h + 2 * border;

    if mode == Some("P") {
        let source = img.to_luma8();
        let mut expanded =
            crate::raster::GrayImage::from_pixel(new_w, new_h, crate::raster::Luma([fill.0]));
        for py in 0..h {
            for px in 0..w {
                expanded.put_pixel(px + border, py + border, *source.get_pixel(px, py));
            }
        }
        return Ok(DynamicImage::ImageLuma8(expanded));
    }

    if mode == Some("PA") {
        let source = img.to_luma_alpha8();
        let mut expanded = crate::raster::GrayAlphaImage::from_pixel(
            new_w,
            new_h,
            crate::raster::LumaA([fill.0, fill.3]),
        );
        for py in 0..h {
            for px in 0..w {
                expanded.put_pixel(px + border, py + border, *source.get_pixel(px, py));
            }
        }
        return Ok(DynamicImage::ImageLumaA8(expanded));
    }

    let src_rgba = img.to_rgba8();
    let (sw, sh) = (src_rgba.width(), src_rgba.height());
    let dims = CheckedDims::new(new_w, new_h, 4)?;
    #[cfg(feature = "parallel")]
    let height = new_h as usize;
    let row_stride = dims.row_stride();
    let source_stride = sw as usize * 4;
    let offset_x = border as usize;
    let offset_y = border as usize;
    let copy_width = sw.min(new_w.saturating_sub(border)) as usize;
    let copy_height = sh.min(new_h.saturating_sub(border)) as usize;
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let source = src_rgba.as_raw();
    let mut output = dims.alloc_buffer();

    #[cfg(feature = "parallel")]
    if dims.total_pixels() >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut output,
            row_stride,
            height,
            |_row_start, _row_end, y, row| {
                expand_rgba_row(
                    row,
                    y as usize,
                    source,
                    source_stride,
                    offset_x,
                    offset_y,
                    copy_width,
                    copy_height,
                    fill,
                );
            }
        );
    } else {
        for (y, row) in output.chunks_exact_mut(row_stride).enumerate() {
            expand_rgba_row(
                row,
                y,
                source,
                source_stride,
                offset_x,
                offset_y,
                copy_width,
                copy_height,
                fill,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output.chunks_exact_mut(row_stride).enumerate() {
        expand_rgba_row(
            row,
            y,
            source,
            source_stride,
            offset_x,
            offset_y,
            copy_width,
            copy_height,
            fill,
        );
    }

    let expanded = crate::image_utils::raw_bytes_to_image(new_w, new_h, output, 4)?;
    Ok(preserve_mode(img, expanded))
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
