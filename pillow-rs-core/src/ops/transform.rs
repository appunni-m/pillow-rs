//! Image.transform — affine, perspective, and mesh transforms.
//! Implements AFFINE using bilinear interpolation.

use image::{DynamicImage, GenericImageView, RgbImage};

use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Apply an affine transform: `[a, b, c, d, e, f]` where
    /// x' = a*x + b*y + c,  y' = d*x + e*y + f
    pub fn transform_affine(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fillcolor: (u8, u8, u8, u8),
    ) -> Result<Image, PilError> {
        if matrix.len() != 6 {
            return Err(PilError::ValueError(
                "Affine transform requires 6 coefficients [a,b,c,d,e,f]".into(),
            ));
        }
        let (a, b, c, d, e, f) = (matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]);
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let rgb = img.to_rgb8();
        let (src_w, src_h) = (rgb.width(), rgb.height());
        let (dst_w, dst_h) = size;

        let mut out = RgbImage::from_pixel(dst_w, dst_h, image::Rgb([fillcolor.0, fillcolor.1, fillcolor.2]));

        // Inverse transform: for each destination pixel, find source pixel
        let det = a * e - b * d;
        if det.abs() < 1e-10 {
            return Err(PilError::ValueError("Singular affine matrix".into()));
        }
        let inv_a = e / det;
        let inv_b = -b / det;
        let inv_c = (b * f - e * c) / det;
        let inv_d = -d / det;
        let inv_e = a / det;
        let inv_f = (d * c - a * f) / det;

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                let sx = inv_a * dx as f64 + inv_b * dy as f64 + inv_c;
                let sy = inv_d * dx as f64 + inv_e * dy as f64 + inv_f;

                if sx >= 0.0 && sx < (src_w - 1) as f64 && sy >= 0.0 && sy < (src_h - 1) as f64 {
                    let sx_u = sx as u32;
                    let sy_u = sy as u32;
                    let fx = sx - sx_u as f64;
                    let fy = sy - sy_u as f64;

                    // Bilinear interpolation
                    let p00 = rgb.get_pixel(sx_u, sy_u);
                    let p10 = rgb.get_pixel((sx_u + 1).min(src_w - 1), sy_u);
                    let p01 = rgb.get_pixel(sx_u, (sy_u + 1).min(src_h - 1));
                    let p11 = rgb.get_pixel((sx_u + 1).min(src_w - 1), (sy_u + 1).min(src_h - 1));

                    for c in 0..3 {
                        let v = (1.0 - fx) * (1.0 - fy) * p00[c] as f64
                            + fx * (1.0 - fy) * p10[c] as f64
                            + (1.0 - fx) * fy * p01[c] as f64
                            + fx * fy * p11[c] as f64;
                        out.get_pixel_mut(dx, dy)[c] = v.round().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
            format: self.format,
        })
    }
}
