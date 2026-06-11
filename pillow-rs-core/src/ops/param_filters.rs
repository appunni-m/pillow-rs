//! Parameterized image filters — GaussianBlur, BoxBlur, UnsharpMask,
//! MaxFilter, MinFilter, MedianFilter, ModeFilter, RankFilter.

use image::DynamicImage;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

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
    /// NOTE: Not yet a PipelineOp variant; executes immediately.
    pub fn unsharp_mask(
        &self,
        radius: f32,
        percent: i32,
        threshold: u8,
    ) -> Result<Image, PilError> {
        let img = self.materialize()?;
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

        Ok(Image::Loaded(DynamicImage::ImageRgb8(out)))
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
    /// NOTE: Not yet a PipelineOp variant; executes immediately.
    pub fn mode_filter(&self, size: u32) -> Result<Image, PilError> {
        let img = self.materialize()?;
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

        Ok(Image::Loaded(DynamicImage::ImageRgb8(out)))
    }
}
