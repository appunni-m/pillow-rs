//! Parameterized image filters — GaussianBlur, BoxBlur, UnsharpMask,
//! MaxFilter, MinFilter, MedianFilter, ModeFilter, RankFilter.

use image::DynamicImage;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// PIL ModeFilter helper: find most common value in histogram.
/// Returns None if max count ≤ 2 (caller should preserve original pixel).
fn find_mode_with_threshold(hist: &[u32; 256]) -> Option<u8> {
    let mut mode = 0u8;
    let mut max_count = hist[0];
    for v in 1..256 {
        if hist[v] > max_count {
            max_count = hist[v];
            mode = v as u8;
        }
    }
    if max_count > 2 {
        Some(mode)
    } else {
        None
    }
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

    /// Unsharp mask: sharpen by subtracting a blurred version.
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    /// Uses PIL-style GaussianBlur for the blurred version.
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
        let amount = percent as f64 / 100.0;

        // For L mode, process grayscale directly to avoid RGB conversion issues
        if img.color().channel_count() == 1 {
            let gray = img.to_luma8();
            let blur_gray = blurred.to_luma8();
            let mut out = image::GrayImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let p = gray.get_pixel(x, y)[0] as i32;
                    let b = blur_gray.get_pixel(x, y)[0] as i32;
                    let diff = p - b;
                    let val = if diff.unsigned_abs() > threshold as u32 {
                        (p as f64 + diff as f64 * amount).clamp(0.0, 255.0) as u8
                    } else {
                        p as u8
                    };
                    out.put_pixel(x, y, image::Luma([val]));
                }
            }
            return Ok(Image::Loaded(DynamicImage::ImageLuma8(out), None));
        }

        // RGB mode: process per-channel
        let rgb = img.to_rgb8();
        let blur_rgb = blurred.to_rgb8();
        let mut out = image::RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p = rgb.get_pixel(x, y);
                let b = blur_rgb.get_pixel(x, y);
                let mut pix = [0u8; 3];
                for c in 0..3 {
                    let diff = p[c] as i32 - b[c] as i32;
                    if diff.unsigned_abs() > threshold as u32 {
                        let v = (p[c] as f64 + diff as f64 * amount).clamp(0.0, 255.0);
                        pix[c] = v as u8;
                    } else {
                        pix[c] = p[c];
                    }
                }
                out.put_pixel(x, y, image::Rgb(pix));
            }
        }

        Ok(Image::Loaded(DynamicImage::ImageRgb8(out), None))
    }

    /// Max filter: each pixel becomes the maximum in its neighborhood.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(
            self,
            PipelineOp::MaxFilter { size },
        ))
    }

    /// Min filter: each pixel becomes the minimum in its neighborhood.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(
            self,
            PipelineOp::MinFilter { size },
        ))
    }

    /// Median filter: each pixel becomes the median in its neighborhood.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(
            self,
            PipelineOp::MedianFilter { size },
        ))
    }

    /// Mode filter: each pixel becomes the most common value in its neighborhood.
    /// PIL C behavior:
    ///   - Single-band only at C level; multi-band processed per-channel
    ///   - Strict `>` tie-breaking (lower value wins)
    ///   - If max count ≤ 2, original pixel is preserved unchanged
    ///   - Pixels outside image boundary are SKIPPED (not clamped/replicated)
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        let img = self.materialize()?;
        let half = (size / 2) as i32;

        let (w, h) = (img.width(), img.height());
        let w_i32 = w as i32;
        let h_i32 = h as i32;

        // For L (single-channel) mode: process grayscale directly
        if img.color().channel_count() == 1 {
            let gray = img.to_luma8();
            let mut out = image::GrayImage::new(w, h);
            for y in 0..h_i32 {
                for x in 0..w_i32 {
                    let mut hist = [0u32; 256];
                    for dy in -half..=half {
                        let sy = y + dy;
                        if sy < 0 || sy >= h_i32 {
                            continue; // PIL skips out-of-bounds rows
                        }
                        for dx in -half..=half {
                            let sx = x + dx;
                            if sx < 0 || sx >= w_i32 {
                                continue; // PIL skips out-of-bounds columns
                            }
                            hist[gray.get_pixel(sx as u32, sy as u32)[0] as usize] += 1;
                        }
                    }
                    // PIL: maxpixel=0, maxcount=histogram[0]; scan 1..255
                    let mut mode = 0u8;
                    let mut max_count = hist[0];
                    for v in 1..256 {
                        if hist[v] > max_count {
                            max_count = hist[v];
                            mode = v as u8;
                        }
                    }
                    // PIL: if max count ≤ 2, preserve original pixel
                    let val = if max_count > 2 {
                        mode
                    } else {
                        gray.get_pixel(x as u32, y as u32)[0]
                    };
                    out.put_pixel(x as u32, y as u32, image::Luma([val]));
                }
            }
            return Ok(Image::Loaded(DynamicImage::ImageLuma8(out), None));
        }

        // For multi-channel: process per-channel (matching PIL behavior per-band)
        let rgb = img.to_rgb8();
        let mut out = image::RgbImage::new(w, h);
        for y in 0..h_i32 {
            for x in 0..w_i32 {
                let mut r_hist = [0u32; 256];
                let mut g_hist = [0u32; 256];
                let mut b_hist = [0u32; 256];
                for dy in -half..=half {
                    let sy = y + dy;
                    if sy < 0 || sy >= h_i32 {
                        continue;
                    }
                    for dx in -half..=half {
                        let sx = x + dx;
                        if sx < 0 || sx >= w_i32 {
                            continue;
                        }
                        let p = rgb.get_pixel(sx as u32, sy as u32);
                        r_hist[p[0] as usize] += 1;
                        g_hist[p[1] as usize] += 1;
                        b_hist[p[2] as usize] += 1;
                    }
                }
                let r_mode = find_mode_with_threshold(&r_hist);
                let g_mode = find_mode_with_threshold(&g_hist);
                let b_mode = find_mode_with_threshold(&b_hist);
                let orig = rgb.get_pixel(x as u32, y as u32);
                out.put_pixel(x as u32, y as u32, image::Rgb([
                    r_mode.unwrap_or(orig[0]),
                    g_mode.unwrap_or(orig[1]),
                    b_mode.unwrap_or(orig[2]),
                ]));
            }
        }
        let result = crate::image::preserve_mode(&DynamicImage::ImageRgb8(img.to_rgb8()), DynamicImage::ImageRgb8(out));
        Ok(Image::Loaded(result, None))
    }

    /// Rank filter: each pixel becomes the k-th smallest value in its neighborhood.
    pub fn rank_filter(&self, size: u32, rank: u32) -> Result<Image, PilError> {
        let size = size.max(3) | 1; // ensure odd, at least 3
        Ok(Image::push_op(self, PipelineOp::RankFilter { size, rank }))
    }
}
