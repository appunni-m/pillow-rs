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
    Image(Box<Image>),
    /// A truthy non-image value was supplied, retaining its host type name so
    /// bindings can expose Pillow's attribute error without inspecting host
    /// objects in the core.
    Invalid(String),
}

mod bbox;
mod histogram;

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
}
