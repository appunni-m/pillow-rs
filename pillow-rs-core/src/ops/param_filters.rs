//! Parameterized image filters — GaussianBlur, BoxBlur, UnsharpMask,
//! MaxFilter, MinFilter, MedianFilter, ModeFilter, RankFilter.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use image::DynamicImage;

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

impl Image {
    /// Gaussian blur with given radius. Larger radius = more blur.
    pub fn gaussian_blur(&self, radius: f32) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::GaussianBlur { sigma: radius },
        ))
    }

    /// Box blur (uniform kernel) with given radius.
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

    /// Unsharp mask: sharpen by subtracting a blurred version.
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    /// Uses PIL-style GaussianBlur for the blurred version.
    /// Handles any number of channels (L=1, LA=2, RGB=3, RGBA=4).
    /// Uses PIL's exact integer arithmetic: `clip8(original + diff * percent / 100)`.
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
        let mut out = vec![0u8; (w * h) as usize * channels];

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
        Ok(Image::Loaded(result, None))
    }

    /// Max filter: each pixel becomes the maximum in its neighborhood.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MaxFilter { size }))
    }

    /// Min filter: each pixel becomes the minimum in its neighborhood.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MinFilter { size }))
    }

    /// Median filter: each pixel becomes the median in its neighborhood.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::MedianFilter { size }))
    }

    /// Mode filter: each pixel becomes the most common value in its neighborhood.
    /// PIL C behavior:
    ///   - Single-band only at C level; multi-band processed per-channel
    ///   - Strict `>` tie-breaking (lower value wins)
    ///   - If max count ≤ 2, original pixel is preserved unchanged
    ///   - Pixels outside image boundary are SKIPPED (not clamped/replicated)
    ///   - Supports any channel count (1=L, 2=LA, 3=RGB, 4=RGBA)
    ///   - For P-mode (palette): operates on palette indices, preserves palette
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3

        // For palette images: extract palette before materialize
        let palette = self.palette();
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
            }));
        }
        // Preserve explicit mode (e.g. "1", "P" via explicit_mode)
        if explicit.is_some() {
            return Ok(Image::Loaded(result, explicit));
        }
        Ok(Image::Loaded(result, None))
    }

    /// Rank filter: each pixel becomes the k-th smallest value in its neighborhood.
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::RankFilter { size, rank }))
    }
}
