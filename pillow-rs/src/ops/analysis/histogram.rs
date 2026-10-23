//! Per-band histogram scans and Pillow-compatible special-mode dispatch.

use crate::error::PilError;
use crate::image::Image;

#[inline(always)]
fn mask_includes_pixel(mask: Option<&[u8]>, index: usize) -> bool {
    match mask {
        Some(mask) => mask[index] != 0,
        None => true,
    }
}

/// Match Pillow's legacy histogram dispatch for unsigned 16-bit modes.
///
/// `src/libImaging/Histo.c` dispatches these `IMAGING_TYPE_SPECIAL` images
/// through the byte histogram path. The histogram therefore scans the first
/// `width` bytes of each `width * 2` byte row, rather than converting the
/// logical `u16` samples to 8-bit luminance values.
fn histogram_l16_pillow(
    img: &crate::raster::DynamicImage,
    mode: &str,
    mask_raw: Option<&[u8]>,
) -> Vec<u32> {
    let samples = img.to_luma16();
    let width = img.width() as usize;
    let height = img.height() as usize;
    let mut hist = vec![0u32; 256];
    let big_endian = mode == "I;16B" || (mode != "I;16L" && cfg!(target_endian = "big"));
    for y in 0..height {
        for x in 0..width {
            if matches!(mask_raw, Some(mask) if mask[y * width + x] == 0) {
                continue;
            }
            let sample = samples.as_raw()[y * width + x / 2];
            let bytes = if big_endian {
                sample.to_be_bytes()
            } else {
                sample.to_le_bytes()
            };
            hist[bytes[x % 2] as usize] += 1;
        }
    }
    hist
}

/// Match Pillow's extrema-scaled histogram for unmasked signed integer data.
fn histogram_i_pillow(img: &crate::raster::DynamicImage) -> Vec<u32> {
    let values: Vec<i32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut hist = vec![0u32; 256];
    let Some((&minimum, &maximum)) = values.iter().min().zip(values.iter().max()) else {
        return hist;
    };
    if minimum == maximum {
        return hist;
    }
    let range = i64::from(maximum) - i64::from(minimum);
    for value in values {
        let scaled = (i64::from(value) - i64::from(minimum)) * 255 / range;
        hist[scaled.clamp(0, 255) as usize] += 1;
    }
    hist
}

/// Match Pillow's extrema-scaled histogram for unmasked float data.
fn histogram_f_pillow(img: &crate::raster::DynamicImage) -> Vec<u32> {
    let values: Vec<f32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut hist = vec![0u32; 256];
    let Some(&first) = values.first() else {
        return hist;
    };
    // Pillow's float extrema scan seeds both extrema from the first sample
    // and updates them only through ordered comparisons.  That preserves a
    // leading NaN as the public extrema, unlike Rust's NaN-ignoring min/max
    // helpers.
    let (mut minimum, mut maximum) = (first, first);
    for &value in &values[1..] {
        if value < minimum {
            minimum = value;
        }
        if value > maximum {
            maximum = value;
        }
    }
    if minimum == maximum {
        return hist;
    }
    let range = maximum - minimum;
    // Pillow computes this reciprocal once in the source float type. Keeping
    // the scale separate from the per-sample multiplication preserves its
    // observable rounding at the upper boundary: depending on the range, the
    // maximum sample may land in bin 254 or bin 255.
    let scale = 255.0 / range;
    for value in values {
        let scaled = ((value - minimum) * scale).floor();
        // The C histogram reducer converts a non-finite scaled value to the
        // first bin. This is observable for NaN samples and non-finite
        // extrema, so retain the source behavior instead of dropping it.
        let bin = if scaled.is_finite() {
            scaled.clamp(0.0, 255.0) as usize
        } else {
            0
        };
        hist[bin] += 1;
    }
    hist
}

fn histogram_special_mode(
    image: &crate::raster::DynamicImage,
    mode: &str,
    mask_raw: Option<&[u8]>,
) -> Result<Option<Vec<u32>>, PilError> {
    if matches!(mode, "I" | "F") {
        if mask_raw.is_some() {
            // Pillow's masked scalar histogram call uses the byte-image entry
            // point and rejects I/F images as the wrong mode.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if image.width() == 0 || image.height() == 0 {
            return Err(PilError::ValueError("min/max not given".into()));
        }
        return Ok(Some(if mode == "I" {
            histogram_i_pillow(image)
        } else {
            histogram_f_pillow(image)
        }));
    }
    if image.color() == crate::raster::ColorType::L16
        && matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
    {
        return Ok(Some(histogram_l16_pillow(image, mode, mask_raw)));
    }
    Ok(None)
}

fn histogram_band_count(image: &crate::raster::DynamicImage) -> usize {
    match image.color() {
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
        _ => 4,
    }
}

fn histogram_native_bytes(
    image: &crate::raster::DynamicImage,
    width: usize,
    masked: bool,
    mask_raw: Option<&[u8]>,
    histogram: &mut [u32],
) -> bool {
    match image {
        crate::raster::DynamicImage::ImageLumaA8(la) => {
            for (y, row) in la.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    histogram[pixel[0] as usize] += 1;
                    // Pillow 12.2 routes masked LA's second band through luma;
                    // the unmasked path retains the native alpha histogram.
                    let second = if masked { pixel[0] } else { pixel[1] };
                    histogram[256 + second as usize] += 1;
                }
            }
        }
        crate::raster::DynamicImage::ImageLuma8(luma) => {
            for (y, row) in luma.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if mask_includes_pixel(mask_raw, y * width + x) {
                        histogram[pixel[0] as usize] += 1;
                    }
                }
            }
        }
        crate::raster::DynamicImage::ImageRgb8(rgb) => {
            for (y, row) in rgb.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    histogram[pixel[0] as usize] += 1;
                    histogram[256 + pixel[1] as usize] += 1;
                    histogram[512 + pixel[2] as usize] += 1;
                }
            }
        }
        crate::raster::DynamicImage::ImageRgba8(rgba) => {
            for (y, row) in rgba.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    for band in 0..4 {
                        histogram[band * 256 + pixel[band] as usize] += 1;
                    }
                }
            }
        }
        _ => return false,
    }
    true
}

fn histogram_converted_bytes(
    image: &crate::raster::DynamicImage,
    width: usize,
    bands: usize,
    mask_raw: Option<&[u8]>,
    histogram: &mut [u32],
) {
    match image.color() {
        crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
            let la = image.to_luma_alpha8();
            for (y, row) in la.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    histogram[pixel[0] as usize] += 1;
                    histogram[256 + pixel[1] as usize] += 1;
                }
            }
        }
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
            let luma = image.to_luma8();
            for (y, row) in luma.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if mask_includes_pixel(mask_raw, y * width + x) {
                        histogram[pixel[0] as usize] += 1;
                    }
                }
            }
        }
        crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => {
            let rgb = image.to_rgb8();
            for (y, row) in rgb.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    histogram[pixel[0] as usize] += 1;
                    histogram[256 + pixel[1] as usize] += 1;
                    histogram[512 + pixel[2] as usize] += 1;
                }
            }
        }
        _ => {
            let rgba = image.to_rgba8();
            for (y, row) in rgba.rows().enumerate() {
                for (x, pixel) in row.enumerate() {
                    if !mask_includes_pixel(mask_raw, y * width + x) {
                        continue;
                    }
                    for band in 0..bands {
                        histogram[band * 256 + pixel[band] as usize] += 1;
                    }
                }
            }
        }
    }
}

fn histogram_materialized(
    image: &crate::raster::DynamicImage,
    mode: &str,
    masked: bool,
    mask_raw: Option<&[u8]>,
) -> Result<Vec<u32>, PilError> {
    if let Some(histogram) = histogram_special_mode(image, mode, mask_raw)? {
        return Ok(histogram);
    }
    let bands = histogram_band_count(image);
    let mut histogram = vec![0u32; 256 * bands];
    let width = image.width() as usize;
    if !histogram_native_bytes(image, width, masked, mask_raw, &mut histogram) {
        histogram_converted_bytes(image, width, bands, mask_raw, &mut histogram);
    }
    Ok(histogram)
}

impl Image {
    /// Computes a per-band 256-bin histogram.
    ///
    /// The returned vector is concatenated by band: all 256 bins for band 0,
    /// then all 256 bins for band 1, and so on.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn histogram(&self) -> Result<Vec<u32>, PilError> {
        self.histogram_with_mask(None)
    }

    /// Computes a per-band histogram restricted to pixels where an optional
    /// mask is non-zero, matching Pillow's masked-histogram semantics.
    pub fn histogram_with_mask(&self, mask: Option<&Image>) -> Result<Vec<u32>, PilError> {
        let image = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&image);
        let mask_image = if let Some(mask) = mask {
            let mask_image = mask.materialized_shared()?;
            if (mask_image.width(), mask_image.height()) != (image.width(), image.height()) {
                return Err(PilError::ValueError("images do not match".into()));
            }
            let mode = mask.mode()?;
            if mode != "1" && mode != "L" {
                return Err(PilError::ValueError("bad transparency mask".into()));
            }
            Some(mask_image)
        } else {
            None
        };
        // Borrow native L8 mask bytes directly; only converted layouts need an
        // owned luma buffer for the scan.
        let owned_mask_luma =
            mask_image
                .as_ref()
                .and_then(|mask_image| match mask_image.as_ref() {
                    crate::raster::DynamicImage::ImageLuma8(_) => None,
                    _ => Some(mask_image.to_luma8().into_raw()),
                });
        let mask_raw: Option<&[u8]> = match (&mask_image, &owned_mask_luma) {
            (Some(mask_image), None) => match mask_image.as_ref() {
                crate::raster::DynamicImage::ImageLuma8(luma) => Some(luma.as_raw().as_slice()),
                _ => unreachable!("native mask must use L8 storage"),
            },
            (_, Some(raw)) => Some(raw.as_slice()),
            _ => None,
        };
        histogram_materialized(&image, &mode, mask_image.is_some(), mask_raw)
    }
}
