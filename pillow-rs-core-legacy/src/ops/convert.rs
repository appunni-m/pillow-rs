use image::DynamicImage;

use crate::color::{pil_grayscale, pil_grayscale_alpha};
use crate::error::PilError;
use crate::image::Image;

/// Convert image between modes.
/// Supports: L, LA, RGB, RGBA, 1 (bilevel, with/without Floyd-Steinberg dither)
/// Matrix-based conversion: 4-element for single-channel→RGB, 12-element for RGB→RGB color space
impl Image {
    pub fn convert(
        &self,
        mode: &str,
        matrix: Option<Vec<f64>>,
        dither: Option<&str>,
        _palette: Option<&str>,
        _colors: Option<u32>,
    ) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;

        if let Some(mat) = matrix {
            return convert_with_matrix(img, mode, &mat)
                .map(|result| Image {
                    inner: crate::lazy::LazyImage::Loaded(result),
                    format: self.format,
                });
        }

        let converted = match mode {
            "L" => DynamicImage::ImageLuma8(pil_grayscale(img)),
            "LA" => DynamicImage::ImageLumaA8(pil_grayscale_alpha(img)),
            "RGB" => image::DynamicImage::ImageRgb8(img.to_rgb8()),
            "RGBA" => image::DynamicImage::ImageRgba8(img.to_rgba8()),
            "1" => {
                let apply_dither = match dither {
                    Some("NONE") | Some("none") => false,
                    _ => true, // default: Floyd-Steinberg
                };
                convert_to_bilevel(img, apply_dither)?
            }
            _ => {
                return Err(PilError::NotImplementedError(format!(
                    "Conversion to mode '{}' not yet implemented",
                    mode
                )));
            }
        };

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(converted),
            format: self.format,
        })
    }
}

fn convert_with_matrix(
    img: &image::DynamicImage,
    target_mode: &str,
    matrix: &[f64],
) -> Result<image::DynamicImage, PilError> {
    match (matrix.len(), target_mode) {
        (4, "RGB") => {
            let luma = img.to_luma8();
            let (w, h) = luma.dimensions();
            let pixels: Vec<u8> = luma
                .iter()
                .flat_map(|&l| {
                    let lf = l as f64;
                    [
                        (matrix[0] * lf).clamp(0.0, 255.0) as u8,
                        (matrix[1] * lf).clamp(0.0, 255.0) as u8,
                        (matrix[2] * lf).clamp(0.0, 255.0) as u8,
                    ]
                })
                .collect();
            Ok(image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "matrix conversion failed"))))?,
            ))
        }
        (12, "RGB") => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let pixels: Vec<u8> = rgb
                .pixels()
                .flat_map(|p| {
                    let r = p[0] as f64;
                    let g = p[1] as f64;
                    let b = p[2] as f64;
                    [
                        (matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3]).clamp(0.0, 255.0) as u8,
                        (matrix[4] * r + matrix[5] * g + matrix[6] * b + matrix[7]).clamp(0.0, 255.0) as u8,
                        (matrix[8] * r + matrix[9] * g + matrix[10] * b + matrix[11]).clamp(0.0, 255.0) as u8,
                    ]
                })
                .collect();
            Ok(image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "matrix conversion failed"))))?,
            ))
        }
        (n, _) => Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}", n
        ))),
    }
}

fn convert_to_bilevel(
    img: &image::DynamicImage,
    apply_dither: bool,
) -> Result<image::DynamicImage, PilError> {
    let mut luma = image::imageops::colorops::grayscale(img);
    if apply_dither {
        image::imageops::colorops::dither(&mut luma, &image::imageops::colorops::BiLevel);
    } else {
        for p in luma.pixels_mut() {
            p[0] = if p[0] > 127 { 255 } else { 0 };
        }
    }
    Ok(image::DynamicImage::ImageLuma8(luma))
}
