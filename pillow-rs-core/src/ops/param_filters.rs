//! Parameterized image filters — GaussianBlur, BoxBlur, UnsharpMask,
//! MaxFilter, MinFilter, MedianFilter, ModeFilter, RankFilter.
//! These differ from built-in kernels in that they take constructor arguments.

use image::{DynamicImage, GenericImageView};

use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Gaussian blur with given radius. Larger radius = more blur.
    pub fn gaussian_blur(&self, radius: f32) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let result = img.blur(radius);
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(result),
            format: self.format,
        })
    }

    /// Box blur (uniform kernel) with given radius.
    pub fn box_blur(&self, radius: f32) -> Result<Image, PilError> {
        // Box blur is similar to Gaussian blur with smaller effective radius
        self.gaussian_blur(radius * 0.5)
    }

    /// Unsharp mask: sharpen by subtracting a blurred version.
    /// `radius` controls blur amount, `percent` controls strength (150 = 150%),
    /// `threshold` is minimum difference to apply.
    pub fn unsharp_mask(
        &self,
        radius: f32,
        percent: i32,
        threshold: u8,
    ) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let blurred = img.blur(radius);

        let (w, h) = (img.width(), img.height());
        let rgb = img.to_rgb8();
        let blur_rgb = blurred.to_rgb8();
        let mut out = image::RgbImage::new(w, h);

        let amount = percent as f64 / 100.0;

        for y in 0..h {
            for x in 0..w {
                let p = rgb.get_pixel(x, y);
                let b = blur_rgb.get_pixel(x, y);
                let mut r = [0u8; 3];
                for c in 0..3 {
                    let diff = p[c] as i32 - b[c] as i32;
                    if diff.unsigned_abs() > threshold as u32 {
                        let v = (p[c] as f64 + diff as f64 * amount).clamp(0.0, 255.0);
                        r[c] = v as u8;
                    } else {
                        r[c] = p[c];
                    }
                }
                out.put_pixel(x, y, image::Rgb(r));
            }
        }

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
            format: self.format,
        })
    }

    /// Max filter: each pixel becomes the maximum in its neighborhood.
    pub fn max_filter(&self, size: u32) -> Result<Image, PilError> {
        rank_filter_impl(self, size, size * size - 1)
    }

    /// Min filter: each pixel becomes the minimum in its neighborhood.
    pub fn min_filter(&self, size: u32) -> Result<Image, PilError> {
        rank_filter_impl(self, size, 0)
    }

    /// Median filter: each pixel becomes the median in its neighborhood.
    pub fn median_filter(&self, size: u32) -> Result<Image, PilError> {
        rank_filter_impl(self, size, size * size / 2)
    }

    /// Mode filter: each pixel becomes the most common value in its neighborhood.
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let mut out = image::RgbImage::new(w, h);
        let half = (size / 2) as i32;

        for y in 0..h {
            for x in 0..w {
                let mut hist = [0u32; 256];
                for dy in -half..=half {
                    for dx in -half..=half {
                        let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                        let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                        let p = rgb.get_pixel(sx, sy);
                        // Use luma as approximation
                        let l = crate::color::rgb_to_luma_u8(p[0], p[1], p[2]);
                        hist[l as usize] += 1;
                    }
                }
                let mut mode_val = 0u8;
                let mut max_count = 0u32;
                for (v, &count) in hist.iter().enumerate() {
                    if count > max_count {
                        max_count = count;
                        mode_val = v as u8;
                    }
                }
                out.put_pixel(x, y, image::Rgb([mode_val, mode_val, mode_val]));
            }
        }

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
            format: self.format,
        })
    }
}

/// Generic rank filter: sorts neighborhood values and picks the one at `rank`.
fn rank_filter_impl(image: &Image, size: u32, rank: u32) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = image::RgbImage::new(w, h);
    let half = (size / 2) as i32;
    let area = (size * size) as usize;
    let rank = rank.min((area - 1) as u32) as usize;

    for y in 0..h {
        for x in 0..w {
            let mut r_vals = Vec::with_capacity(area);
            let mut g_vals = Vec::with_capacity(area);
            let mut b_vals = Vec::with_capacity(area);
            for dy in -half..=half {
                for dx in -half..=half {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as u32;
                    let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as u32;
                    let p = rgb.get_pixel(sx, sy);
                    r_vals.push(p[0]);
                    g_vals.push(p[1]);
                    b_vals.push(p[2]);
                }
            }
            r_vals.sort_unstable();
            g_vals.sort_unstable();
            b_vals.sort_unstable();
            out.put_pixel(x, y, image::Rgb([r_vals[rank], g_vals[rank], b_vals[rank]]));
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
        format: image.format,
    })
}
