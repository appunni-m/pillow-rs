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
        let img = self.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());

        if img_w == 0 || img_h == 0 {
            return Ok(None);
        }

        let mode = self.mode()?;
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
            match self.scalar_samples(&mode)? {
                crate::image::ScalarImageSamples::Integer(values) => {
                    for (index, value) in values.into_iter().enumerate() {
                        update(index, value != 0);
                    }
                }
                crate::image::ScalarImageSamples::Float(values) => {
                    for (index, value) in values.into_iter().enumerate() {
                        update(index, value != 0.0);
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

        let has_alpha = matches!(
            img.color(),
            crate::raster::ColorType::La8
                | crate::raster::ColorType::La16
                | crate::raster::ColorType::Rgba8
                | crate::raster::ColorType::Rgba16
                | crate::raster::ColorType::Rgba32F
        );
        let rgba = img.to_rgba8();
        for y in 0..img_h {
            for x in 0..img_w {
                let px = rgba.get_pixel(x, y);
                // Pillow GetBBox.c: 3-band modes zero the alpha byte of the mask
                // (RGB only), 4-band alpha modes (LA/RGBA/PA/RGBa) use the alpha byte
                // when alpha_only is set, and every other 32-bit mode keeps the full
                // 0xffffffff mask — so alpha_only=false still counts a pixel whose
                // alpha is nonzero even when all RGB channels are zero.
                let is_nonzero = if alpha_only && has_alpha {
                    px[3] > 0
                } else if has_alpha {
                    px[0] > 0 || px[1] > 0 || px[2] > 0 || px[3] > 0
                } else {
                    px[0] > 0 || px[1] > 0 || px[2] > 0
                };
                if is_nonzero {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
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
        let img = self.materialize()?;

        let bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };

        let mut extrema = vec![(255u8, 0u8); bands];
        // Use the appropriate pixel reader based on color type
        match img.color() {
            crate::raster::ColorType::L8 => {
                let luma = img.to_luma8();
                for px in luma.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                }
            }
            crate::raster::ColorType::La8 => {
                let la = img.to_luma_alpha8();
                for px in la.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                    extrema[1].0 = extrema[1].0.min(px[1]);
                    extrema[1].1 = extrema[1].1.max(px[1]);
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
        let img = self.materialize()?;
        let mask_luma = if let Some(mask) = mask {
            let mask_img = mask.materialize()?;
            if (mask_img.width(), mask_img.height()) != (img.width(), img.height()) {
                return Err(PilError::ValueError("images do not match".into()));
            }
            let mode = mask.mode()?;
            if mode != "1" && mode != "L" {
                return Err(PilError::ValueError("bad transparency mask".into()));
            }
            Some(mask_img.to_luma8())
        } else {
            None
        };
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let mut hist = vec![0u32; 256 * n_bands];
        let mask_px = mask_luma.as_ref();
        let masked_pixel = |x: u32, y: u32| -> bool {
            match mask_px {
                Some(mask_img) => {
                    let px = mask_img.get_pixel(x, y);
                    px[0] != 0
                }
                None => true,
            }
        };
        // Use mode-aware pixel reading to avoid to_rgba8() remapping channels
        match img.color() {
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
                let la = img.to_luma_alpha8();
                for (y, row) in la.rows().enumerate() {
                    for (x, px) in row.enumerate() {
                        if !masked_pixel(x as u32, y as u32) {
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
                        if !masked_pixel(x as u32, y as u32) {
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
                        if !masked_pixel(x as u32, y as u32) {
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
                        if !masked_pixel(x as u32, y as u32) {
                            continue;
                        }
                        for b in 0..n_bands {
                            hist[b * 256 + px[b] as usize] += 1;
                        }
                    }
                }
            }
        }
        Ok(hist)
    }
}
