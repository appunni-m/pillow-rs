//! Parameterized Pillow image filters.
//!
//! Most methods return lazy pipeline operations. Filters that need multiple
//! passes or mode-specific CPU behavior may materialize immediately.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::DynamicImage;

/// Find the mode (most common value) and its count from a histogram.
/// Uses PIL's strict `>` tie-breaking (lower value wins on tie).
/// Starts with pixel 0 as initial mode, scans 1..255.
fn find_mode_with_count(hist: &[u32; 256]) -> (u8, u32) {
    let mut mode = 0u8;
    let mut max_count = hist[0];
    for (v, &count) in hist.iter().enumerate().skip(1) {
        if count > max_count {
            max_count = count;
            mode = v as u8;
        }
    }
    (mode, max_count)
}

/// Formats the Pillow-compatible representation of a Color3DLUT filter.
pub fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    let target = target_mode
        .map(|mode| format!(" target_mode={mode}"))
        .unwrap_or_default();
    format!(
        "<Color3DLUT from {table_type} size={}x{}x{} channels={channels}{target}>",
        size.0, size.1, size.2
    )
}

impl Image {
    /// Applies Gaussian blur with the given radius.
    ///
    /// Larger radius produces more blur.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn gaussian_blur(&self, radius: f32) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::GaussianBlur { sigma: radius },
        ))
    }

    /// Applies box blur with a uniform kernel radius.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn box_blur(&self, radius: f32) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::BoxBlur {
                radius: radius as u32,
            },
        ))
    }

    /// PIL-compatible clip8: clamp to [0, 255].
    /// Matches PIL's clip8(): `return ss <= 0.0 ? 0 : ss >= 255.0 ? 255 : (UINT8)ss`
    fn pil_clip8(v: i32) -> u8 {
        if v >= 255 {
            255
        } else if v <= 0 {
            0
        } else {
            v as u8
        }
    }

    /// Applies Pillow-style unsharp masking.
    ///
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    /// Uses PIL-style GaussianBlur for the blurred version.
    /// Handles any number of channels (L=1, LA=2, RGB=3, RGBA=4).
    /// Uses PIL's exact integer arithmetic: `clip8(original + diff * percent / 100)`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, blur execution, allocation
    /// checks, or raw image reconstruction fails.
    pub fn unsharp_mask(
        &self,
        radius: f32,
        percent: i32,
        threshold: u8,
    ) -> Result<Image, PilError> {
        let img = self.materialize()?;
        // Use PIL-style GaussianBlur via the pipeline (sigma→box radius conversion)
        let blurred = Image::push_op(self, PipelineOp::GaussianBlur { sigma: radius });
        let blurred = blurred.materialize()?;

        let (w, h) = (img.width(), img.height());
        let channels = img.color().channel_count() as usize;

        let raw = img.as_bytes();
        let blur_raw = blurred.as_bytes();
        let mut out = CheckedDims::new(w, h, channels as u8)?.alloc_buffer();

        for y in 0..h {
            for x in 0..w {
                let base = (y * w + x) as usize * channels;
                for c in 0..channels {
                    let p = raw[base + c] as i32;
                    let b = blur_raw[base + c] as i32;
                    let diff = p - b;
                    // PIL uses integer arithmetic: diff * percent / 100 (truncating)
                    out[base + c] = if diff.unsigned_abs() > threshold as u32 {
                        Self::pil_clip8(p + diff * percent / 100)
                    } else {
                        p as u8
                    };
                }
            }
        }

        let result = crate::image::raw_bytes_to_image(w, h, out, channels)?;
        Ok(Image::from_dynamic(result, None))
    }

    /// Applies a maximum filter over an odd neighborhood.
    ///
    /// `size` is rounded up to an odd value of at least `3`.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MaxFilter { size }))
    }

    /// Applies a minimum filter over an odd neighborhood.
    ///
    /// `size` is rounded up to an odd value of at least `3`.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MinFilter { size }))
    }

    /// Applies a median filter over an odd neighborhood.
    ///
    /// `size` is rounded up to an odd value of at least `3`.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MedianFilter { size }))
    }

    /// Applies a mode filter over an odd neighborhood.
    ///
    /// Each pixel becomes the most common value in its neighborhood when that
    /// value occurs more than twice; otherwise the original pixel is preserved.
    ///
    /// PIL C behavior:
    ///   - Single-band only at C level; multi-band processed per-channel
    ///   - Strict `>` tie-breaking (lower value wins)
    ///   - If max count ≤ 2, original pixel is preserved unchanged
    ///   - Pixels outside image boundary are SKIPPED (not clamped/replicated)
    ///   - Supports any channel count (1=L, 2=LA, 3=RGB, 4=RGBA)
    ///   - For P-mode (palette): operates on palette indices, preserves palette
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization, allocation checks, or raw image
    /// reconstruction fails.
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3

        // For palette images: extract palette before materialize
        let palette = self.palette();
        let palette_alpha = self.palette_alpha();
        let explicit = self.explicit_mode().map(|s| s.to_string());

        let img = self.materialize()?;
        let half = (size / 2) as i32;

        let (w_u32, h_u32) = (img.width(), img.height());
        let w = w_u32 as i32;
        let h = h_u32 as i32;
        let channels = img.color().channel_count() as usize;
        let raw = img.as_bytes();

        let mut out = vec![0u8; (w_u32 * h_u32) as usize * channels];

        for y in 0..h {
            for x in 0..w {
                // Per-channel histograms
                let mut hists: Vec<[u32; 256]> = vec![[0u32; 256]; channels];
                for dy in -half..=half {
                    let sy = y + dy;
                    if sy < 0 || sy >= h {
                        continue; // PIL skips out-of-bounds rows
                    }
                    for dx in -half..=half {
                        let sx = x + dx;
                        if sx < 0 || sx >= w {
                            continue; // PIL skips out-of-bounds columns
                        }
                        let base = ((sy * w + sx) as usize) * channels;
                        for c in 0..channels {
                            hists[c][raw[base + c] as usize] += 1;
                        }
                    }
                }
                let out_base = ((y * w + x) as usize) * channels;
                for c in 0..channels {
                    let orig_val = raw[((y * w + x) as usize) * channels + c];
                    let (mode, max_count) = find_mode_with_count(&hists[c]);
                    out[out_base + c] = if max_count > 2 { mode } else { orig_val };
                }
            }
        }
        let result = crate::image::raw_bytes_to_image(w_u32, h_u32, out, channels)?;
        // Preserve palette for P-mode images
        if let Some(pal) = palette {
            let indices = match &result {
                DynamicImage::ImageLuma8(gray) => gray.clone(),
                _ => {
                    return Err(PilError::ValueError(
                        "mode_filter: unexpected output for palette image".into(),
                    ));
                }
            };
            return Ok(Image::Paletted(crate::image::PalettedData {
                indices,
                palette: pal,
                palette_alpha: palette_alpha.unwrap_or_default(),
                source_format: None,
                info: None,
                materialized: crate::image::materialization_cache(),
            }));
        }
        // Preserve explicit mode (e.g. "1", "P" via explicit_mode)
        if explicit.is_some() {
            return Ok(Image::from_dynamic(result, explicit));
        }
        Ok(Image::from_dynamic(result, None))
    }

    /// Applies a rank filter over an odd neighborhood.
    ///
    /// Each pixel becomes the `rank`-th value after sorting the neighborhood.
    /// `size` is rounded up to an odd value of at least `3`.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::RankFilter { size, rank }))
    }

    /// Applies a 3D color lookup table with trilinear interpolation.
    ///
    /// `size` is the LUT grid size, `table` contains the LUT values, and
    /// `channels` is the number of output channels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; LUT validation is handled during pipeline
    /// execution.
    pub fn color3dlut(
        &self,
        size: (u32, u32, u32),
        table: Vec<f64>,
        channels: u32,
        target_mode: Option<&str>,
    ) -> Result<Image, PilError> {
        if channels != 3 && channels != 4 {
            return Err(PilError::ValueError(
                "Only 3 or 4 output channels are supported".into(),
            ));
        }
        if !(2..=65).contains(&size.0) || !(2..=65).contains(&size.1) || !(2..=65).contains(&size.2)
        {
            return Err(PilError::ValueError(
                "Table size in any dimension should be from 2 to 65".into(),
            ));
        }
        let expected_len = size.0 as usize * size.1 as usize * size.2 as usize * channels as usize;
        if table.len() != expected_len {
            return Err(PilError::ValueError(
                "The table should have table_channels * size1D * size2D * size3D float items."
                    .into(),
            ));
        }
        let source_name = self.mode()?;
        let source_mode = match source_name.as_str() {
            "RGB" => PixelMode::RGB,
            "RGBA" => PixelMode::RGBA,
            "CMYK" => PixelMode::CMYK,
            _ => return Err(PilError::ValueError("image has wrong mode".into())),
        };
        let target_name = target_mode.unwrap_or(source_name.as_str());
        let target = match target_name {
            "RGB" => PixelMode::RGB,
            "RGBA" => PixelMode::RGBA,
            "CMYK" => PixelMode::CMYK,
            _ => return Err(PilError::ValueError("image has wrong mode".into())),
        };
        if target.channels() < channels as usize {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }

        Ok(Image::push_mode_changing_op(
            self,
            PipelineOp::Color3DLut {
                size,
                table,
                channels,
                source_mode,
                target_mode: target,
            },
            target_name,
        ))
    }
}
