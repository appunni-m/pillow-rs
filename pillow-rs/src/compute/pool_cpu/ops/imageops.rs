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

/// Build the per-channel LUT used by Pillow's equalize operation.
///
/// Histogram construction is scalar reduction/control work. The returned
/// native-band table is intentionally separate from applying it so SIMD can
/// keep the complete pixel pass in its vector LUT data plane.
pub(crate) fn equalize_lut(img: &DynamicImage, channels: usize) -> Option<Vec<u8>> {
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
    // Keep the admitted L/RGB layouts in fixed-band loops. A runtime channel
    // index makes this scalar reduction dominate identity equalize workloads.
    if channels == 1 {
        for &value in raw {
            histograms[0][usize::from(value)] += 1;
        }
    } else {
        for pixel in raw.chunks_exact(3) {
            histograms[0][usize::from(pixel[0])] += 1;
            histograms[1][usize::from(pixel[1])] += 1;
            histograms[2][usize::from(pixel[2])] += 1;
        }
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
fn invert_bytes_serial(bytes: &mut [u8]) {
    for value in bytes {
        *value = u8::MAX - *value;
    }
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

#[inline]
fn apply_equalize_row(source: &[u8], row: &mut [u8], luts: &[[u8; 256]; 3], apply: &[bool; 3]) {
    for (output, input) in row.chunks_exact_mut(3).zip(source.chunks_exact(3)) {
        for channel in 0..3 {
            if apply[channel] {
                output[channel] = luts[channel][input[channel] as usize];
            }
        }
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
    // PIL 12 equalize: build LUT from non-zero histogram bins
    // step = (sum(non_zero_bins) - last_bin_count) / 255
    // lut[i] = floor(accumulator / step) where accumulator tracks step/2 + cumulative hist
    let rgb = img.to_rgb8();
    // Start from a copy of the input: uniform or single-value histograms
    // keep the source pixels unchanged (PIL's equalize identity path).
    let mut out = rgb.clone();
    let mut luts = [[0u8; 256]; 3];
    let mut apply = [false; 3];
    let mut histograms = [[0u32; 256]; 3];
    for px in rgb.pixels() {
        for ch in 0..3 {
            histograms[ch][px[ch] as usize] += 1;
        }
    }
    for ch in 0..3 {
        let mut nonzero_bins = 0usize;
        let mut last_nonzero_count = 0u32;
        let mut total = 0u32;
        for &count in &histograms[ch] {
            total += count;
            if count > 0 {
                nonzero_bins += 1;
                last_nonzero_count = count;
            }
        }
        if nonzero_bins <= 1 {
            // Identity LUT
            continue; // out already has original pixels from the RgbImage
        }
        let step = (total - last_nonzero_count) / 255;
        if step == 0 {
            continue; // Identity LUT
        }
        let mut n = step / 2;
        for i in 0..256 {
            luts[ch][i] = (n / step).min(255) as u8;
            n += histograms[ch][i];
        }
        apply[ch] = true;
    }
    let (width, height) = rgb.dimensions();
    let row_stride = width as usize * 3;
    let source = rgb.as_raw();
    #[cfg(feature = "parallel")]
    if (width as usize).saturating_mul(height as usize) >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            out.as_mut(),
            row_stride,
            height as usize,
            |row_start, _row_end, _y, row| {
                apply_equalize_row(
                    &source[row_start..row_start + row_stride],
                    row,
                    &luts,
                    &apply,
                );
            }
        );
    } else {
        for y in 0..height as usize {
            let row_start = y * row_stride;
            apply_equalize_row(
                &source[row_start..row_start + row_stride],
                &mut out.as_mut()[row_start..row_start + row_stride],
                &luts,
                &apply,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for y in 0..height as usize {
        let row_start = y * row_stride;
        apply_equalize_row(
            &source[row_start..row_start + row_stride],
            &mut out.as_mut()[row_start..row_start + row_stride],
            &luts,
            &apply,
        );
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

/// Invert: subtract each pixel value from 255 (all channels, matching PIL's point()).
pub fn op_invert(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let raw = img.as_bytes();
    let mut out = raw.to_vec();
    #[cfg(feature = "parallel")]
    let stride = w as usize * channels;
    #[cfg(feature = "parallel")]
    if (w as usize).saturating_mul(h as usize) >= POINT_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut out,
            stride,
            h as usize,
            |_row_start, _row_end, _y, row| {
                invert_bytes_serial(row);
            }
        );
    } else {
        invert_bytes_serial(&mut out);
    }
    #[cfg(not(feature = "parallel"))]
    invert_bytes_serial(&mut out);
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
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio < dest_ratio {
        // Image is taller: adjust height to cover
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is wider: adjust width to cover
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    let result = pil_resize(img, nw.max(1), nh.max(1), filter, explicit_mode);
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
    // Bleed pixels (PIL: bleed * image.size)
    let bleed_w = bleed * iw as f64;
    let bleed_h = bleed * ih as f64;
    // Live size
    let live_w = (iw as f64 - 2.0 * bleed_w).max(1.0);
    let live_h = (ih as f64 - 2.0 * bleed_h).max(1.0);
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
        w.max(1),
        h.max(1),
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
    let b = border;
    let (w, h) = (img.width(), img.height());
    // Pillow permits a border exactly half the image size and returns a
    // zero-sized image; only a strictly oversized border is invalid.
    if 2 * b > w {
        // Pillow delegates this invalid box to Image.crop(), whose public
        // contract reports the right edge being left of the left edge when
        // the width is the first invalid dimension.
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if 2 * b > h {
        // Keep the height-specific crop error observable for rectangular
        // inputs instead of collapsing it into the width diagnostic above.
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
    }
    Ok(img.crop_imm(b, b, w - 2 * b, h - 2 * b))
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
