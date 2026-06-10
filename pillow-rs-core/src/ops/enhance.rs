//! ImageEnhance — brightness, contrast, color, and sharpness adjustments.

use crate::error::PilError;
use crate::image::Image;
use image::DynamicImage;

impl Image {
    /// Adjust brightness by factor. 1.0 = unchanged, 0.0 = black.
    pub fn enhance_brightness(&self, factor: f64) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let result = DynamicImage::ImageRgba8(image::imageops::brighten(
            img,
            (factor * 255.0 - 255.0) as i32,
        ));
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(result),
            format: self.format,
        })
    }

    /// Adjust contrast by factor. 1.0 = unchanged, 0.0 = solid gray.
    pub fn enhance_contrast(&self, factor: f64) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let result = DynamicImage::ImageRgba8(image::imageops::contrast(img, factor as f32));
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(result),
            format: self.format,
        })
    }

    /// Adjust color saturation by factor. 1.0 = unchanged, 0.0 = grayscale.
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let gray = img.to_luma8();
        let mut rgb = img.to_rgb8();
        for (px, gp) in rgb.pixels_mut().zip(gray.pixels()) {
            let g = gp[0] as f64;
            px[0] = ((g + factor * (px[0] as f64 - g)).clamp(0.0, 255.0)) as u8;
            px[1] = ((g + factor * (px[1] as f64 - g)).clamp(0.0, 255.0)) as u8;
            px[2] = ((g + factor * (px[2] as f64 - g)).clamp(0.0, 255.0)) as u8;
        }
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(rgb)),
            format: self.format,
        })
    }

    /// Adjust sharpness by factor. 1.0 = unchanged, <1.0 = blur, >1.0 = sharpen.
    pub fn enhance_sharpness(&self, factor: f64) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        if factor <= 1.0 {
            let sigma = ((1.0 - factor) * 5.0).max(0.01) as f32;
            let result = img.blur(sigma);
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(result),
                format: self.format,
            })
        } else {
            let sigma = ((factor - 1.0) * 0.5).max(0.01) as f32;
            let blurred = img.blur(sigma);
            let blur_rgb = blurred.to_rgb8();
            let mut rgb = img.to_rgb8();
            let amount = (factor - 1.0).min(5.0);
            for (px, bp) in rgb.pixels_mut().zip(blur_rgb.pixels()) {
                for c in 0..3 {
                    let diff = px[c] as f64 - bp[c] as f64;
                    px[c] = ((px[c] as f64 + diff * amount).clamp(0.0, 255.0)) as u8;
                }
            }
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(rgb)),
                format: self.format,
            })
        }
    }
}
