//! Pillow-compatible image analysis operations.
//!
//! These methods materialize lazy pipelines before scanning pixels. Results are
//! returned as Rust primitives instead of Python tuples or lists.

use crate::error::PilError;
use crate::image::Image;

/// Host-neutral optional mask input for image analysis operations.
#[derive(Debug, Clone)]
pub enum ImageAnalysisMask {
    /// No mask was supplied.
    None,
    /// A mask image extracted by a binding.
    Image(Image),
    /// A truthy non-image value was supplied, retaining its host type name so
    /// bindings can expose Pillow's attribute error without inspecting host
    /// objects in the core.
    Invalid(String),
}

/// Validate a Pillow transparency/statistics mask without touching host types.
pub fn validate_transparency_mask(image: &Image, mask: &Image) -> Result<(), PilError> {
    let mode = mask.mode()?;
    let size = mask.size()?;
    if !matches!(mode.as_str(), "1" | "L") {
        return Err(PilError::ValueError("bad transparency mask".into()));
    }
    // Pillow's ImageOps mask path distinguishes an unsupported mode from a
    // same-mode size mismatch: the former is "bad transparency mask", while
    // the latter is "images do not match".
    if size != image.size()? {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(())
}

impl Image {
    /// Computes a histogram after validating a host-neutral optional mask.
    pub fn histogram_with_input(&self, mask: ImageAnalysisMask) -> Result<Vec<u32>, PilError> {
        match mask {
            ImageAnalysisMask::None => self.histogram(),
            ImageAnalysisMask::Image(mask) => self.histogram_with_mask(Some(&mask)),
            ImageAnalysisMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
                "'{type_name}' object has no attribute 'load'"
            ))),
        }
    }

    /// Computes entropy after validating a host-neutral optional mask.
    pub fn entropy_with_input(&self, mask: ImageAnalysisMask) -> Result<f64, PilError> {
        match mask {
            ImageAnalysisMask::None => self.entropy(),
            ImageAnalysisMask::Image(mask) => self.entropy_with_mask(Some(&mask)),
            ImageAnalysisMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
                "'{type_name}' object has no attribute 'load'"
            ))),
        }
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

impl Image {
    /// Returns the bounding box of non-zero image content.
    ///
    /// The result is `(left, top, right, bottom)` with `right` and `bottom`
    /// exclusive, matching Pillow `getbbox`. `None` means every inspected pixel
    /// is zero. When `alpha_only` is true and the image has alpha, only alpha is
    /// inspected.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getbbox(&self, alpha_only: bool) -> Result<Option<(u32, u32, u32, u32)>, PilError> {
        let img = self.materialized_shared()?;
        let (img_w, img_h) = (img.width(), img.height());

        if img_w == 0 || img_h == 0 {
            return Ok(None);
        }

        let mode = self.mode_from_materialized(&img);
        if mode == "I" || mode == "F" {
            let mut left = img_w;
            let mut top = img_h;
            let mut right = 0u32;
            let mut bottom = 0u32;
            let mut update = |index: usize, is_nonzero: bool| {
                if is_nonzero {
                    let x = (index % img_w as usize) as u32;
                    let y = (index / img_w as usize) as u32;
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            };
            // Materialization has already validated the four-byte scalar
            // storage. Scan the retained frame directly so this read-only
            // reduction does not allocate one decoded value per pixel.
            if mode == "I" {
                for (index, sample) in img.as_bytes().chunks_exact(4).enumerate() {
                    let value = i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
                    update(index, value != 0);
                }
            } else {
                for (index, sample) in img.as_bytes().chunks_exact(4).enumerate() {
                    let value = f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
                    update(index, value != 0.0);
                }
            }
            return if left > right {
                Ok(None)
            } else {
                Ok(Some((left, top, right + 1, bottom + 1)))
            };
        }

        if img.color() == crate::raster::ColorType::L16
            && matches!(mode.as_str(), "I;16" | "I;16L" | "I;16B" | "I;16N")
        {
            // Pillow's ImagingCore getbbox dispatches I;16* through its
            // byte-oriented image8 scan. Preserve that public behavior here:
            // each row contributes its first `width` stored bytes, rather than
            // one predicate per decoded 16-bit sample.
            let width = img_w as usize;
            let row_stride = width * 2;
            let raw = img.as_bytes();
            let mut left = img_w;
            let mut top = img_h;
            let mut right = 0u32;
            let mut bottom = 0u32;
            for y in 0..img_h as usize {
                let row = &raw[y * row_stride..(y + 1) * row_stride];
                for x in 0..width {
                    if row[x] != 0 {
                        let x = x as u32;
                        let y = y as u32;
                        left = left.min(x);
                        top = top.min(y);
                        right = right.max(x);
                        bottom = bottom.max(y);
                    }
                }
            }
            return if left > right {
                Ok(None)
            } else {
                Ok(Some((left, top, right + 1, bottom + 1)))
            };
        }

        let mut left = img_w;
        let mut top = img_h;
        let mut right = 0u32;
        let mut bottom = 0u32;

        let mut include = |index: usize, is_nonzero: bool| {
            if is_nonzero {
                let x = (index % img_w as usize) as u32;
                let y = (index / img_w as usize) as u32;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
            }
        };

        // Keep native 8-bit terminal reads on their existing shared storage.
        // The conversion fallback remains for typed and unusual layouts whose
        // public mode semantics still depend on the canonical RGBA adapter.
        match img.as_ref() {
            crate::raster::DynamicImage::ImageLuma8(image) => {
                for (index, pixel) in image.pixels().enumerate() {
                    include(index, pixel[0] != 0);
                }
            }
            crate::raster::DynamicImage::ImageLumaA8(image) => {
                for (index, pixel) in image.pixels().enumerate() {
                    include(
                        index,
                        if alpha_only {
                            pixel[1] != 0
                        } else {
                            pixel[0] != 0 || pixel[1] != 0
                        },
                    );
                }
            }
            crate::raster::DynamicImage::ImageRgb8(image) => {
                for (index, pixel) in image.pixels().enumerate() {
                    include(index, pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0);
                }
            }
            crate::raster::DynamicImage::ImageRgba8(image) => {
                for (index, pixel) in image.pixels().enumerate() {
                    include(
                        index,
                        if alpha_only {
                            pixel[3] != 0
                        } else {
                            pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 || pixel[3] != 0
                        },
                    );
                }
            }
            _ => {
                let has_alpha = matches!(
                    img.color(),
                    crate::raster::ColorType::La8
                        | crate::raster::ColorType::La16
                        | crate::raster::ColorType::Rgba8
                        | crate::raster::ColorType::Rgba16
                        | crate::raster::ColorType::Rgba32F
                );
                let rgba = img.to_rgba8();
                for (index, pixel) in rgba.pixels().enumerate() {
                    // Pillow GetBBox.c uses alpha only for alpha-bearing modes;
                    // RGB keeps all three visible channels in its mask.
                    let is_nonzero = if alpha_only && has_alpha {
                        pixel[3] > 0
                    } else if has_alpha {
                        pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0 || pixel[3] > 0
                    } else {
                        pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0
                    };
                    include(index, is_nonzero);
                }
            }
        }

        // left > right is true exactly when no inspected pixel was nonzero;
        // top > bottom is correlated with the same condition, so testing it
        // separately adds an unreachable branch arm (Pillow returns None for
        // the same empty case).
        if left > right {
            Ok(None)
        } else {
            Ok(Some((left, top, right + 1, bottom + 1)))
        }
    }

    /// Returns minimum and maximum byte values per band.
    ///
    /// Bands are returned in decoded image order. Each pair is `(min, max)`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getextrema(&self) -> Result<Vec<(u8, u8)>, PilError> {
        let img = self.materialized_shared()?;

        let bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };

        let mut extrema = vec![(255u8, 0u8); bands];
        // Use the appropriate pixel reader based on color type
        match img.as_ref() {
            crate::raster::DynamicImage::ImageLuma8(luma) => {
                for px in luma.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                }
            }
            crate::raster::DynamicImage::ImageLumaA8(la) => {
                for px in la.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                    extrema[1].0 = extrema[1].0.min(px[1]);
                    extrema[1].1 = extrema[1].1.max(px[1]);
                }
            }
            crate::raster::DynamicImage::ImageRgb8(rgb) => {
                for px in rgb.pixels() {
                    for b in 0..3 {
                        extrema[b].0 = extrema[b].0.min(px[b]);
                        extrema[b].1 = extrema[b].1.max(px[b]);
                    }
                }
            }
            crate::raster::DynamicImage::ImageRgba8(rgba) => {
                for px in rgba.pixels() {
                    for b in 0..4 {
                        extrema[b].0 = extrema[b].0.min(px[b]);
                        extrema[b].1 = extrema[b].1.max(px[b]);
                    }
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    for b in 0..bands.min(4) {
                        extrema[b].0 = extrema[b].0.min(px[b]);
                        extrema[b].1 = extrema[b].1.max(px[b]);
                    }
                }
            }
        }
        Ok(extrema)
    }

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
        let img = self.materialized_shared()?;
        let mode = self.mode_from_materialized(&img);
        let mask_image = if let Some(mask) = mask {
            let mask_image = mask.materialized_shared()?;
            if (mask_image.width(), mask_image.height()) != (img.width(), img.height()) {
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
        // Native L8 masks already expose the exact bytes needed by Pillow's
        // non-zero mask test. Borrow them directly; only converted layouts
        // need an owned luma buffer.
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
        if matches!(mode.as_str(), "I" | "F") {
            if mask_raw.is_some() {
                // Pillow's masked scalar histogram call uses the byte-image
                // entry point and rejects I/F images as the wrong mode.
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            if img.width() == 0 || img.height() == 0 {
                return Err(PilError::ValueError("min/max not given".into()));
            }
            return Ok(if mode == "I" {
                histogram_i_pillow(&img)
            } else {
                histogram_f_pillow(&img)
            });
        }
        if img.color() == crate::raster::ColorType::L16
            && matches!(mode.as_str(), "I;16" | "I;16L" | "I;16B" | "I;16N")
        {
            return Ok(histogram_l16_pillow(&img, &mode, mask_raw));
        }
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hist = vec![0u32; 256 * n_bands];
        let width = img.width() as usize;
        let masked_pixel = |index: usize| -> bool {
            match mask_raw {
                Some(mask) => mask[index] != 0,
                None => true,
            }
        };
        // Use native pixel storage for ordinary byte layouts. This keeps the
        // terminal reduction result-sized: no full-frame conversion is needed
        // merely to count channels already present in the shared image.
        match img.as_ref() {
            crate::raster::DynamicImage::ImageLumaA8(la) => {
                for (y, row) in la.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked_pixel(y * width + x) {
                            continue;
                        }
                        hist[px[0] as usize] += 1;
                        // Pillow 12.2's masked LA histogram routes the second
                        // band through the luma byte as well. Preserve that
                        // source-compatible behavior only for the masked
                        // path; the unmasked path retains the native alpha
                        // histogram.
                        let second = if mask_image.is_some() { px[0] } else { px[1] };
                        hist[256 + second as usize] += 1;
                    }
                }
            }
            crate::raster::DynamicImage::ImageLuma8(luma) => {
                for (y, row) in luma.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked_pixel(y * width + x) {
                            continue;
                        }
                        hist[px[0] as usize] += 1;
                    }
                }
            }
            crate::raster::DynamicImage::ImageRgb8(rgb) => {
                for (y, row) in rgb.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked_pixel(y * width + x) {
                            continue;
                        }
                        hist[px[0] as usize] += 1;
                        hist[256 + px[1] as usize] += 1;
                        hist[512 + px[2] as usize] += 1;
                    }
                }
            }
            crate::raster::DynamicImage::ImageRgba8(rgba) => {
                for (y, row) in rgba.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked_pixel(y * width + x) {
                            continue;
                        }
                        for b in 0..4 {
                            hist[b * 256 + px[b] as usize] += 1;
                        }
                    }
                }
            }
            _ => match img.color() {
                crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
                    let la = img.to_luma_alpha8();
                    for (y, row) in la.rows().enumerate() {
                        for (x, px) in row.enumerate() {
                            if !masked_pixel(y * width + x) {
                                continue;
                            }
                            hist[px[0] as usize] += 1;
                            hist[256 + px[1] as usize] += 1;
                        }
                    }
                }
                crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
                    let luma = img.to_luma8();
                    for (y, row) in luma.rows().enumerate() {
                        for (x, px) in row.enumerate() {
                            if !masked_pixel(y * width + x) {
                                continue;
                            }
                            hist[px[0] as usize] += 1;
                        }
                    }
                }
                crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => {
                    let rgb = img.to_rgb8();
                    for (y, row) in rgb.rows().enumerate() {
                        for (x, px) in row.enumerate() {
                            if !masked_pixel(y * width + x) {
                                continue;
                            }
                            hist[px[0] as usize] += 1;
                            hist[256 + px[1] as usize] += 1;
                            hist[512 + px[2] as usize] += 1;
                        }
                    }
                }
                _ => {
                    let rgba = img.to_rgba8();
                    for (y, row) in rgba.rows().enumerate() {
                        for (x, px) in row.enumerate() {
                            if !masked_pixel(y * width + x) {
                                continue;
                            }
                            for b in 0..n_bands {
                                hist[b * 256 + px[b] as usize] += 1;
                            }
                        }
                    }
                }
            },
        }
        Ok(hist)
    }
}
